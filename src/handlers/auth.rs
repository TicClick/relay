use std::collections::HashMap;

use nanoid::nanoid;
use reqwest;
use rosu_v2::model::user::UserCompact;
use serde::{Deserialize, Serialize};
use viz::{IntoResponse, Request, RequestExt, Response, ResponseExt, StatusCode};

use crate::{
    config::{self, Config},
    templates::auth::{AuthErrorPage, AuthInitiationPage, AuthSuccessPage},
    token::AccessToken,
};

pub const API_AUTHORIZATION_URL: &str = "https://osu.ppy.sh/oauth/authorize";
pub const API_AUTHENTICATION_URL: &str = "https://osu.ppy.sh/oauth/token";

pub const SESSION_FIELD_STATE: &str = "state";
pub const SESSION_FIELD_TOKEN: &str = "token";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OAuth2FeedbackQuery {
    pub code: String,
    pub state: String,
}

fn make_authorization_url(config: &Config) -> (reqwest::Url, String) {
    let state = nanoid!(10);
    let url = reqwest::Url::parse_with_params(
        API_AUTHORIZATION_URL,
        &[
            ("client_id", config.api.client_id.to_string()),
            ("client_secret", config.api.client_secret.clone()),
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

fn show_success_page(user_data: Option<UserCompact>, token: AccessToken) -> viz::Result<Response> {
    Ok(Response::html(
        AuthSuccessPage {
            user_data,
            token,
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

async fn test_and_save_token(
    r: Request,
    client: reqwest::Client,
    token: AccessToken,
) -> viz::Result<Response> {
    let user_data_request = client
        .get(reqwest::Url::parse("https://osu.ppy.sh/api/v2/me").unwrap())
        .header("Accept", "application/json")
        .bearer_auth(token.access_token.to_owned())
        .build()
        .unwrap();

    match client.execute(user_data_request).await {
        Err(e) => show_authentication_error(&format!(
            "failed to check who you are through the osu! api: {}",
            e
        )),
        Ok(response) => {
            r.session().set(SESSION_FIELD_TOKEN, token.clone()).unwrap();
            let text = response.text().await.unwrap();
            let user_data: UserCompact = serde_json::from_str(&text).unwrap();
            show_success_page(Some(user_data), token)
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
                Some(t) => test_and_save_token(r, reqwest::Client::new(), t).await,
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
                            test_and_save_token(r, client, token).await
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
