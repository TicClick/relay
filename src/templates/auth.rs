use markup::{self, Render};

use crate::model::{AccessToken, UserCompact};

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
    AuthSuccessPage<'a>(data: UserCompact, token: AccessToken, session_id: &'a str, logout_url: &'a str) {
        @BaseTemplate {
            title: "authentication",
            content: _AuthSuccessContent { data, token, session_id, logout_url }
        }
    }

    _AuthSuccessContent<'a> (data: &'a UserCompact, token: &'a AccessToken, session_id: &'a str, logout_url: &'a str) {
        h2 { "Status" }
        p {
            a[href = format!("https://osu.ppy.sh/users/{}", data.user_id)] {
                img[
                    src = &data.avatar_url,
                    alt = "your avatar",
                    title = data.username.to_string() + " (#" + &data.user_id.to_string() + ")"
                ] {}
            }
        }
        p {
            "Current osu! API token: " code {
                @token.access_token[0..8] "..." @token.access_token[token.access_token.len() - 8..]
            } "(obtained at: " @token.obtained_at().to_string() ", expires in: " @token.lifetime() " seconds)"
            br { }
            "Your relay session identifier: " code { @session_id }
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
