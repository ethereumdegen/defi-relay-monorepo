use axum::{routing::get, routing::post, Router};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_governor::{governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod db;
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
                .unwrap_or_else(|_| "keystore_server=debug,tower_http=debug".into()),
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

    // Run migrations at runtime
    let migrator = Migrator::new(Path::new("./migrations")).await?;
    migrator.run(&pool).await?;
    tracing::info!("Migrations completed");

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

    // Setup CORS with configured origins
    let cors = if config.allowed_origins.iter().any(|o| o == "*") {
        CorsLayer::permissive()
    } else {
        let origins: Vec<_> = config
            .allowed_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any)
    };

    // Rate limiting configs (per IP) - use SmartIpKeyExtractor for proxy support
    // Auth: 10 requests per minute
    let auth_governor = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor)
        .per_second(6) // ~10 per minute = 1 every 6 seconds burst
        .burst_size(10)
        .finish()
        .unwrap();

    // Write: 10 requests per minute
    let write_governor = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor)
        .per_second(6)
        .burst_size(10)
        .finish()
        .unwrap();

    // Read: 30 requests per minute
    let read_governor = GovernorConfigBuilder::default()
        .key_extractor(SmartIpKeyExtractor)
        .per_second(2) // ~30 per minute
        .burst_size(30)
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

    // Build router with rate limiting per route group
    let auth_routes = Router::new()
        .route("/api/authorize", post(handlers::auth::authorize))
        .route("/api/authorize/verify", post(handlers::auth::verify))
        .layer(GovernorLayer {
            config: Arc::new(auth_governor),
        });

    let write_routes = Router::new()
        .route("/api/store_keys", post(handlers::keys::store_keys))
        .route("/api/delete_keys", post(handlers::keys::delete_keys))
        .route("/api/logout", post(handlers::keys::logout))
        .layer(GovernorLayer {
            config: Arc::new(write_governor),
        });

    let read_routes = Router::new()
        .route("/api/get_keys", post(handlers::keys::get_keys))
        .layer(GovernorLayer {
            config: Arc::new(read_governor),
        });

    let app = Router::new()
        // Health check (no rate limit)
        .route("/api/health", get(handlers::health::health_check))
        // Merge rate-limited routes
        .merge(auth_routes)
        .merge(write_routes)
        .merge(read_routes)
        .with_state(state)
        // Request body limit: 21MB (encrypted data max is 10MB hex = 20MB chars + JSON overhead)
        .layer(RequestBodyLimitLayer::new(21 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    tracing::info!("Keystore server listening on port {}", port);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown_tx))
        .await?;

    Ok(())
}

async fn shutdown_signal(shutdown_tx: broadcast::Sender<()>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown");
    let _ = shutdown_tx.send(());

    // Wait for workers to finish
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    tracing::info!("Shutdown complete");
}
