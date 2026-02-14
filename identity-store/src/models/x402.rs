//! x402 Protocol V1 Types for payment verification and settlement

use serde::{Deserialize, Serialize};

/// 402 Payment Required response body
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequiredResponse {
    pub x402_version: u32,
    pub accepts: Vec<PaymentRequirements>,
    pub error: Option<String>,
}

/// Payment requirements (what the seller accepts)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub max_amount_required: String,
    pub resource: String,
    pub description: String,
    pub mime_type: String,
    pub pay_to: String,
    pub max_timeout_seconds: u64,
    pub asset: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

/// V1 Verify/Settle request - contains both payload and requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyRequest {
    pub x402_version: u32,
    pub payment_payload: serde_json::Value,
    pub payment_requirements: PaymentRequirements,
}

/// Alias for SettleRequest - same structure as VerifyRequest in V1
#[allow(dead_code)]
pub type SettleRequest = VerifyRequest;

/// V1 Verify response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResponse {
    pub is_valid: bool,
    #[serde(default)]
    pub invalid_reason: Option<String>,
    #[serde(default)]
    pub payer: Option<String>,
}

/// V1 Settle response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleResponse {
    pub success: bool,
    pub network: String,
    #[serde(default)]
    pub transaction: Option<String>,
    #[serde(default)]
    pub error_reason: Option<String>,
    #[serde(default)]
    pub payer: Option<String>,
}
