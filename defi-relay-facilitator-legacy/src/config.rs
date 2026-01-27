use ethers::types::Address;

// Base mainnet chain ID
pub const BASE_CHAIN_ID: u64 = 8453;

// Base mainnet network identifier
pub const BASE_NETWORK: &str = "eip155:8453";

// USDC contract address on Base mainnet
pub const USDC_ADDRESS: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

// x402 protocol version
pub const X402_VERSION: u8 = 2;

// Payment scheme
pub const PAYMENT_SCHEME: &str = "exact";

// USDC token name for EIP-712 domain
pub const USDC_NAME: &str = "USD Coin";

// USDC token version for EIP-712 domain
pub const USDC_VERSION: &str = "2";

pub fn usdc_address() -> Address {
    USDC_ADDRESS.parse().expect("Invalid USDC address")
}

pub fn load_env_var(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| format!("{} must be set in environment", name))
}
