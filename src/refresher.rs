use std::collections::{BTreeMap, HashMap};
use std::iter::zip;
use std::sync::Arc;

use rand::Rng;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use sessions::Storage;
use tokio::time::sleep;

use crate::config::Config;
use crate::handlers::auth::{API_AUTHENTICATION_URL, SESSION_FIELD_TOKEN};
use crate::model::AccessToken;
use crate::storage::ValkeyStorage;

const SHORT_SLEEP_SECS: u64 = 30;
const LONG_SLEEP_SECS: u64 = 60 * 60;
const AVG_UPDATES_PER_MINUTE: u64 = 50;
const UPDATE_THRESHOLD_SECS: i32 = 4 * 60 * 60;

pub struct TokenRefresher {
    config: Config,
    storage: ValkeyStorage,

    task: Option<tokio::task::JoinHandle<()>>,
}

impl TokenRefresher {
    pub fn new(config: Config, storage: ValkeyStorage) -> Self {
        Self {
            config,
            storage,
            task: None,
        }
    }

    pub fn start(&mut self) {
        if self.task.is_none() {
            let config = self.config.clone();
            let storage = self.storage.clone();
            self.task = Some(tokio::spawn(refresher_loop(config, storage)));
        }
    }

    pub async fn stop(&mut self) {
        if let Some(th) = self.task.take() {
            th.abort();
        }
    }
}

async fn refresher_loop(config: Config, storage: ValkeyStorage) {
    let config = Arc::new(config);
    let storage = Arc::new(storage);

    loop {
        match storage.client.get_multiplexed_tokio_connection().await {
            Err(e) => {
                log::error!("Failed to connect to Valkey: {}", e);
                sleep(std::time::Duration::from_secs(SHORT_SLEEP_SECS)).await;
            }
            Ok(mut conn) => {
                let now = std::time::Instant::now();
                match conn.keys::<&str, Vec<String>>("*").await {
                    Err(e) => {
                        log::error!("Failed to read all sessions from Valkey: {}", e);
                        sleep(std::time::Duration::from_secs(SHORT_SLEEP_SECS)).await;
                    }
                    Ok(all_sessions) => {
                        log::info!(
                            "{} session(s) total ({}ms)",
                            all_sessions.len(),
                            now.elapsed().as_millis()
                        );

                        let now = std::time::Instant::now();
                        let mut ttl_tasks = Vec::with_capacity(all_sessions.len());
                        for k in all_sessions.iter().cloned() {
                            ttl_tasks.push(tokio::spawn(fetch_ttl(conn.clone(), k)));
                        }
                        let mut scheduled_for_update = Vec::with_capacity(all_sessions.len());
                        for (key, handle) in zip(all_sessions.into_iter(), ttl_tasks) {
                            if let Ok(Some(exp)) = handle.await {
                                if exp <= UPDATE_THRESHOLD_SECS {
                                    scheduled_for_update.push(key);
                                }
                            }
                        }

                        let keys_len = scheduled_for_update.len();
                        log::info!(
                            "{} session(s) with ttl less than {}s ({}ms)",
                            keys_len,
                            UPDATE_THRESHOLD_SECS,
                            now.elapsed().as_millis()
                        );

                        let mut successes = 0;
                        let mut failures = 0;
                        let now = std::time::Instant::now();

                        let mut tasks = Vec::with_capacity(keys_len);
                        for k in scheduled_for_update.into_iter() {
                            let task = tokio::spawn(refresh_single_token(
                                config.clone(),
                                storage.clone(),
                                conn.clone(),
                                k,
                                keys_len as u64,
                            ));
                            tasks.push(task);
                        }

                        for handle in tasks {
                            match handle.await {
                                Ok(_) => successes += 1,
                                Err(e) => {
                                    log::warn!(
                                        "Failed to update one of tokens due to unhandled error: {}",
                                        e
                                    );
                                    failures += 1
                                }
                            }
                        }

                        log::info!(
                            "Success: {}, failure: {} ({}ms) -- sleeping {}s",
                            successes,
                            failures,
                            now.elapsed().as_millis(),
                            LONG_SLEEP_SECS
                        );

                        sleep(std::time::Duration::from_secs(LONG_SLEEP_SECS)).await;
                    }
                }
            }
        }
    }
}

async fn fetch_ttl(mut conn: MultiplexedConnection, k: String) -> Option<i32> {
    match conn.ttl::<String, i32>(k).await {
        Ok(val) => Some(val),
        Err(_) => None,
    }
}

fn make_token_refresh_request(config: &Config, refresh_token: &str) -> reqwest::Request {
    reqwest::Client::new()
        .post(API_AUTHENTICATION_URL)
        .form(&HashMap::from([
            ("client_id", config.api.client_id.to_string()),
            ("client_secret", config.api.client_secret.clone()),
            ("grant_type", "refresh_token".to_owned()),
            ("refresh_token", refresh_token.to_owned()),
        ]))
        .header("Accept", "application/json")
        .build()
        .unwrap()
}

async fn refresh_single_token(
    config: Arc<Config>,
    storage: Arc<ValkeyStorage>,
    mut conn: MultiplexedConnection,
    key: String,
    total_keys: u64,
) -> eyre::Result<()> {
    // Distribute updates evenly across time, so that the API isn't DoSed to hell -- the intention is ~1 RPS.
    let max_wait_time = total_keys / AVG_UPDATES_PER_MINUTE;
    if max_wait_time > 0 {
        let sleep_duration =
            std::time::Duration::from_secs(rand::thread_rng().gen_range(0..max_wait_time));
        sleep(sleep_duration).await;
    }

    let session_data = conn.get::<&str, String>(&key).await?;
    let mut deserialized: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&session_data)?;

    if let Some(token) = deserialized.get(SESSION_FIELD_TOKEN) {
        let token: AccessToken = serde_json::from_value(token.clone())?;

        let request = make_token_refresh_request(&config, &token.refresh_token);
        let result = reqwest::Client::new().execute(request).await;

        match result {
            Err(e) => {
                log::warn!(
                    "Failed to refresh osu! API token for {}: {}. Removing the whole session",
                    key,
                    e
                );
                if let Err(e) = (*storage).remove(&key).await {
                    log::error!("Failed to remove the session from storage: {}", e);
                }
            }
            Ok(result) => {
                let text = result.text().await?;
                let token: AccessToken = serde_json::from_str(&text)?;
                let exp = std::time::Duration::from_secs(token.expires_in.try_into().unwrap());

                deserialized.insert(SESSION_FIELD_TOKEN.to_owned(), serde_json::to_value(token)?);
                if let Err(e) = (*storage).set(&key, deserialized, &exp).await {
                    log::error!("Failed to insert {} with the updated token: {}", key, e);
                }
            }
        }
    }

    Ok(())
}
