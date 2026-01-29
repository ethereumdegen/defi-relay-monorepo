use std::env;

use crate::models::{DomainEthAddress, DomainUint256};

#[derive(Clone, Debug)]
pub struct Config {
    pub moonshot_endpoint: String,
    pub moonshot_api_key: String,
    pub bot_wallet_address: DomainEthAddress,
    pub facilitator_url: String,
    pub port: u16,
    pub cost_per_request: DomainUint256,
    pub base_url: Option<String>,
    pub max_input_tokens: u32,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();

        let moonshot_endpoint = env::var("MOONSHOT_ENDPOINT")
            .unwrap_or_else(|_| "https://api.moonshot.ai/v1/chat/completions".to_string());

        let moonshot_api_key = env::var("MOONSHOT_API_KEY")
            .map_err(|_| "MOONSHOT_API_KEY is required")?;

        let bot_wallet_address = env::var("BOT_WALLET_ADDRESS")
            .map_err(|_| "BOT_WALLET_ADDRESS is required")?;
        let bot_wallet_address = DomainEthAddress::from_hex(&bot_wallet_address)
            .map_err(|e| format!("Invalid BOT_WALLET_ADDRESS: {}", e))?;

        let facilitator_url = env::var("FACILITATOR_URL")
            .map_err(|_| "FACILITATOR_URL is required")?;

        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .map_err(|_| "PORT must be a valid port number")?;

        let cost_per_request = env::var("COST_PER_REQUEST")
            .unwrap_or_else(|_| "1000".to_string());
        let cost_per_request = DomainUint256::from_str(&cost_per_request)
            .map_err(|e| format!("Invalid COST_PER_REQUEST: {}", e))?;

        let base_url = env::var("BASE_URL").ok();

        let max_input_tokens = env::var("MAX_INPUT_TOKENS")
            .unwrap_or_else(|_| "50000".to_string())
            .parse::<u32>()
            .map_err(|_| "MAX_INPUT_TOKENS must be a valid number")?;

        Ok(Config {
            moonshot_endpoint,
            moonshot_api_key,
            bot_wallet_address,
            facilitator_url,
            port,
            cost_per_request,
            base_url,
            max_input_tokens,
        })
    }
}
