use crate::models::{DomainEthAddress, DomainUint256};
use crate::services::RpcClient;
use std::collections::HashMap;
use std::env;

/// Registry of network names to RPC clients
#[derive(Clone)]
pub struct NetworkRegistry {
    networks: HashMap<String, RpcClient>,
}

impl NetworkRegistry {
    pub fn new() -> Self {
        NetworkRegistry {
            networks: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, client: RpcClient) {
        self.networks.insert(name.to_lowercase(), client);
    }

    pub fn get(&self, name: &str) -> Option<&RpcClient> {
        self.networks.get(&name.to_lowercase())
    }

    pub fn list_networks(&self) -> Vec<&String> {
        self.networks.keys().collect()
    }
}

impl Default for NetworkRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub wallet_address: DomainEthAddress,
    pub facilitator_url: String,
    pub port: u16,
    pub cost_per_request: DomainUint256,
    pub cost_per_heavy_request: DomainUint256,
    pub base_url: Option<String>,
}

/// Heavy RPC methods that require higher pricing
pub const HEAVY_METHODS: &[&str] = &[
    "eth_getLogs",
    "eth_getFilterLogs",
    "eth_newFilter",
    "eth_newBlockFilter",
    "eth_newPendingTransactionFilter",
    "debug_traceTransaction",
    "debug_traceCall",
    "debug_traceBlockByNumber",
    "debug_traceBlockByHash",
    "trace_block",
    "trace_transaction",
    "trace_call",
    "trace_rawTransaction",
    "trace_replayTransaction",
    "trace_replayBlockTransactions",
];

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

        let cost_per_request = env::var("COST_PER_REQUEST").unwrap_or_else(|_| "100".to_string());
        let cost_per_request = DomainUint256::from_str(&cost_per_request)
            .map_err(|e| format!("Invalid COST_PER_REQUEST: {}", e))?;

        let cost_per_heavy_request =
            env::var("COST_PER_HEAVY_REQUEST").unwrap_or_else(|_| "1000".to_string());
        let cost_per_heavy_request = DomainUint256::from_str(&cost_per_heavy_request)
            .map_err(|e| format!("Invalid COST_PER_HEAVY_REQUEST: {}", e))?;

        let base_url = env::var("BASE_URL").ok();

        Ok(Config {
            wallet_address,
            facilitator_url,
            port,
            cost_per_request,
            cost_per_heavy_request,
            base_url,
        })
    }
}

/// Load network RPC endpoints from environment variables.
/// Looks for variables like MAINNET_RPC_URL, BASE_RPC_URL, ARBITRUM_RPC_URL, etc.
pub fn load_networks_from_env() -> NetworkRegistry {
    dotenvy::dotenv().ok();

    let mut registry = NetworkRegistry::new();

    // Known network suffixes to look for
    let known_networks = [
        "MAINNET",
        "BASE",
        "ARBITRUM",
        "OPTIMISM",
        "POLYGON",
        "AVALANCHE",
        "BSC",
        "FANTOM",
        "GNOSIS",
        "SEPOLIA",
        "GOERLI",
        "HOLESKY",
        "BASE_SEPOLIA",
        "ARBITRUM_SEPOLIA",
    ];

    for network in known_networks {
        let env_var = format!("{}_RPC_URL", network);
        if let Ok(url) = env::var(&env_var) {
            let network_name = network.to_lowercase().replace('_', "-");
            tracing::info!("Registered network: {} -> {}", network_name, url);
            registry.register(&network_name, RpcClient::new(&url));
        }
    }

    // Also scan for any *_RPC_URL environment variables we might have missed
    for (key, value) in env::vars() {
        if key.ends_with("_RPC_URL") && !key.starts_with("_") {
            let network_name = key
                .trim_end_matches("_RPC_URL")
                .to_lowercase()
                .replace('_', "-");
            if registry.get(&network_name).is_none() {
                tracing::info!("Registered network: {} -> {}", network_name, value);
                registry.register(&network_name, RpcClient::new(&value));
            }
        }
    }

    registry
}
