use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{web, App, HttpServer};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod db;
mod error;
mod handlers;
mod middleware;
mod models;
mod services;

use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub config: Arc<Config>,
    pub http_client: reqwest::Client,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "keystore_server=debug,actix_web=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::from_env();
    let port = config.port;

    // Create database pool
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    tracing::info!("Connected to database");

    // Create HTTP client for x402 facilitator communication
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    // Log x402 payment status
    if config.x402.is_some() {
        tracing::info!("x402 payment enabled for store_keys endpoint");
    } else {
        tracing::info!("x402 payment disabled (free storage)");
    }

    // Create app state
    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config.clone()),
        http_client,
    };

    // Rate limiting config (per IP)
    let governor_conf = GovernorConfigBuilder::default()
        .per_second(6)
        .burst_size(10)
        .finish()
        .unwrap();

    // Setup shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Spawn cleanup worker
    let cleanup_pool = pool.clone();
    let cleanup_shutdown = shutdown_tx.subscribe();
    tokio::spawn(async move {
        services::cleanup::run_cleanup_worker(cleanup_pool, cleanup_shutdown).await;
    });

    // Setup CORS with configured origins
    let allowed_origins = config.allowed_origins.clone();

    let shutdown_tx_clone = shutdown_tx.clone();

    let server = HttpServer::new(move || {
        let cors = if allowed_origins.iter().any(|o| o == "*") {
            Cors::permissive()
        } else {
            let origins = allowed_origins.clone();
            Cors::default()
                .allowed_origin_fn(move |origin, _req_head| {
                    origins
                        .iter()
                        .any(|o| o == origin.to_str().unwrap_or(""))
                })
                .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                .allow_any_header()
                .max_age(3600)
        };

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(state.clone()))
            // Request body limit: 21MB (encrypted data max is 10MB hex = 20MB chars + JSON overhead)
            .app_data(web::PayloadConfig::default().limit(21 * 1024 * 1024))
            .app_data(web::JsonConfig::default().limit(21 * 1024 * 1024))
            // Health check (no rate limit)
            .route(
                "/api/health",
                web::get().to(handlers::health::health_check),
            )
            // Rate-limited API routes
            .service(
                web::scope("/api")
                    .wrap(Governor::new(&governor_conf))
                    .route("/authorize", web::post().to(handlers::auth::authorize))
                    .route(
                        "/authorize/verify",
                        web::post().to(handlers::auth::verify),
                    )
                    .route("/store_keys", web::post().to(handlers::keys::store_keys))
                    .route(
                        "/delete_keys",
                        web::post().to(handlers::keys::delete_keys),
                    )
                    .route("/logout", web::post().to(handlers::keys::logout))
                    .route("/get_keys", web::post().to(handlers::keys::get_keys)),
            )
    })
    .bind(("0.0.0.0", port))?
    .run();

    tracing::info!("Keystore server listening on port {}", port);

    let result = server.await;

    // Signal workers to shut down
    tracing::info!("Shutdown signal received, starting graceful shutdown");
    let _ = shutdown_tx_clone.send(());

    // Wait for workers to finish
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    tracing::info!("Shutdown complete");

    result?;
    Ok(())
}
