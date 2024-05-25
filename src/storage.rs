use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use redis::Commands;
use sessions::Storage;

use crate::config::Config;

pub const SESSION_COOKIE_NAME: &str = "session-id";

#[derive(Clone)]
pub struct ValkeyStorage {
    client: redis::Client,
    cache: Arc<Mutex<HashMap<String, sessions::Data>>>,
}

impl ValkeyStorage {
    pub fn new(c: &Config) -> Self {
        Self {
            client: redis::Client::open(c.service.valkey.address.to_owned()).unwrap(),
            cache: Arc::default(),
        }
    }
}

impl Storage for ValkeyStorage {
    async fn get(&self, key: &str) -> std::io::Result<Option<sessions::Data>> {
        log::info!("Loading session: {}", key);

        if let Some(v) = self.cache.lock().unwrap().get(key) {
            return Ok(Some(v.clone()));
        }

        let mut conn = self.client.get_connection().unwrap();
        match conn.get::<&str, String>(key) {
            redis::RedisResult::Ok(v) => match serde_json::from_str(&v) {
                Ok(loaded) => Ok(Some(loaded)),
                Err(e) => {
                    log::error!("Error while deserializing key from Valkey: {}", e);
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                }
            },
            redis::RedisResult::Err(e) => {
                log::error!("Error while loading key from Valkey: {}", e);
                Err(std::io::Error::new(std::io::ErrorKind::Other, e))
            }
        }
    }

    async fn set(
        &self,
        key: &str,
        val: sessions::Data,
        exp: &std::time::Duration,
    ) -> std::io::Result<()> {
        log::info!("Saving session: {} (exp: {:?})", key, exp);

        self.cache
            .lock()
            .unwrap()
            .insert(key.to_owned(), val.clone());

        let mut conn = self.client.get_connection().unwrap();
        match serde_json::to_string(&val) {
            Err(e) => {
                log::error!("Failed to serialize session to string: {}", e);
                Ok(())
            }
            Ok(serialized) => match conn.set_ex::<&str, String, ()>(key, serialized, exp.as_secs()) {
                Ok(_) => Ok(()),
                Err(e) => {
                    log::error!("Failed to save session to Valkey: {}", e);
                    Ok(())
                }
            },
        }
    }

    async fn remove(&self, key: &str) -> std::io::Result<()> {
        log::info!("removing session: {}", key);

        self.cache.lock().unwrap().remove(key);

        let mut conn = self.client.get_connection().unwrap();
        if let Err(e) = conn.del::<&str, ()>(key) {
            log::error!("Error while deleting key from Valkey: {}", e);
        }

        Ok(())
    }
}
