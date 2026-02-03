use sqlx::PgPool;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

use super::{challenge::ChallengeService, session::SessionService};

/// Background worker that cleans up expired sessions and challenges
pub async fn run_cleanup_worker(pool: PgPool, mut shutdown: broadcast::Receiver<()>) {
    let mut interval = interval(Duration::from_secs(60)); // Run every minute

    tracing::info!("Cleanup worker started");

    loop {
        tokio::select! {
            biased;

            _ = shutdown.recv() => {
                tracing::info!("Cleanup worker shutting down");
                break;
            }

            _ = interval.tick() => {
                match cleanup(&pool).await {
                    Ok((sessions, challenges)) => {
                        if sessions > 0 || challenges > 0 {
                            tracing::debug!(
                                "Cleaned up {} expired sessions and {} expired challenges",
                                sessions,
                                challenges
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!("Cleanup worker error: {}", e);
                    }
                }
            }
        }
    }
}

async fn cleanup(pool: &PgPool) -> Result<(u64, u64), sqlx::Error> {
    let sessions = SessionService::delete_expired(pool).await?;
    let challenges = ChallengeService::delete_expired(pool).await?;
    Ok((sessions, challenges))
}
