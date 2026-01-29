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

/// Get Alchemy RPC URL for a network
/// Returns (network_name, alchemy_url) if the network is supported by Alchemy
fn alchemy_url_for_network(network: &str, api_key: &str) -> Option<(String, String)> {
    let (name, slug) = match network {
        "mainnet" => ("mainnet", "eth-mainnet"),
        "base" => ("base", "base-mainnet"),
        "arbitrum" => ("arbitrum", "arb-mainnet"),
        "optimism" => ("optimism", "opt-mainnet"),
        "polygon" => ("polygon", "polygon-mainnet"),
        "sepolia" => ("sepolia", "eth-sepolia"),
        "base-sepolia" => ("base-sepolia", "base-sepolia"),
        "arbitrum-sepolia" => ("arbitrum-sepolia", "arb-sepolia"),
        _ => return None,
    };

    let url = format!("https://{}.g.alchemy.com/v2/{}", slug, api_key);
    Some((name.to_string(), url))
}

/// Load network RPC endpoints using ALCHEMY_API_KEY.
/// Automatically registers all supported Alchemy networks.
pub fn load_networks_from_env() -> NetworkRegistry {
    dotenvy::dotenv().ok();

    let mut registry = NetworkRegistry::new();

    // Check for ALCHEMY_API_KEY first (preferred method)
    if let Ok(api_key) = env::var("ALCHEMY_API_KEY") {
        tracing::info!("Using ALCHEMY_API_KEY for RPC endpoints");

        let networks = [
            "mainnet",
            "base",
            "arbitrum",
            "optimism",
            "polygon",
            "sepolia",
            "base-sepolia",
            "arbitrum-sepolia",
        ];

        for network in networks {
            if let Some((name, url)) = alchemy_url_for_network(network, &api_key) {
                tracing::info!("Registered network: {} -> https://{}.g.alchemy.com/v2/***", name,
                    match network {
                        "mainnet" => "eth-mainnet",
                        "base" => "base-mainnet",
                        "arbitrum" => "arb-mainnet",
                        "optimism" => "opt-mainnet",
                        "polygon" => "polygon-mainnet",
                        "sepolia" => "eth-sepolia",
                        "base-sepolia" => "base-sepolia",
                        "arbitrum-sepolia" => "arb-sepolia",
                        _ => "unknown",
                    }
                );
                registry.register(&name, RpcClient::new(&url));
            }
        }
    } else {
        // Fallback: Look for individual *_RPC_URL environment variables
        tracing::warn!("ALCHEMY_API_KEY not set, falling back to individual *_RPC_URL variables");

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
    }

    registry
}
