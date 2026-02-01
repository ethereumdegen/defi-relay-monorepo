//! Type definitions for the V2 EIP-155 "permit" payment scheme.
//!
//! This module re-exports types from V1 and defines V2-specific wire format
//! types for EIP-2612 permit-based payments on EVM chains.

pub use crate::scheme::v1_eip155_permit::types::PermitScheme;

use crate::chain::eip155::{ChecksummedAddress, TokenAmount};
use crate::proto::v2;
use crate::scheme::v1_eip155_permit::types::{PermitEvmPayload, PaymentRequirementsExtra};

pub type VerifyRequest = v2::VerifyRequest<PaymentPayload, PaymentRequirements>;
pub type SettleRequest = VerifyRequest;
pub type PaymentPayload = v2::PaymentPayload<PaymentRequirements, PermitEvmPayload>;
pub type PaymentRequirements =
    v2::PaymentRequirements<PermitScheme, TokenAmount, ChecksummedAddress, PaymentRequirementsExtra>;
