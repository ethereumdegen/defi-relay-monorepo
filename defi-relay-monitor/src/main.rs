mod config;
mod services;

use actix_web::{web, App, HttpResponse, HttpServer};
use config::{Config, MonitorConfig};
use std::path::PathBuf;
use std::sync::Arc;

struct AppState {
    config: Config,
    monitor_config: MonitorConfig,
    client: reqwest::Client,
}

async fn index(state: web::Data<Arc<AppState>>) -> HttpResponse {
    let mut output = String::new();
    output.push_str("DeFi Relay Monitor\n");
    output.push_str("==================\n\n");

    // Fetch wallet balances and API balances concurrently
    let (wallet_balances, api_balances) = tokio::join!(
        services::eth_balance::fetch_wallet_balances(
            &state.client,
            &state.config.base_rpc_url,
            &state.monitor_config.wallets,
        ),
        services::api_balance::fetch_api_balances(&state.client, &state.config),
    );

    // Wallet Balances section
    output.push_str("Wallet Balances (Base Network)\n");
    output.push_str("---------------------------\n");
    for b in &wallet_balances {
        let addr_display = abbreviate_address(&b.address);
        match &b.balance {
            Ok(balance) => {
                output.push_str(&format!(
                    "{:<18}{:<16}{:.6} {}\n",
                    b.name, addr_display, balance, b.token.symbol()
                ));
            }
            Err(e) => {
                output.push_str(&format!(
                    "{:<18}{:<16}ERROR - {}\n",
                    b.name, addr_display, e
                ));
            }
        }
    }

    // API Balances section
    output.push_str("\nAPI Balances\n");
    output.push_str("---------------------------\n");
    for b in &api_balances {
        match &b.result {
            Ok(balance_str) => {
                output.push_str(&format!("{:<16}{}\n", b.name, balance_str));
            }
            Err(e) => {
                output.push_str(&format!("{:<16}ERROR - {}\n", b.name, e));
            }
        }
    }

    if api_balances.is_empty() {
        output.push_str("(no API keys configured)\n");
    }

    // Timestamp
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    output.push_str(&format!("\nLast refreshed: {now}\n"));

    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(output)
}

async fn health() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(r#"{"status":"ok"}"#)
}

fn abbreviate_address(addr: &str) -> String {
    if addr.len() > 12 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env();
    let port = config.port;

    // Load RON config
    let config_path = PathBuf::from("config.ron");
    let monitor_config = MonitorConfig::load(&config_path).unwrap_or_else(|e| {
        tracing::warn!("Failed to load config.ron: {e}, using empty wallet list");
        MonitorConfig {
            wallets: Vec::new(),
        }
    });

    tracing::info!(
        "Loaded {} wallet(s) from config.ron",
        monitor_config.wallets.len()
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to build HTTP client");

    let state = Arc::new(AppState {
        config,
        monitor_config,
        client,
    });

    tracing::info!("Starting defi-relay-monitor on port {port}");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
