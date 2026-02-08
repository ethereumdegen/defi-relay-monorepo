use std::env;

/// x402 payment configuration (optional)
#[derive(Debug, Clone)]
pub struct X402Config {
    pub facilitator_url: String,
    pub facilitator_signer: String,
    pub wallet_address: String,
    pub cost_per_upload: String,
    pub payment_network: String,
    pub payment_token_address: String,
    pub payment_token_symbol: String,
    pub payment_token_decimals: u8,
    pub payment_token_name: String,
    pub payment_token_version: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub domain: String,
    pub public_url: String,
    pub session_ttl_secs: i64,
    pub challenge_ttl_secs: i64,
    pub max_identity_json_size: usize,
    pub allowed_origins: Vec<String>,
    /// x402 payment config - None means storage is free
    pub x402: Option<X402Config>,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");

        let port = env::var("PORT")
            .or_else(|_| env::var("BACKEND_PORT"))
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .expect("PORT must be a valid number");

        let domain = env::var("IDENTITY_DOMAIN")
            .unwrap_or_else(|_| "identity.defirelay.com".to_string());

        let public_url = env::var("PUBLIC_URL")
            .unwrap_or_else(|_| format!("https://{}", domain));

        let session_ttl_secs = env::var("SESSION_TTL_SECS")
            .unwrap_or_else(|_| "3600".to_string())
            .parse()
            .expect("SESSION_TTL_SECS must be a valid number");

        let challenge_ttl_secs = env::var("CHALLENGE_TTL_SECS")
            .unwrap_or_else(|_| "300".to_string())
            .parse()
            .expect("CHALLENGE_TTL_SECS must be a valid number");

        let max_identity_json_size = env::var("MAX_IDENTITY_JSON_SIZE")
            .unwrap_or_else(|_| "262144".to_string()) // 256KB default
            .parse()
            .expect("MAX_IDENTITY_JSON_SIZE must be a valid number");

        let allowed_origins = env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "https://stark.defirelay.com,http://localhost:5173".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        // x402 config is optional - only enabled if X402_WALLET_ADDRESS is set
        let x402 = env::var("X402_WALLET_ADDRESS").ok().map(|wallet_address| {
            X402Config {
                facilitator_url: env::var("X402_FACILITATOR_URL")
                    .unwrap_or_else(|_| "https://pay2.defirelay.com".to_string()),
                facilitator_signer: env::var("X402_FACILITATOR_SIGNER")
                    .expect("X402_FACILITATOR_SIGNER must be set when X402_WALLET_ADDRESS is set"),
                wallet_address,
                cost_per_upload: env::var("X402_COST_PER_UPLOAD")
                    .unwrap_or_else(|_| "1000000000000000000000".to_string()), // 1000 tokens with 18 decimals
                payment_network: env::var("X402_PAYMENT_NETWORK")
                    .unwrap_or_else(|_| "base-sepolia".to_string()),
                payment_token_address: env::var("X402_PAYMENT_TOKEN_ADDRESS")
                    .expect("X402_PAYMENT_TOKEN_ADDRESS must be set when X402_WALLET_ADDRESS is set"),
                payment_token_symbol: env::var("X402_PAYMENT_TOKEN_SYMBOL")
                    .unwrap_or_else(|_| "STARKBOT".to_string()),
                payment_token_decimals: env::var("X402_PAYMENT_TOKEN_DECIMALS")
                    .unwrap_or_else(|_| "18".to_string())
                    .parse()
                    .expect("X402_PAYMENT_TOKEN_DECIMALS must be a valid number"),
                payment_token_name: env::var("X402_PAYMENT_TOKEN_NAME")
                    .unwrap_or_else(|_| "StarkBot".to_string()),
                payment_token_version: env::var("X402_PAYMENT_TOKEN_VERSION")
                    .unwrap_or_else(|_| "1".to_string()),
            }
        });

        Config {
            database_url,
            port,
            domain,
            public_url,
            session_ttl_secs,
            challenge_ttl_secs,
            max_identity_json_size,
            allowed_origins,
            x402,
        }
    }
}
