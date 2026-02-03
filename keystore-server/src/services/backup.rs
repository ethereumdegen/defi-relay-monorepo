use sqlx::PgPool;

use crate::models::Backup;

pub struct BackupService;

impl BackupService {
    /// Get backup by wallet address
    pub async fn get_by_wallet(pool: &PgPool, wallet_id: &str) -> Result<Option<Backup>, sqlx::Error> {
        sqlx::query_as::<_, Backup>(
            r#"
            SELECT id, wallet_id, encrypted_data, key_count, created_at, updated_at
            FROM backups
            WHERE wallet_id = $1
            "#,
        )
        .bind(wallet_id)
        .fetch_optional(pool)
        .await
    }

    /// Insert or update backup for wallet
    pub async fn upsert(
        pool: &PgPool,
        wallet_id: &str,
        encrypted_data: &str,
        key_count: i32,
    ) -> Result<Backup, sqlx::Error> {
        sqlx::query_as::<_, Backup>(
            r#"
            INSERT INTO backups (wallet_id, encrypted_data, key_count, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (wallet_id)
            DO UPDATE SET
                encrypted_data = EXCLUDED.encrypted_data,
                key_count = EXCLUDED.key_count,
                updated_at = NOW()
            RETURNING id, wallet_id, encrypted_data, key_count, created_at, updated_at
            "#,
        )
        .bind(wallet_id)
        .bind(encrypted_data)
        .bind(key_count)
        .fetch_one(pool)
        .await
    }
}
