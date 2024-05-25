use markup::{self, Render};
use rosu_v2::model::user::UserCompact;

use crate::token::AccessToken;

const WEBSITE_TITLE: &str = "relay";

pub trait MarkupTemplate {
    fn to_string(&self) -> String;
}

markup::define! {
    Footer() {
        footer { small { "relay | uses " a[href = "https://github.com/andybrewer/mvp/" ] { "MVP.css" } } }
    }

    BaseTemplate<'a, Content: Render>(title: &'a str, content: Content) {
        @markup::doctype()

        html {
            head {
                title { @title " – " @WEBSITE_TITLE }
                link [rel = "stylesheet", href = "https://unpkg.com/mvp.css" ] {}
            }
        }
        body {
            main {
                article { @content }
            }
        }

        @Footer { }
    }
}

markup::define! {
    AuthSuccessPage<'a>(user_data: Option<UserCompact>, token: AccessToken, logout_url: &'a str) {
        @BaseTemplate {
            title: "authentication",
            content: _AuthSuccessContent { user_data, token, logout_url }
        }
    }

    _AuthSuccessContent<'a> (user_data: &'a Option<UserCompact>, token: &'a AccessToken, logout_url: &'a str) {
        h2 { "Status" }
        @if let Some(data) = user_data {
            p {
                a[href = format!("https://osu.ppy.sh/users/{}", data.user_id)] {
                    img[
                        src = &data.avatar_url,
                        alt = "your avatar",
                        title = data.username.to_string() + " (#" + &data.user_id.to_string() + ")"
                    ] {}
                }
            }
        }
        p {
            "Current osu! API token: " code {
                @token.access_token[0..8] "..." @token.access_token[token.access_token.len() - 8..]
            } "(obtained at: " @token.obtained_at().to_string() ", expires in: " @token.lifetime() " seconds)"
            br { }
            a[href = logout_url] { b { "Log out" } }
        }
    }
}

markup::define! {
    AuthInitiationPage<'a>(auth_url: &'a str) {
        @BaseTemplate {
            title: "authentication",
            content: _AuthInitiationContent { auth_url }
        }
    }

    _AuthInitiationContent<'a>(auth_url: &'a str) {
        h2 { "Start using osu! API" }
        p {
            a[href = auth_url] { b { "Authenticate" } }
        }
    }
}

markup::define! {
    AuthErrorPage<'a>(error: &'a str, logout_url: &'a str) {
        @BaseTemplate {
            title: "authentication error",
            content: _AuthErrorContent { error, logout_url }
        }
    }

    _AuthErrorContent<'a>(error: &'a str, logout_url: &'a str) {
        h2 { "Authentication error" }
        p {
            aside { @error }
            a[href = logout_url] { b { "Try again" } }
        }
    }
}
