use thiserror::Error;

#[derive(Error, Debug)]
pub enum FacilitatorError {
    #[error("Invalid x402 version: expected {expected}, got {got}")]
    InvalidVersion { expected: u8, got: u8 },

    #[error("Invalid scheme: expected {expected}, got {got}")]
    InvalidScheme { expected: String, got: String },

    #[error("Invalid network: expected {expected}, got {got}")]
    InvalidNetwork { expected: String, got: String },

    #[error("Invalid token: expected {expected}, got {got}")]
    InvalidToken { expected: String, got: String },

    #[error("Invalid 'to' address: expected facilitator {expected}, got {got}")]
    InvalidToAddress { expected: String, got: String },

    #[error("Insufficient payment amount: required {required}, got {got}")]
    InsufficientAmount { required: String, got: String },

    #[error("Payment not yet valid: validAfter {valid_after} is in the future")]
    PaymentNotYetValid { valid_after: u64 },

    #[error("Payment expired: validBefore {valid_before} has passed")]
    PaymentExpired { valid_before: u64 },

    #[error("Invalid signature: {0}")]
    InvalidSignature(String),

    #[error("Nonce already used")]
    NonceAlreadyUsed,

    #[error("Settlement failed: {0}")]
    SettlementFailed(String),

    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Hex decode error: {0}")]
    HexDecodeError(String),

    #[error("Invalid payload: {0}")]
    InvalidPayload(String),
}

impl From<hex::FromHexError> for FacilitatorError {
    fn from(e: hex::FromHexError) -> Self {
        FacilitatorError::HexDecodeError(e.to_string())
    }
}
