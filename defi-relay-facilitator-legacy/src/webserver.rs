use std::sync::Arc;

use actix_cors::Cors;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use dotenvy::dotenv;
use tokio::io;

use defi_relay_facilitator::app_state::{AppConfig, AppState};
use defi_relay_facilitator::config::{load_env_var, usdc_address};
use defi_relay_facilitator::controllers::facilitator_controller::FacilitatorController;
use defi_relay_facilitator::controllers::web_controller::WebController;
use defi_relay_facilitator::services::eip712_verifier::Eip712Verifier;
use defi_relay_facilitator::services::settlement_service::SettlementService;

#[tokio::main]
async fn main() -> io::Result<()> {
    dotenv().ok();
    env_logger::init();

    log::info!("Starting x402 Facilitator for Base mainnet");

    // Load configuration from environment
    let rpc_url = load_env_var("BASE_RPC_URL").expect("BASE_RPC_URL must be set");
    let private_key = load_env_var("FACILITATOR_PRIVATE_KEY").expect("FACILITATOR_PRIVATE_KEY must be set");
    let bind_address = load_env_var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    // Initialize services
    let settlement_service = SettlementService::new(&rpc_url, &private_key)
        .expect("Failed to create settlement service");

    let facilitator_address = settlement_service.facilitator_address();
    log::info!("Facilitator address: {:?}", facilitator_address);

    let eip712_verifier = Eip712Verifier::new(usdc_address());
    log::info!("EIP-712 verifier initialized for USDC at {:?}", usdc_address());

    let app_config = Arc::new(AppConfig {
        facilitator_address,
        bind_address: bind_address.clone(),
    });

    let eip712_verifier = Arc::new(eip712_verifier);
    let settlement_service = Arc::new(settlement_service);

    log::info!("Starting HTTP server on {}", bind_address);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec!["Authorization", "Accept", "Content-Type"])
            .max_age(3600);

        let app_state = AppState {
            config: Arc::clone(&app_config),
            eip712_verifier: Arc::clone(&eip712_verifier),
            settlement_service: Arc::clone(&settlement_service),
        };

        App::new()
            .app_data(Data::new(app_state))
            .wrap(cors)
            .wrap(actix_web::middleware::Logger::default())
            .configure(FacilitatorController::config)
    })
    .bind(&bind_address)?
    .run()
    .await
}
