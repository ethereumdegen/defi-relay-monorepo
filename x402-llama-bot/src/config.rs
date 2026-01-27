use std::env;

use crate::models::{DomainEthAddress, DomainUint256};

#[derive(Clone, Debug)]
pub struct Config {
    pub do_agent_endpoint: String,
    pub do_agent_secret: String,
    pub bot_wallet_address: DomainEthAddress,
    pub facilitator_url: String,
    pub port: u16,
    pub cost_per_request: DomainUint256,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();

        let do_agent_endpoint = env::var("DO_AGENT_ENDPOINT")
            .map_err(|_| "DO_AGENT_ENDPOINT is required")?;

        let do_agent_secret = env::var("DO_AGENT_SECRET")
            .map_err(|_| "DO_AGENT_SECRET is required")?;

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

        Ok(Config {
            do_agent_endpoint,
            do_agent_secret,
            bot_wallet_address,
            facilitator_url,
            port,
            cost_per_request,
        })
    }
}
