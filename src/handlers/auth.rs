use std::collections::HashMap;

use nanoid::nanoid;
use reqwest;
use viz::{IntoResponse, Request, RequestExt, Response, ResponseExt, StatusCode};

use crate::config::{self, Config};
use crate::model::{AccessToken, OAuth2FeedbackQuery, UserCompact};
use crate::storage::SESSION_COOKIE_NAME;
use crate::templates::auth::{AuthErrorPage, AuthInitiationPage, AuthSuccessPage};

pub const API_AUTHORIZATION_URL: &str = "https://osu.ppy.sh/oauth/authorize";
pub const API_AUTHENTICATION_URL: &str = "https://osu.ppy.sh/oauth/token";

pub const SESSION_FIELD_STATE: &str = "state";
pub const SESSION_FIELD_TOKEN: &str = "token";

fn make_authorization_url(config: &Config) -> (reqwest::Url, String) {
    let state = nanoid!(10);
    let url = reqwest::Url::parse_with_params(
        API_AUTHORIZATION_URL,
        &[
            ("client_id", config.api.client_id.to_string()),
            ("redirect_uri", config.api.redirect_url.clone()),
            ("response_type", "code".to_owned()),
            ("scope", config.api.scope.join(" ")),
            ("state", state.clone()),
        ],
    )
    .unwrap();
    (url, state)
}

fn make_authentication_request(config: &Config, query: &OAuth2FeedbackQuery) -> reqwest::Request {
    reqwest::Client::new()
        .post(API_AUTHENTICATION_URL)
        .form(&HashMap::from([
            ("client_id", config.api.client_id.to_string()),
            ("client_secret", config.api.client_secret.clone()),
            ("code", query.code.clone()),
            ("grant_type", "authorization_code".to_owned()),
            ("redirect_uri", config.api.redirect_url.clone()),
            ("state", query.state.clone()),
        ]))
        .header("Accept", "application/json")
        .build()
        .unwrap()
}

fn show_authentication_page(r: Request, config: &config::Config) -> viz::Result<Response> {
    let (url, state) = make_authorization_url(config);
    match r.session().set(SESSION_FIELD_STATE, state) {
        Ok(_) => Ok(Response::html(
            AuthInitiationPage {
                auth_url: url.as_ref(),
            }
            .to_string(),
        )),
        Err(e) => Ok(Response::html(format!(
            "failed to set authentication state: {}",
            e
        ))),
    }
}

fn show_success_page(
    data: UserCompact,
    token: AccessToken,
    session_id: &str,
) -> viz::Result<Response> {
    Ok(Response::html(
        AuthSuccessPage {
            data,
            token,
            session_id,
            logout_url: "/auth/logout",
        }
        .to_string(),
    ))
}

fn show_authentication_error(error: &str) -> viz::Result<Response> {
    Ok(Response::html(
        AuthErrorPage {
            error,
            logout_url: "/auth/logout",
        }
        .to_string(),
    ))
}

async fn show_index_with_user_data(
    client: reqwest::Client,
    token: AccessToken,
    session_id: &str,
) -> viz::Result<Response> {
    let user_data_request = client
        .get(reqwest::Url::parse("https://osu.ppy.sh/api/v2/me").unwrap())
        .header("Accept", "application/json")
        .bearer_auth(token.access_token.to_owned())
        .build()
        .unwrap();

    match client.execute(user_data_request).await {
        Err(e) => show_authentication_error(&format!(
            "failed to check who you are through the osu! API: {}",
            e
        )),
        Ok(response) => {
            let text = response.text().await.unwrap();
            let user_data: UserCompact = serde_json::from_str(&text).unwrap();
            show_success_page(user_data, token, session_id)
        }
    }
}

pub async fn index(r: Request) -> viz::Result<Response> {
    let config = r
        .state::<config::Config>()
        .ok_or_else(|| StatusCode::INTERNAL_SERVER_ERROR.into_error())?;

    let maybe_state = r.session().get::<String>(SESSION_FIELD_STATE)?;
    let maybe_query = r.query::<OAuth2FeedbackQuery>();

    match maybe_query {
        Err(outer_error) => {
            let token = r.session().get::<AccessToken>(SESSION_FIELD_TOKEN).unwrap();
            match token {
                Some(t) => {
                    let cookie_storage = r.cookies().unwrap();
                    let session_id_cookie = r.cookie(SESSION_COOKIE_NAME).unwrap();
                    let decrypted = cookie_storage.private_decrypt(session_id_cookie);
                    show_index_with_user_data(reqwest::Client::new(), t, decrypted.unwrap().value())
                        .await
                }
                None => {
                    if outer_error.to_string().contains("missing field") {
                        show_authentication_page(r, &config)
                    } else {
                        r.session().remove(SESSION_FIELD_STATE);
                        show_authentication_error(&format!(
                            "error while reading query string: {}",
                            outer_error
                        ))
                    }
                }
            }
        }
        Ok(query) => match maybe_state {
            None => {
                r.session().remove(SESSION_FIELD_STATE);
                show_authentication_page(r, &config)
            }
            Some(ref state) => {
                if state != &query.state {
                    r.session().remove(SESSION_FIELD_STATE);
                    show_authentication_error(
                        "local authentication state doesn't match that of osu! web -- try again",
                    )
                } else {
                    let client = reqwest::Client::new();
                    match client
                        .execute(make_authentication_request(&config, &query))
                        .await
                    {
                        Err(e) => show_authentication_error(&format!(
                            "failed to request the API token: {}",
                            e
                        )),
                        Ok(response) => {
                            let text = response.text().await.unwrap();
                            let token: AccessToken = serde_json::from_str(&text).unwrap();
                            match r.session().set(SESSION_FIELD_TOKEN, token) {
                                Ok(()) => {
                                    Ok(Response::redirect_with_status("/auth", StatusCode::FOUND))
                                }
                                Err(e) => show_authentication_error(&format!(
                                    "failed to save the obtained API token: {}",
                                    e
                                )),
                            }
                        }
                    }
                }
            }
        },
    }
}

pub async fn logout(r: Request) -> viz::Result<Response> {
    r.session().clear();
    Ok(Response::redirect_with_status("/auth", StatusCode::FOUND))
}
