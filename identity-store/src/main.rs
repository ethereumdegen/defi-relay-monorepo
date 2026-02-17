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
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "identity_store=debug,actix_web=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let port = config.port;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    tracing::info!("Connected to database");

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    if config.x402.is_some() {
        tracing::info!("x402 payment enabled for store_identity endpoint");
    } else {
        tracing::info!("x402 payment disabled (free storage)");
    }

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

    // Shutdown channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Spawn cleanup worker
    let cleanup_pool = pool.clone();
    let cleanup_shutdown = shutdown_tx.subscribe();
    tokio::spawn(async move {
        services::cleanup::run_cleanup_worker(cleanup_pool, cleanup_shutdown).await;
    });

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
            // Request body limit: 1MB (identity JSON max is 256KB + JSON overhead)
            .app_data(web::PayloadConfig::default().limit(1024 * 1024))
            .app_data(web::JsonConfig::default().limit(1024 * 1024))
            // Health check (no rate limit)
            .route(
                "/api/health",
                web::get().to(handlers::health::health_check),
            )
            // Public read routes (no auth required, rate limited)
            .service(
                web::resource("/api/identity/{hash}")
                    .wrap(Governor::new(&governor_conf))
                    .route(web::get().to(handlers::identity::get_identity_by_hash)),
            )
            .service(
                web::resource("/api/identity/{hash}/raw")
                    .wrap(Governor::new(&governor_conf))
                    .route(web::get().to(handlers::identity::get_identity_raw)),
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
                    .route(
                        "/store_identity",
                        web::post().to(handlers::identity::store_identity),
                    )
                    .route(
                        "/get_identity",
                        web::post().to(handlers::identity::get_identity),
                    )
                    .route(
                        "/delete_identity",
                        web::post().to(handlers::identity::delete_identity),
                    )
                    .route("/logout", web::post().to(handlers::identity::logout)),
            )
    })
    .bind(("0.0.0.0", port))?
    .run();

    tracing::info!("Identity store listening on port {}", port);

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
