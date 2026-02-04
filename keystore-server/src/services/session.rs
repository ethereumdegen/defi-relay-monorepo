use chrono::{DateTime, Utc};
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct SessionService;

impl SessionService {
    /// Generate a new session token
    pub fn generate_token() -> String {
        let random_bytes: [u8; 32] = rand::thread_rng().gen();
        let mut hasher = Sha256::new();
        hasher.update(&random_bytes);
        hasher.update(Uuid::new_v4().as_bytes());
        let result = hasher.finalize();
        format!("ks_{}", hex::encode(&result[..24]))
    }

    /// Create a new session
    pub async fn create(
        pool: &PgPool,
        token: &str,
        wallet_id: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO sessions (token, wallet_id, expires_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(token)
        .bind(wallet_id)
        .bind(expires_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get wallet_id for a valid (non-expired) session token
    pub async fn get_wallet_id(pool: &PgPool, token: &str) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query(
            r#"
            SELECT wallet_id
            FROM sessions
            WHERE token = $1 AND expires_at > NOW()
            "#,
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;

        Ok(result.map(|row| row.get("wallet_id")))
    }

    /// Delete all expired sessions
    pub async fn delete_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM sessions WHERE expires_at < NOW()"#)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete a specific session (logout)
    pub async fn delete(pool: &PgPool, token: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM sessions WHERE token = $1"#)
            .bind(token)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
