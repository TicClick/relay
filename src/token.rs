use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccessToken {
    pub access_token: String,
    pub expires_in: i32,
    pub refresh_token: String,
    pub token_type: String,

    #[serde(default = "utcnow")]
    pub ctime: i64,
}

impl AccessToken {
    pub fn obtained_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.ctime, 0).unwrap()
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.ctime + self.expires_in as i64, 0).unwrap()
    }

    pub fn expired(&self) -> bool {
        self.expires_at() >= Utc::now()
    }

    pub fn lifetime(&self) -> i64 {
        0.max(self.ctime + self.expires_in as i64 - Utc::now().timestamp())
    }
}

fn utcnow() -> i64 {
    Utc::now().timestamp()
}
