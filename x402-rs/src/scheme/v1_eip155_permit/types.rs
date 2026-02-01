//! Type definitions for the V1 EIP-155 "permit" payment scheme.
//!
//! This module defines the wire format types for EIP-2612 permit-based payments
//! on EVM chains using the V1 x402 protocol.
//!
//! # Key Difference from EIP-3009 (exact scheme)
//!
//! - EIP-3009 `transferWithAuthorization`: Single call transfers tokens directly
//! - EIP-2612 `permit`: Sets approval only, requires separate `transferFrom` call
//!
//! The permit scheme uses a two-step settlement: permit() + transferFrom()

use alloy_primitives::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::lit_str;
use crate::proto::v1;
use crate::timestamp::UnixTimestamp;

lit_str!(PermitScheme, "permit");

pub type VerifyRequest = v1::VerifyRequest<PaymentPayload, PaymentRequirements>;
pub type SettleRequest = VerifyRequest;
pub type PaymentPayload = v1::PaymentPayload<PermitScheme, PermitEvmPayload>;

/// Full payload required to authorize an EIP-2612 permit:
/// includes the signature and the permit authorization struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermitEvmPayload {
    pub signature: Bytes,
    pub authorization: PermitEvmPayloadAuthorization,
}

/// EIP-712 structured data for EIP-2612 permit authorization.
/// Defines who can spend how many tokens on behalf of the owner.
///
/// Key differences from EIP-3009:
/// - `owner` (payer) instead of `from`
/// - `spender` (facilitator) instead of `to` - the facilitator will call transferFrom
/// - `nonce` is sequential (U256) not random (B256)
/// - `deadline` instead of `validBefore`/`validAfter` pair
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermitEvmPayloadAuthorization {
    /// Token owner (payer) who is granting the approval
    pub owner: Address,
    /// Spender who is authorized to transfer tokens (must be the facilitator)
    pub spender: Address,
    /// Amount of tokens to approve
    pub value: U256,
    /// Sequential nonce for replay protection (fetched from contract)
    pub nonce: U256,
    /// Deadline timestamp after which the permit is invalid
    pub deadline: UnixTimestamp,
}

pub type PaymentRequirements =
    v1::PaymentRequirements<PermitScheme, U256, Address, PaymentRequirementsExtra>;

/// Re-export PaymentRequirementsExtra from exact scheme for compatibility
pub use crate::scheme::v1_eip155_exact::types::PaymentRequirementsExtra;
