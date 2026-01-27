use serde::{Deserialize, Serialize};

use super::domains::{DomainBytes32, DomainEthAddress, DomainUint256};

/// USDC contract address on Base mainnet
pub const USDC_BASE_ADDRESS: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

/// Base mainnet network identifier (x402 v2 format)
pub const BASE_NETWORK: &str = "eip155:8453";

/// x402 protocol version
pub const X402_VERSION: u8 = 2;

/// Get USDC address as DomainEthAddress
pub fn usdc_address() -> DomainEthAddress {
    DomainEthAddress::from_hex(USDC_BASE_ADDRESS).expect("Invalid USDC address constant")
}

/// Payment requirement returned in 402 response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequired {
    pub x402_version: u8,
    pub accepts: Vec<PaymentRequirements>,
}

/// Payment requirements (x402 v2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub x402_version: u8,
    pub scheme: String,
    pub network: String,
    pub max_amount_required: DomainUint256,
    pub resource: String,
    pub description: String,
    pub pay_to_address: DomainEthAddress,
    pub asset: DomainEthAddress,
    pub max_timeout_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

impl PaymentRequired {
    pub fn new(pay_to: DomainEthAddress, amount: DomainUint256, resource: &str) -> Self {
        PaymentRequired {
            x402_version: X402_VERSION,
            accepts: vec![PaymentRequirements {
                x402_version: X402_VERSION,
                scheme: "exact".to_string(),
                network: BASE_NETWORK.to_string(),
                max_amount_required: amount,
                resource: resource.to_string(),
                description: "Chat with Llama agent".to_string(),
                pay_to_address: pay_to,
                asset: usdc_address(),
                max_timeout_seconds: 60,
                mime_type: None,
                extra: None,
            }],
        }
    }

    pub fn to_base64(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            json.as_bytes(),
        ))
    }
}

/// EIP-3009 authorization payload (x402 v2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Eip3009Payload {
    pub from: DomainEthAddress,
    pub to: DomainEthAddress,
    pub value: DomainUint256,
    pub valid_after: DomainUint256,
    pub valid_before: DomainUint256,
    pub nonce: DomainBytes32,
}

/// Payment payload sent by client in X-PAYMENT header (x402 v2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: u8,
    pub scheme: String,
    pub network: String,
    pub payload: Eip3009Payload,
    pub signature: String,
}

impl PaymentPayload {
    pub fn from_base64(encoded: &str) -> Result<Self, crate::error::AppError> {
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD.decode(encoded)?;
        let json_str = String::from_utf8(decoded)
            .map_err(|e| crate::error::AppError::InvalidPayment(e.to_string()))?;
        let payload: PaymentPayload = serde_json::from_str(&json_str)?;
        Ok(payload)
    }
}

/// Request to facilitator /verify endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyRequest {
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

/// Response from facilitator /verify endpoint (x402 v2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResponse {
    pub is_valid: bool,
    #[serde(default)]
    pub payer: Option<DomainEthAddress>,
    #[serde(default)]
    pub invalid_reason: Option<String>,
}

/// Payment response header content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentResponse {
    pub x402_version: u8,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl PaymentResponse {
    pub fn success() -> Self {
        PaymentResponse {
            x402_version: X402_VERSION,
            success: true,
            error: None,
        }
    }

    pub fn failure(error: &str) -> Self {
        PaymentResponse {
            x402_version: X402_VERSION,
            success: false,
            error: Some(error.to_string()),
        }
    }

    pub fn to_base64(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            json.as_bytes(),
        ))
    }
}
