use serde::Deserialize;
use std::path::Path;

/// ENV-based configuration
pub struct Config {
    pub port: u16,
    pub base_rpc_url: String,
    pub moonshot_api_key: Option<String>,
    pub minimax_api_key: Option<String>,
    pub minimax_group_id: Option<String>,
    pub openai_api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            base_rpc_url: std::env::var("BASE_RPC_URL")
                .unwrap_or_else(|_| "https://mainnet.base.org".to_string()),
            moonshot_api_key: std::env::var("MOONSHOT_API_KEY").ok().filter(|s| !s.is_empty()),
            minimax_api_key: std::env::var("MINIMAX_API_KEY").ok().filter(|s| !s.is_empty()),
            minimax_group_id: std::env::var("MINIMAX_GROUP_ID").ok().filter(|s| !s.is_empty()),
            openai_api_key: std::env::var("OPENAI_API_KEY").ok().filter(|s| !s.is_empty()),
        }
    }
}

/// Token type to monitor for each wallet
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    ETH,
    USDC,
}

impl Token {
    pub fn symbol(self) -> &'static str {
        match self {
            Token::ETH => "ETH",
            Token::USDC => "USDC",
        }
    }

    pub fn decimals(self) -> u32 {
        match self {
            Token::ETH => 18,
            Token::USDC => 6,
        }
    }
}

/// RON-based wallet configuration
#[derive(Debug, Deserialize)]
pub struct MonitorConfig {
    pub wallets: Vec<WalletEntry>,
}

#[derive(Debug, Deserialize)]
pub struct WalletEntry {
    pub name: String,
    pub address: String,
    pub token: Token,
}

impl MonitorConfig {
    pub fn load(path: &Path) -> Result<Self, MonitorConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: MonitorConfig = ron::from_str(&contents)?;
        Ok(config)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MonitorConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse RON config: {0}")]
    Ron(#[from] ron::error::SpannedError),
}
