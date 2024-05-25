use eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api: API,
    pub service: Service,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct API {
    pub client_id: u64,
    pub client_secret: String,
    pub redirect_url: String,
    pub scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub bind_host: String,
    pub bind_port: u16,
    pub cookie_key: Option<String>,
    pub valkey: Valkey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Valkey {
    pub address: String,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let data = std::fs::read_to_string(path)?;
        serde_yaml::from_str::<Config>(&data).map_err(|e| e.into())
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let data = serde_yaml::to_string(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }
}
