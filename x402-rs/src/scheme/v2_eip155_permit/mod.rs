//! V2 EIP-155 "permit" payment scheme implementation.
//!
//! This module implements the "permit" payment scheme for EVM chains using
//! the V2 x402 protocol. It builds on the V1 implementation but uses
//! CAIP-2 chain identifiers instead of network names.
//!
//! # Differences from V1
//!
//! - Uses CAIP-2 chain IDs (e.g., `eip155:8453`) instead of network names
//! - Payment requirements are embedded in the payload for verification
//! - Cleaner separation between accepted requirements and authorization
//!
//! # Key Difference from EIP-3009 (exact scheme)
//!
//! - EIP-3009 `transferWithAuthorization`: Single call transfers tokens directly
//! - EIP-2612 `permit`: Sets approval only, requires separate `transferFrom` call
//!
//! # Features
//!
//! - EIP-712 typed data signing for permit authorization
//! - EIP-6492 support for counterfactual smart wallet signatures
//! - EIP-1271 support for deployed smart wallet signatures
//! - EOA signature support with split (v, r, s) components
//! - On-chain balance and nonce verification before settlement
//! - Atomic settlement via Multicall3 (permit + transferFrom)

pub mod client;
pub mod types;

use alloy_primitives::U256;
use alloy_provider::Provider;
use alloy_sol_types::Eip712Domain;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;

use crate::chain::ChainProvider;
use crate::chain::eip155::{
    ChecksummedAddress, Eip155ChainReference, Eip155MetaTransactionProvider, Eip155TokenDeployment,
};
use crate::chain::{ChainId, ChainProviderOps, DeployedTokenAmount};
use crate::proto;
use crate::proto::PaymentVerificationError;
use crate::proto::v2;
use crate::scheme::v1_eip155_exact::{
    Eip155ExactError, IEIP3009, assert_domain, assert_enough_balance, assert_enough_value,
};
use crate::scheme::v1_eip155_permit::{
    PermitEvmPayment, assert_deadline, settle_payment, verify_payment,
};
use crate::scheme::{
    X402SchemeFacilitator, X402SchemeFacilitatorBuilder, X402SchemeFacilitatorError, X402SchemeId,
};

#[allow(unused)]
pub use types::*;

pub struct V2Eip155Permit;

impl V2Eip155Permit {
    #[allow(dead_code)] // Public for consumption by downstream crates.
    pub fn price_tag<A: Into<ChecksummedAddress>>(
        pay_to: A,
        asset: DeployedTokenAmount<U256, Eip155TokenDeployment>,
    ) -> v2::PriceTag {
        let chain_id: ChainId = asset.token.chain_reference.into();
        let extra = asset
            .token
            .eip712
            .and_then(|eip712| serde_json::to_value(&eip712).ok());
        let requirements = v2::PaymentRequirements {
            scheme: PermitScheme.to_string(),
            pay_to: pay_to.into().to_string(),
            asset: asset.token.address.to_string(),
            network: chain_id,
            amount: asset.amount.to_string(),
            max_timeout_seconds: 300,
            extra,
        };
        v2::PriceTag {
            requirements,
            enricher: None,
        }
    }
}

impl X402SchemeId for V2Eip155Permit {
    fn namespace(&self) -> &str {
        "eip155"
    }

    fn scheme(&self) -> &str {
        types::PermitScheme.as_ref()
    }
}

impl X402SchemeFacilitatorBuilder<&ChainProvider> for V2Eip155Permit {
    fn build(
        &self,
        provider: &ChainProvider,
        config: Option<serde_json::Value>,
    ) -> Result<Box<dyn X402SchemeFacilitator>, Box<dyn std::error::Error>> {
        let eip155_provider = if let ChainProvider::Eip155(provider) = provider {
            Arc::clone(provider)
        } else {
            return Err("V2Eip155Permit::build: provider must be an Eip155ChainProvider".into());
        };
        self.build(eip155_provider, config)
    }
}

impl<P> X402SchemeFacilitatorBuilder<P> for V2Eip155Permit
where
    P: Eip155MetaTransactionProvider + ChainProviderOps + Send + Sync + 'static,
    Eip155ExactError: From<P::Error>,
{
    fn build(
        &self,
        provider: P,
        _config: Option<serde_json::Value>,
    ) -> Result<Box<dyn X402SchemeFacilitator>, Box<dyn std::error::Error>> {
        Ok(Box::new(V2Eip155PermitFacilitator::new(provider)))
    }
}

pub struct V2Eip155PermitFacilitator<P> {
    provider: P,
}

impl<P> V2Eip155PermitFacilitator<P> {
    /// Creates a new facilitator with the given provider.
    pub fn new(provider: P) -> Self {
        Self { provider }
    }
}

#[async_trait::async_trait]
impl<P> X402SchemeFacilitator for V2Eip155PermitFacilitator<P>
where
    P: Eip155MetaTransactionProvider + ChainProviderOps + Send + Sync,
    P::Inner: Provider,
    Eip155ExactError: From<P::Error>,
{
    async fn verify(
        &self,
        request: &proto::VerifyRequest,
    ) -> Result<proto::VerifyResponse, X402SchemeFacilitatorError> {
        let request = types::VerifyRequest::from_proto(request.clone())?;
        let payload = &request.payment_payload;
        let requirements = &request.payment_requirements;
        let (contract, payment, eip712_domain) = assert_valid_payment(
            self.provider.inner(),
            self.provider.chain(),
            &self.provider.signer_addresses(),
            payload,
            requirements,
        )
        .await?;

        let payer = verify_payment(
            self.provider.inner(),
            &contract,
            &payment,
            &eip712_domain,
            requirements.pay_to.into(),
        )
        .await?;
        Ok(v2::VerifyResponse::valid(payer.to_string()).into())
    }

    async fn settle(
        &self,
        request: &proto::SettleRequest,
    ) -> Result<proto::SettleResponse, X402SchemeFacilitatorError> {
        let request = types::SettleRequest::from_proto(request.clone())?;
        let payload = &request.payment_payload;
        let requirements = &request.payment_requirements;
        let (contract, payment, eip712_domain) = assert_valid_payment(
            self.provider.inner(),
            self.provider.chain(),
            &self.provider.signer_addresses(),
            payload,
            requirements,
        )
        .await?;

        let tx_hash = settle_payment(
            &self.provider,
            &contract,
            &payment,
            &eip712_domain,
            requirements.pay_to.into(),
        )
        .await?;

        Ok(v2::SettleResponse::Success {
            payer: payment.owner.to_string(),
            transaction: tx_hash.to_string(),
            network: payload.accepted.network.to_string(),
        }
        .into())
    }

    async fn supported(&self) -> Result<proto::SupportedResponse, X402SchemeFacilitatorError> {
        let chain_id = self.provider.chain_id();
        let kinds = vec![proto::SupportedPaymentKind {
            x402_version: v2::X402Version2.into(),
            scheme: PermitScheme.to_string(),
            network: chain_id.clone().into(),
            extra: None,
        }];
        let signers = {
            let mut signers = HashMap::with_capacity(1);
            signers.insert(chain_id, self.provider.signer_addresses());
            signers
        };
        Ok(proto::SupportedResponse {
            kinds,
            extensions: Vec::new(),
            signers,
        })
    }
}

/// Runs all preconditions needed for a successful permit payment:
/// - Valid scheme, network, and receiver.
/// - Spender must be the facilitator.
/// - Nonce matches on-chain nonce.
/// - Valid deadline (not expired).
/// - Correct EIP-712 domain construction.
/// - Sufficient on-chain balance.
/// - Sufficient value in payload.
#[instrument(skip_all, err)]
async fn assert_valid_payment<P: Provider>(
    provider: P,
    chain: &Eip155ChainReference,
    facilitator_signers: &[String],
    payload: &types::PaymentPayload,
    requirements: &types::PaymentRequirements,
) -> Result<(IEIP3009::IEIP3009Instance<P>, PermitEvmPayment, Eip712Domain), Eip155ExactError> {
    let accepted = &payload.accepted;
    if accepted != requirements {
        return Err(PaymentVerificationError::AcceptedRequirementsMismatch.into());
    }
    let payload_inner = &payload.payload;

    let chain_id: ChainId = chain.into();
    let payload_chain_id = &accepted.network;
    if payload_chain_id != &chain_id {
        return Err(PaymentVerificationError::ChainIdMismatch.into());
    }

    let authorization = &payload_inner.authorization;

    // Verify spender is the facilitator
    let spender_str = authorization.spender.to_string();
    let is_valid_spender = facilitator_signers.iter().any(|s| s.eq_ignore_ascii_case(&spender_str));
    if !is_valid_spender {
        return Err(PaymentVerificationError::InvalidSpender.into());
    }

    // Check deadline
    let deadline = authorization.deadline;
    assert_deadline(deadline)?;

    let asset_address = accepted.asset;
    let contract = IEIP3009::new(asset_address.into(), provider);

    let domain = assert_domain(chain, &contract, &asset_address.into(), &accepted.extra).await?;

    // Verify nonce matches on-chain
    let on_chain_nonce = contract
        .nonces(authorization.owner)
        .call()
        .await?;
    if on_chain_nonce != authorization.nonce {
        return Err(PaymentVerificationError::InvalidNonce {
            expected: on_chain_nonce.to_string(),
            actual: authorization.nonce.to_string(),
        }.into());
    }

    let amount_required = accepted.amount;
    assert_enough_balance(&contract, &authorization.owner, amount_required.into()).await?;
    assert_enough_value(&authorization.value, &amount_required.into())?;

    let payment = PermitEvmPayment {
        owner: authorization.owner,
        spender: authorization.spender,
        value: authorization.value,
        nonce: authorization.nonce,
        deadline: authorization.deadline,
        signature: payload_inner.signature.clone(),
    };

    Ok((contract, payment, domain))
}
