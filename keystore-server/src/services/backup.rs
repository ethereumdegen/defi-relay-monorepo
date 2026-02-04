use sqlx::PgPool;

use crate::models::Backup;

pub struct BackupService;

impl BackupService {
    /// Get backup by wallet address
    pub async fn get_by_wallet(pool: &PgPool, wallet_id: &str) -> Result<Option<Backup>, sqlx::Error> {
        sqlx::query_as::<_, Backup>(
            r#"
            SELECT id, wallet_id, encrypted_data, key_count, created_at, updated_at,
                   last_payment_tx, last_payment_at
            FROM backups
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .fetch_optional(pool)
        .await
    }

    /// Insert or update backup for wallet (with optional payment audit trail)
    pub async fn upsert(
        pool: &PgPool,
        wallet_id: &str,
        encrypted_data: &str,
        key_count: i32,
        payment_tx: Option<&str>,
    ) -> Result<Backup, sqlx::Error> {
        sqlx::query_as::<_, Backup>(
            r#"
            INSERT INTO backups (wallet_id, encrypted_data, key_count, updated_at, last_payment_tx, last_payment_at)
            VALUES ($1, $2, $3, NOW(), $4, CASE WHEN $4 IS NOT NULL THEN NOW() ELSE NULL END)
            ON CONFLICT (wallet_id)
            DO UPDATE SET
                encrypted_data = EXCLUDED.encrypted_data,
                key_count = EXCLUDED.key_count,
                updated_at = NOW(),
                last_payment_tx = COALESCE(EXCLUDED.last_payment_tx, backups.last_payment_tx),
                last_payment_at = CASE WHEN EXCLUDED.last_payment_tx IS NOT NULL THEN NOW() ELSE backups.last_payment_at END
            RETURNING id, wallet_id, encrypted_data, key_count, created_at, updated_at,
                      last_payment_tx, last_payment_at
            "#,
        )
        .bind(wallet_id)
        .bind(encrypted_data)
        .bind(key_count)
        .bind(payment_tx)
        .fetch_one(pool)
        .await
    }

    /// Delete backup for wallet
    /// Returns true if a backup was deleted, false if no backup existed
    pub async fn delete(pool: &PgPool, wallet_id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM backups
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
