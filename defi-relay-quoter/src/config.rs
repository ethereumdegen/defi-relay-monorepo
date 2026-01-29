use crate::models::{DomainEthAddress, DomainUint256};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub wallet_address: DomainEthAddress,
    pub facilitator_url: String,
    pub port: u16,
    pub cost_per_request: DomainUint256,
    #[allow(dead_code)] // Reserved for future use in discovery documents
    pub base_url: Option<String>,
    pub zerox_api_key: String,
    pub zerox_base_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();

        let wallet_address =
            env::var("WALLET_ADDRESS").map_err(|_| "WALLET_ADDRESS is required")?;
        let wallet_address = DomainEthAddress::from_hex(&wallet_address)
            .map_err(|e| format!("Invalid WALLET_ADDRESS: {}", e))?;

        let facilitator_url =
            env::var("FACILITATOR_URL").map_err(|_| "FACILITATOR_URL is required")?;

        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .map_err(|_| "PORT must be a valid port number")?;

        // Default to 1000 raw USDC per request
        let cost_per_request =
            env::var("COST_PER_REQUEST").unwrap_or_else(|_| "1000".to_string());
        let cost_per_request = DomainUint256::from_str(&cost_per_request)
            .map_err(|e| format!("Invalid COST_PER_REQUEST: {}", e))?;

        let base_url = env::var("BASE_URL").ok();

        let zerox_api_key =
            env::var("ZEROX_API_KEY").map_err(|_| "ZEROX_API_KEY is required")?;

        let zerox_base_url = env::var("ZEROX_BASE_URL")
            .unwrap_or_else(|_| "https://api.0x.org".to_string());

        Ok(Config {
            wallet_address,
            facilitator_url,
            port,
            cost_per_request,
            base_url,
            zerox_api_key,
            zerox_base_url,
        })
    }
}
