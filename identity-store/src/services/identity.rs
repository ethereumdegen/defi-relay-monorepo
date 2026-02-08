use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::models::Identity;

pub struct IdentityService;

impl IdentityService {
    /// Compute SHA256 content hash of identity JSON
    pub fn content_hash(identity_json: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(identity_json.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Get identity by wallet address (most recent)
    pub async fn get_by_wallet(pool: &PgPool, wallet_id: &str) -> Result<Option<Identity>, sqlx::Error> {
        sqlx::query_as::<_, Identity>(
            r#"
            SELECT id, wallet_id, identity_json, content_hash, created_at, updated_at,
                   last_payment_tx, last_payment_at
            FROM identities
            WHERE wallet_id = $1
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .bind(wallet_id)
        .fetch_optional(pool)
        .await
    }

    /// Get identity by content hash (public, no auth needed)
    pub async fn get_by_hash(pool: &PgPool, content_hash: &str) -> Result<Option<Identity>, sqlx::Error> {
        sqlx::query_as::<_, Identity>(
            r#"
            SELECT id, wallet_id, identity_json, content_hash, created_at, updated_at,
                   last_payment_tx, last_payment_at
            FROM identities
            WHERE content_hash = $1
            "#,
        )
        .bind(content_hash)
        .fetch_optional(pool)
        .await
    }

    /// Insert or update identity for wallet (upsert by wallet_id, updates content_hash)
    pub async fn upsert(
        pool: &PgPool,
        wallet_id: &str,
        identity_json: &str,
        payment_tx: Option<&str>,
    ) -> Result<Identity, sqlx::Error> {
        let content_hash = Self::content_hash(identity_json);

        sqlx::query_as::<_, Identity>(
            r#"
            INSERT INTO identities (wallet_id, identity_json, content_hash, updated_at,
                                    last_payment_tx, last_payment_at)
            VALUES ($1, $2, $3, NOW(), $4, CASE WHEN $4 IS NOT NULL THEN NOW() ELSE NULL END)
            ON CONFLICT (wallet_id)
            DO UPDATE SET
                identity_json = EXCLUDED.identity_json,
                content_hash = EXCLUDED.content_hash,
                updated_at = NOW(),
                last_payment_tx = COALESCE(EXCLUDED.last_payment_tx, identities.last_payment_tx),
                last_payment_at = CASE WHEN EXCLUDED.last_payment_tx IS NOT NULL THEN NOW() ELSE identities.last_payment_at END
            RETURNING id, wallet_id, identity_json, content_hash, created_at, updated_at,
                      last_payment_tx, last_payment_at
            "#,
        )
        .bind(wallet_id)
        .bind(identity_json)
        .bind(&content_hash)
        .bind(payment_tx)
        .fetch_one(pool)
        .await
    }

    /// Delete identity for wallet
    pub async fn delete(pool: &PgPool, wallet_id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM identities
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
