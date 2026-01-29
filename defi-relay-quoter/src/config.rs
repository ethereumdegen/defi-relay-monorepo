use crate::models::{DomainEthAddress, DomainUint256};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub wallet_address: DomainEthAddress,
    pub facilitator_url: String,
    pub port: u16,
    pub cost_per_price: DomainUint256,
    pub cost_per_quote: DomainUint256,
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

        // Default to 500 raw USDC for price (indicative)
        let cost_per_price =
            env::var("COST_PER_PRICE").unwrap_or_else(|_| "500".to_string());
        let cost_per_price = DomainUint256::from_str(&cost_per_price)
            .map_err(|e| format!("Invalid COST_PER_PRICE: {}", e))?;

        // Default to 1000 raw USDC for quote (full quote)
        let cost_per_quote =
            env::var("COST_PER_QUOTE").unwrap_or_else(|_| "1000".to_string());
        let cost_per_quote = DomainUint256::from_str(&cost_per_quote)
            .map_err(|e| format!("Invalid COST_PER_QUOTE: {}", e))?;

        let base_url = env::var("BASE_URL").ok();

        let zerox_api_key =
            env::var("ZEROX_API_KEY").map_err(|_| "ZEROX_API_KEY is required")?;

        let zerox_base_url = env::var("ZEROX_BASE_URL")
            .unwrap_or_else(|_| "https://api.0x.org".to_string());

        Ok(Config {
            wallet_address,
            facilitator_url,
            port,
            cost_per_price,
            cost_per_quote,
            base_url,
            zerox_api_key,
            zerox_base_url,
        })
    }
}
