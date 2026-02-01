//! Client-side payment signing for the V2 EIP-155 "permit" scheme.
//!
//! This module provides [`V2Eip155PermitClient`] for signing EIP-2612
//! `permit` payments on EVM chains using the V2 protocol.
//!
//! # Usage
//!
//! ```ignore
//! use x402::scheme::v2_eip155_permit::client::V2Eip155PermitClient;
//! use alloy_signer_local::PrivateKeySigner;
//!
//! let signer = PrivateKeySigner::random();
//! let client = V2Eip155PermitClient::new(signer);
//! ```

use crate::chain::eip155::Eip155ChainReference;
use crate::proto::v2::ResourceInfo;
use crate::proto::{PaymentRequired, v2};
use crate::scheme::X402SchemeId;
use crate::scheme::client::{
    PaymentCandidate, PaymentCandidateSigner, X402Error, X402SchemeClient,
};
use crate::scheme::v1_eip155_exact::client::SignerLike;
use crate::scheme::v1_eip155_permit::client::{Eip2612SigningParams, sign_eip2612_permit};
use crate::scheme::v2_eip155_permit::V2Eip155Permit;
use crate::scheme::v2_eip155_permit::types;
use crate::util::Base64Bytes;
use alloy_primitives::{Address, U256};
use async_trait::async_trait;

#[derive(Debug)]
#[allow(dead_code)] // Public for consumption by downstream crates.
pub struct V2Eip155PermitClient<S> {
    signer: S,
}

#[allow(dead_code)] // Public for consumption by downstream crates.
impl<S> V2Eip155PermitClient<S> {
    pub fn new(signer: S) -> Self {
        Self { signer }
    }
}

impl<S> X402SchemeId for V2Eip155PermitClient<S> {
    fn namespace(&self) -> &str {
        V2Eip155Permit.namespace()
    }

    fn scheme(&self) -> &str {
        V2Eip155Permit.scheme()
    }
}

impl<S> X402SchemeClient for V2Eip155PermitClient<S>
where
    S: SignerLike + Clone + Send + Sync + 'static,
{
    fn accept(&self, payment_required: &PaymentRequired) -> Vec<PaymentCandidate> {
        let payment_required = match payment_required {
            PaymentRequired::V2(payment_required) => payment_required,
            PaymentRequired::V1(_) => {
                return vec![];
            }
        };
        payment_required
            .accepts
            .iter()
            .filter_map(|v| {
                let requirements: types::PaymentRequirements = v.as_concrete()?;
                let chain_reference = Eip155ChainReference::try_from(&requirements.network).ok()?;
                let candidate = PaymentCandidate {
                    chain_id: requirements.network.clone(),
                    asset: requirements.asset.to_string(),
                    amount: requirements.amount.into(),
                    scheme: self.scheme().to_string(),
                    x402_version: self.x402_version(),
                    pay_to: requirements.pay_to.to_string(),
                    signer: Box::new(PayloadSigner {
                        resource_info: Some(payment_required.resource.clone()),
                        signer: self.signer.clone(),
                        chain_reference,
                        requirements,
                    }),
                };
                Some(candidate)
            })
            .collect::<Vec<_>>()
    }
}

#[allow(dead_code)] // Public for consumption by downstream crates.
struct PayloadSigner<S> {
    signer: S,
    resource_info: Option<ResourceInfo>,
    chain_reference: Eip155ChainReference,
    requirements: types::PaymentRequirements,
}

#[async_trait]
impl<S> PaymentCandidateSigner for PayloadSigner<S>
where
    S: Sync + SignerLike,
{
    async fn sign_payment(&self) -> Result<String, X402Error> {
        // NOTE: For permit scheme, the client needs to know the facilitator's address
        // and fetch the current nonce from the contract. This is typically done by:
        // 1. Getting the facilitator address from the payment requirements or a separate API
        // 2. Calling token.nonces(owner) to get the current nonce
        //
        // For now, we return an error since the client needs additional context
        // that isn't available in the standard payment requirements.
        Err(X402Error::SigningError(
            "Permit scheme requires nonce fetching from contract. Use sign_eip2612_permit directly with proper nonce.".to_string()
        ))
    }
}

/// Extended payload signer that has access to nonce and spender information.
///
/// This is used when the client has pre-fetched the nonce and knows the facilitator address.
#[allow(dead_code)] // Public for consumption by downstream crates.
pub struct PermitPayloadSignerWithNonce<S> {
    pub signer: S,
    pub resource_info: Option<ResourceInfo>,
    pub chain_reference: Eip155ChainReference,
    pub requirements: types::PaymentRequirements,
    /// The facilitator address that will be the spender
    pub spender: Address,
    /// Pre-fetched nonce from the token contract
    pub nonce: U256,
}

#[async_trait]
impl<S> PaymentCandidateSigner for PermitPayloadSignerWithNonce<S>
where
    S: SignerLike + Sync,
{
    async fn sign_payment(&self) -> Result<String, X402Error> {
        let params = Eip2612SigningParams {
            chain_id: self.chain_reference.inner(),
            asset_address: self.requirements.asset.0,
            spender: self.spender,
            amount: self.requirements.amount.into(),
            nonce: self.nonce,
            max_timeout_seconds: self.requirements.max_timeout_seconds,
            extra: self.requirements.extra.clone(),
        };

        let evm_payload = sign_eip2612_permit(&self.signer, &params).await?;

        // Build the payment payload
        let payload = types::PaymentPayload {
            x402_version: v2::X402Version2,
            accepted: self.requirements.clone(),
            resource: self.resource_info.clone(),
            payload: evm_payload,
        };
        let json = serde_json::to_vec(&payload)?;
        let b64 = Base64Bytes::encode(&json);

        Ok(b64.to_string())
    }
}
