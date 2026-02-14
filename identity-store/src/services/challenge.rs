use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::models::Challenge;

pub struct ChallengeService;

impl ChallengeService {
    /// Create a new challenge, replacing any existing challenges for this wallet (atomic upsert)
    pub async fn create(
        pool: &PgPool,
        wallet_id: &str,
        nonce: &str,
        message: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO challenges (wallet_id, nonce, message, expires_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (wallet_id) DO UPDATE SET
                nonce = EXCLUDED.nonce,
                message = EXCLUDED.message,
                expires_at = EXCLUDED.expires_at,
                created_at = NOW()
            "#,
        )
        .bind(wallet_id)
        .bind(nonce)
        .bind(message)
        .bind(expires_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get the most recent pending (non-expired) challenge for a wallet
    pub async fn get_pending(pool: &PgPool, wallet_id: &str) -> Result<Option<Challenge>, sqlx::Error> {
        sqlx::query_as::<_, Challenge>(
            r#"
            SELECT id, wallet_id, nonce, message, expires_at, created_at
            FROM challenges
            WHERE wallet_id = $1 AND expires_at > NOW()
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(wallet_id)
        .fetch_optional(pool)
        .await
    }

    /// Delete all challenges for a wallet (after successful verification)
    pub async fn delete_for_wallet(pool: &PgPool, wallet_id: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM challenges WHERE wallet_id = $1"#)
            .bind(wallet_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete all expired challenges
    pub async fn delete_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(r#"DELETE FROM challenges WHERE expires_at < NOW()"#)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}
