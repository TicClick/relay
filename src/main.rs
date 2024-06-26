use std::fmt::Write;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::str::FromStr;

use eyre::Result;
use refresher::TokenRefresher;
use storage::ValkeyStorage;
use storage::SESSION_COOKIE_NAME;
use tokio::net::TcpListener;

use viz::types::State;
use viz::{
    middleware::{
        cookie,
        helper::CookieOptions,
        session::{self, Store},
    },
    types::CookieKey,
};
use viz::{serve, Router};

pub mod config;
pub mod handlers;
pub mod middleware;
pub mod model;
pub mod refresher;
pub mod storage;
pub mod templates;

const DEFAULT_CONFIG_PATH: &str = "./config.yaml";

fn bin2hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, b| {
        let _ = write!(output, "{b:02X}");
        output
    })
}

fn hex2bin(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

pub fn generate_session_id() -> String {
    nanoid::nanoid!(64)
}

pub fn verify_session_id(sid: &str) -> bool {
    sid.len() == 64
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let mut c = config::Config::load(DEFAULT_CONFIG_PATH)?;
    let bind_ip = Ipv4Addr::from_str(&c.service.bind_host)?;
    let addr = SocketAddr::from((bind_ip, c.service.bind_port));

    let listener = TcpListener::bind(addr).await?;

    let key = match c.service.cookie_key {
        Some(ref k) => CookieKey::from(&hex2bin(k)),
        None => {
            let key = CookieKey::generate();
            c.service.cookie_key = Some(bin2hex(key.master()));
            c.save(DEFAULT_CONFIG_PATH)?;
            key
        }
    };

    let storage = storage::ValkeyStorage::new(&c);

    let app = Router::new()
        .get("/", handlers::index::index)
        .nest(
            "/auth",
            Router::new()
                .get("/", handlers::auth::index)
                .get("/logout", handlers::auth::logout),
        )
        .get("/api/token", handlers::api::token)
        .with(middleware::Config::new(c.service.max_concurrent_requests))
        .with(State::<config::Config>::new(c.clone()))
        .with(State::<ValkeyStorage>::new(storage.clone()))
        .with(session::Config::new(
            Store::new(storage.clone(), generate_session_id, verify_session_id),
            CookieOptions::default().name(SESSION_COOKIE_NAME),
        ))
        .with(cookie::Config::with_key(key));

    let mut refresher = TokenRefresher::new(c, storage);
    refresher.start();

    serve(listener, app).await?;

    Ok(())
}
