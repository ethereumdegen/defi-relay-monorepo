use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub domain: String,
    pub session_ttl_secs: i64,
    pub challenge_ttl_secs: i64,
    pub max_encrypted_data_size: usize,
    pub allowed_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");

        let port = env::var("PORT")
            .or_else(|_| env::var("BACKEND_PORT"))
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .expect("PORT must be a valid number");

        let domain = env::var("KEYSTORE_DOMAIN")
            .unwrap_or_else(|_| "keystore.defirelay.com".to_string());

        let session_ttl_secs = env::var("SESSION_TTL_SECS")
            .unwrap_or_else(|_| "3600".to_string()) // 1 hour default
            .parse()
            .expect("SESSION_TTL_SECS must be a valid number");

        let challenge_ttl_secs = env::var("CHALLENGE_TTL_SECS")
            .unwrap_or_else(|_| "300".to_string()) // 5 minutes default
            .parse()
            .expect("CHALLENGE_TTL_SECS must be a valid number");

        let max_encrypted_data_size = env::var("MAX_ENCRYPTED_DATA_SIZE")
            .unwrap_or_else(|_| "1048576".to_string()) // 1MB default
            .parse()
            .expect("MAX_ENCRYPTED_DATA_SIZE must be a valid number");

        let allowed_origins = env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "https://stark.defirelay.com,http://localhost:5173".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        Config {
            database_url,
            port,
            domain,
            session_ttl_secs,
            challenge_ttl_secs,
            max_encrypted_data_size,
            allowed_origins,
        }
    }
}
