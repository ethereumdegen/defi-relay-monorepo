use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Challenge {
    pub id: i32,
    pub wallet_id: String,
    pub nonce: String,
    pub message: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
