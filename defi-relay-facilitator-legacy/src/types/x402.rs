use serde::{Deserialize, Serialize};

use super::domains::{DomainBytes32, DomainEthAddress, DomainUint256};

/// EIP-3009 authorization payload
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

/// Full x402 payment payload including signature
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: u8,
    pub scheme: String,
    pub network: String,
    pub payload: Eip3009Payload,
    pub signature: String,
}

/// Payment requirements from the resource server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub x402_version: u8,
    pub scheme: String,
    pub network: String,
    pub max_amount_required: DomainUint256,
    pub resource: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub pay_to_address: DomainEthAddress,
    pub max_timeout_seconds: u64,
    pub asset: DomainEthAddress,
    #[serde(default)]
    pub extra: Option<serde_json::Value>,
}

/// Supported payment kind
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportedKind {
    pub x402_version: u8,
    pub scheme: String,
    pub network: String,
}

/// Response for /supported endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedResponse {
    pub kinds: Vec<SupportedKind>,
}

/// Request body for /verify endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyRequest {
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

/// Response for /verify endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResponse {
    pub is_valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer: Option<DomainEthAddress>,
}

/// Request body for /settle endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

/// Response for /settle endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
