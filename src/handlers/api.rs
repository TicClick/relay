use sessions::Storage;
use viz::{IntoResponse, Request, RequestExt, Response, ResponseExt, StatusCode};

use super::auth::SESSION_FIELD_TOKEN;
use crate::storage::ValkeyStorage;

const SESSION_HEADER_NAME: &str = "X-Relay-Session";

pub async fn token(r: Request) -> viz::Result<Response> {
    let session_header = r
        .headers()
        .get(SESSION_HEADER_NAME)
        .ok_or(StatusCode::UNAUTHORIZED.into_error())?;
    let session_id = session_header.to_str().unwrap();

    let session_storage: ValkeyStorage = r
        .state()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR.into_error())?;

    match session_storage.get(session_id).await {
        Err(e) => {
            log::error!(
                "Error while loading the token of {} from Valkey: {}",
                session_id,
                e
            );
            Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Ok(Some(session)) => {
            if let Some(t) = session.get(SESSION_FIELD_TOKEN) {
                Ok(Response::json(t.to_string()).unwrap())
            } else {
                Ok(StatusCode::NOT_FOUND.into_response())
            }
        }
        Ok(None) => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}
