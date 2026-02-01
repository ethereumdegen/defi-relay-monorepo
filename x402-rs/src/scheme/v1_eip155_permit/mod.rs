//! V1 EIP-155 "permit" payment scheme implementation.
//!
//! This module implements the "permit" payment scheme for EVM chains using
//! the V1 x402 protocol. It uses EIP-2612 `permit` for gasless token approvals,
//! followed by `transferFrom` to complete the transfer.
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
//! - Atomic settlement via PermitExecutor contract
//!
//! # Settlement Flow
//!
//! The facilitator calls PermitExecutor which atomically executes:
//! ```text
//! PermitExecutor.executePermitTransfer(token, owner, value, deadline, v, r, s, payTo)
//!     -> token.permit(owner, spender=PermitExecutor, value, deadline, v, r, s)
//!     -> token.transferFrom(owner, payTo, value)
//! ```
//!
//! For EIP-6492 counterfactual wallets:
//! ```text
//! PermitExecutor.executeCounterfactualPermitTransfer(factory, factoryCalldata, ...)
//!     -> factory.deploy(factoryCalldata)  // if wallet not deployed
//!     -> token.permit(...)
//!     -> token.transferFrom(...)
//! ```

use alloy_primitives::{address, Address, Bytes, TxHash, U256, B256};
use alloy_provider::Provider;
use alloy_sol_types::{Eip712Domain, SolCall, SolStruct, sol};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::Instrument;
use tracing::instrument;
use tracing_core::Level;

pub mod client;
pub mod types;

use crate::chain::ChainProvider;
use crate::chain::eip155::{
    ChecksummedAddress, Eip155ChainReference, Eip155MetaTransactionProvider, Eip155TokenDeployment,
    MetaTransaction,
};
use crate::chain::{ChainId, ChainProviderOps, DeployedTokenAmount};
use crate::proto;
use crate::proto::{PaymentVerificationError, v1};
use crate::scheme::v1_eip155_exact::{
    Eip155ExactError, IEIP3009, VALIDATOR_ADDRESS, Validator6492,
    assert_domain, assert_enough_balance, assert_enough_value,
    StructuredSignature, StructuredSignatureFormatError,
};
use crate::scheme::{
    X402SchemeFacilitator, X402SchemeFacilitatorBuilder, X402SchemeFacilitatorError, X402SchemeId,
};
use crate::timestamp::UnixTimestamp;

pub use types::*;

/// PermitExecutor contract address on Base mainnet
/// Deployed at: https://basescan.org/address/0x8b60e6327ca1d15e858474aa1d3756b7270a8dfc
pub const PERMIT_EXECUTOR_BASE: Address = address!("8b60e6327ca1d15e858474aa1d3756b7270a8dfc");

sol! {
    /// PermitExecutor contract interface for atomic permit + transferFrom execution
    #[allow(dead_code)]
    #[sol(rpc)]
    interface IPermitExecutor {
        function executePermitTransfer(
            address token,
            address tokenOwner,
            uint256 value,
            uint256 deadline,
            uint8 v,
            bytes32 r,
            bytes32 s,
            address payTo
        ) external;

        function executePermitTransferWithSignature(
            address token,
            address tokenOwner,
            uint256 value,
            uint256 deadline,
            bytes calldata signature,
            address payTo
        ) external;

        function executeCounterfactualPermitTransfer(
            address factory,
            bytes calldata factoryCalldata,
            address token,
            address tokenOwner,
            uint256 value,
            uint256 deadline,
            bytes calldata signature,
            address payTo
        ) external;

        function executeCounterfactualPermitTransferSplit(
            address factory,
            bytes calldata factoryCalldata,
            address token,
            address tokenOwner,
            uint256 value,
            uint256 deadline,
            uint8 v,
            bytes32 r,
            bytes32 s,
            address payTo
        ) external;
    }
}

pub struct V1Eip155Permit;

impl V1Eip155Permit {
    #[allow(dead_code)] // Public for consumption by downstream crates.
    pub fn price_tag<A: Into<ChecksummedAddress>>(
        pay_to: A,
        asset: DeployedTokenAmount<U256, Eip155TokenDeployment>,
    ) -> v1::PriceTag {
        let chain_id: ChainId = asset.token.chain_reference.into();
        let network = chain_id
            .as_network_name()
            .unwrap_or_else(|| panic!("Can not get network name for chain id {}", chain_id));
        let extra = asset
            .token
            .eip712
            .and_then(|eip712| serde_json::to_value(&eip712).ok());
        v1::PriceTag {
            scheme: PermitScheme.to_string(),
            pay_to: pay_to.into().to_string(),
            asset: asset.token.address.to_string(),
            network: network.to_string(),
            amount: asset.amount.to_string(),
            max_timeout_seconds: 300,
            extra,
            enricher: None,
        }
    }
}

impl X402SchemeId for V1Eip155Permit {
    fn x402_version(&self) -> u8 {
        1
    }
    fn namespace(&self) -> &str {
        "eip155"
    }
    fn scheme(&self) -> &str {
        PermitScheme.as_ref()
    }
}

impl X402SchemeFacilitatorBuilder<&ChainProvider> for V1Eip155Permit {
    fn build(
        &self,
        provider: &ChainProvider,
        config: Option<serde_json::Value>,
    ) -> Result<Box<dyn X402SchemeFacilitator>, Box<dyn std::error::Error>> {
        let eip155_provider = if let ChainProvider::Eip155(provider) = provider {
            Arc::clone(provider)
        } else {
            return Err("V1Eip155Permit::build: provider must be an Eip155ChainProvider".into());
        };
        self.build(eip155_provider, config)
    }
}

impl<P> X402SchemeFacilitatorBuilder<P> for V1Eip155Permit
where
    P: Eip155MetaTransactionProvider + ChainProviderOps + Send + Sync + 'static,
    Eip155ExactError: From<P::Error>,
{
    fn build(
        &self,
        provider: P,
        config: Option<serde_json::Value>,
    ) -> Result<Box<dyn X402SchemeFacilitator>, Box<dyn std::error::Error>> {
        // Extract PermitExecutor address from config, or use default for Base
        let permit_executor = config
            .as_ref()
            .and_then(|c| c.get("permitExecutor"))
            .and_then(|v| v.as_str())
            .map(|s| s.parse::<Address>())
            .transpose()?
            .unwrap_or(PERMIT_EXECUTOR_BASE);

        Ok(Box::new(V1Eip155PermitFacilitator::new(provider, permit_executor)))
    }
}

pub struct V1Eip155PermitFacilitator<P> {
    provider: P,
    permit_executor: Address,
}

impl<P> V1Eip155PermitFacilitator<P> {
    /// Creates a new facilitator with the given provider and PermitExecutor address.
    pub fn new(provider: P, permit_executor: Address) -> Self {
        Self { provider, permit_executor }
    }
}

#[async_trait::async_trait]
impl<P> X402SchemeFacilitator for V1Eip155PermitFacilitator<P>
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
            &self.permit_executor,
            payload,
            requirements,
        )
        .await?;

        let payer = verify_payment(
            self.provider.inner(),
            &self.permit_executor,
            &contract,
            &payment,
            &eip712_domain,
            requirements.pay_to,
        )
        .await?;

        Ok(v1::VerifyResponse::valid(payer.to_string()).into())
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
            &self.permit_executor,
            payload,
            requirements,
        )
        .await?;

        let tx_hash = settle_payment(
            &self.provider,
            &self.permit_executor,
            &contract,
            &payment,
            &eip712_domain,
            requirements.pay_to,
        )
        .await?;
        Ok(v1::SettleResponse::Success {
            payer: payment.owner.to_string(),
            transaction: tx_hash.to_string(),
            network: payload.network.clone(),
        }
        .into())
    }

    async fn supported(&self) -> Result<proto::SupportedResponse, X402SchemeFacilitatorError> {
        let chain_id = self.provider.chain_id();
        let kinds = {
            let mut kinds = Vec::with_capacity(1);
            let network = chain_id.as_network_name();
            if let Some(network) = network {
                kinds.push(proto::SupportedPaymentKind {
                    x402_version: v1::X402Version1.into(),
                    scheme: PermitScheme.to_string(),
                    network: network.to_string(),
                    extra: None,
                });
            }
            kinds
        };
        let signers = {
            let mut signers = HashMap::with_capacity(1);
            // Return PermitExecutor address as the signer for permit scheme
            // Clients must use this address as the spender when signing permits
            signers.insert(chain_id, vec![self.permit_executor.to_string()]);
            signers
        };
        Ok(proto::SupportedResponse {
            kinds,
            extensions: Vec::new(),
            signers,
        })
    }
}

/// A fully specified EIP-2612 permit payload for EVM settlement.
#[derive(Debug)]
pub struct PermitEvmPayment {
    /// Token owner (payer) who grants the approval.
    pub owner: Address,
    /// Spender who is authorized to transfer (must be PermitExecutor contract).
    pub spender: Address,
    /// Approval amount (token units).
    pub value: U256,
    /// Sequential nonce for replay protection.
    pub nonce: U256,
    /// Deadline timestamp after which permit is invalid.
    pub deadline: UnixTimestamp,
    /// Raw signature bytes (EIP-1271 or EIP-6492-wrapped).
    pub signature: Bytes,
}

sol!(
    /// Solidity-compatible struct definition for EIP-2612 `permit`.
    ///
    /// This matches the EIP-2612 format used in EIP-712 typed data:
    /// it authorizes `spender` to spend `value` tokens on behalf of `owner`.
    #[derive(Serialize, Deserialize)]
    struct Permit {
        address owner;
        address spender;
        uint256 value;
        uint256 nonce;
        uint256 deadline;
    }
);

/// Canonical data required to verify a permit signature.
#[derive(Debug, Clone)]
struct SignedPermitMessage {
    /// Expected signer (an EOA or contract wallet).
    address: Address,
    /// 32-byte digest that was signed (typically an EIP-712 hash).
    hash: alloy_primitives::B256,
    /// Structured signature, either EIP-6492 or EIP-1271.
    signature: StructuredSignature,
}

impl SignedPermitMessage {
    /// Construct a [`SignedPermitMessage`] from a [`PermitEvmPayment`] and its
    /// corresponding [`Eip712Domain`].
    pub fn extract(
        payment: &PermitEvmPayment,
        domain: &Eip712Domain,
    ) -> Result<Self, StructuredSignatureFormatError> {
        let permit = Permit {
            owner: payment.owner,
            spender: payment.spender,
            value: payment.value,
            nonce: payment.nonce,
            deadline: U256::from(payment.deadline.as_secs()),
        };
        let eip712_hash = permit.eip712_signing_hash(domain);
        let structured_signature = StructuredSignature::try_from_bytes(
            payment.signature.clone(),
            payment.owner,
            &eip712_hash,
        )?;
        Ok(Self {
            address: payment.owner,
            hash: eip712_hash,
            signature: structured_signature,
        })
    }
}

/// Runs all preconditions needed for a successful permit payment:
/// - Valid scheme, network, and receiver.
/// - Spender must be the PermitExecutor contract.
/// - Nonce matches on-chain nonce.
/// - Valid deadline (not expired).
/// - Correct EIP-712 domain construction.
/// - Sufficient on-chain balance.
/// - Sufficient value in payload.
#[instrument(skip_all, err)]
async fn assert_valid_payment<'a, P: Provider>(
    provider: &'a P,
    chain: &Eip155ChainReference,
    permit_executor: &Address,
    payload: &types::PaymentPayload,
    requirements: &types::PaymentRequirements,
) -> Result<
    (
        IEIP3009::IEIP3009Instance<&'a P>,
        PermitEvmPayment,
        Eip712Domain,
    ),
    Eip155ExactError,
> {
    let chain_id: ChainId = chain.into();
    let payload_chain_id = ChainId::from_network_name(&payload.network)
        .ok_or(PaymentVerificationError::UnsupportedChain)?;
    if payload_chain_id != chain_id {
        return Err(PaymentVerificationError::ChainIdMismatch.into());
    }
    let requirements_chain_id = ChainId::from_network_name(&requirements.network)
        .ok_or(PaymentVerificationError::UnsupportedChain)?;
    if requirements_chain_id != chain_id {
        return Err(PaymentVerificationError::ChainIdMismatch.into());
    }

    let authorization = &payload.payload.authorization;

    // Verify spender is the PermitExecutor contract
    if authorization.spender != *permit_executor {
        return Err(PaymentVerificationError::InvalidSpender.into());
    }

    // Check deadline
    let deadline = authorization.deadline;
    assert_deadline(deadline)?;

    let asset_address = requirements.asset;
    let contract = IEIP3009::new(asset_address, provider);

    let domain = assert_domain(chain, &contract, &asset_address, &requirements.extra).await?;

    // Verify nonce matches on-chain
    let on_chain_nonce = contract
        .nonces(authorization.owner)
        .call()
        .into_future()
        .instrument(tracing::info_span!(
            "fetch_permit_nonce",
            token_contract = %asset_address,
            owner = %authorization.owner,
            otel.kind = "client"
        ))
        .await?;
    if on_chain_nonce != authorization.nonce {
        return Err(PaymentVerificationError::InvalidNonce {
            expected: on_chain_nonce.to_string(),
            actual: authorization.nonce.to_string(),
        }.into());
    }

    let amount_required = requirements.max_amount_required;
    assert_enough_balance(&contract, &authorization.owner, amount_required).await?;
    assert_enough_value(&authorization.value, &amount_required)?;

    let payment = PermitEvmPayment {
        owner: authorization.owner,
        spender: authorization.spender,
        value: authorization.value,
        nonce: authorization.nonce,
        deadline: authorization.deadline,
        signature: payload.payload.signature.clone(),
    };

    Ok((contract, payment, domain))
}

/// Validates that the deadline has not passed.
///
/// Adds a 6-second grace buffer to account for latency.
#[instrument(skip_all, err)]
pub fn assert_deadline(deadline: UnixTimestamp) -> Result<(), PaymentVerificationError> {
    let now = UnixTimestamp::now();
    if deadline < now + 6 {
        return Err(PaymentVerificationError::Expired);
    }
    Ok(())
}

pub async fn verify_payment<P: Provider>(
    provider: &P,
    permit_executor: &Address,
    contract: &IEIP3009::IEIP3009Instance<&P>,
    payment: &PermitEvmPayment,
    eip712_domain: &Eip712Domain,
    pay_to: Address,
) -> Result<Address, Eip155ExactError> {
    let signed_message = SignedPermitMessage::extract(payment, eip712_domain)?;
    let payer = signed_message.address;
    let executor = IPermitExecutor::new(*permit_executor, provider);

    match signed_message.signature {
        StructuredSignature::EIP6492 {
            ref factory,
            ref factory_calldata,
            ref inner,
            ref original,
        } => {
            // First validate the EIP-6492 signature
            let validator6492 = Validator6492::new(VALIDATOR_ADDRESS, provider);
            let is_valid = validator6492
                .isValidSigWithSideEffects(payer, signed_message.hash, original.clone())
                .call()
                .into_future()
                .instrument(tracing::info_span!("validate_eip6492_signature",
                    owner = %payment.owner,
                    otel.kind = "client",
                ))
                .await
                .map_err(|e| PaymentVerificationError::InvalidSignature(e.to_string()))?;

            if !is_valid {
                return Err(PaymentVerificationError::InvalidSignature(
                    "Chain reported EIP-6492 signature to be invalid".to_string(),
                )
                .into());
            }

            // Simulate executeCounterfactualPermitTransfer via PermitExecutor
            executor
                .executeCounterfactualPermitTransfer(
                    *factory,
                    factory_calldata.clone(),
                    *contract.address(),
                    payment.owner,
                    payment.value,
                    U256::from(payment.deadline.as_secs()),
                    inner.clone(),
                    pay_to,
                )
                .call()
                .into_future()
                .instrument(tracing::info_span!("verify_permit_via_executor",
                    owner = %payment.owner,
                    spender = %payment.spender,
                    value = %payment.value,
                    deadline = %payment.deadline.as_secs(),
                    pay_to = %pay_to,
                    token_contract = %contract.address(),
                    sig_kind = "EIP6492",
                    otel.kind = "client",
                ))
                .await
                .map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?;
        }
        StructuredSignature::EIP1271(ref sig) => {
            // Simulate executePermitTransferWithSignature via PermitExecutor
            executor
                .executePermitTransferWithSignature(
                    *contract.address(),
                    payment.owner,
                    payment.value,
                    U256::from(payment.deadline.as_secs()),
                    sig.clone(),
                    pay_to,
                )
                .call()
                .into_future()
                .instrument(tracing::info_span!("verify_permit_via_executor",
                    owner = %payment.owner,
                    spender = %payment.spender,
                    value = %payment.value,
                    deadline = %payment.deadline.as_secs(),
                    pay_to = %pay_to,
                    token_contract = %contract.address(),
                    sig_kind = "EIP1271",
                    otel.kind = "client",
                ))
                .await
                .map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?;
        }
        StructuredSignature::EOA(ref sig) => {
            let v = 27 + (sig.v() as u8);
            let r = B256::from(sig.r());
            let s = B256::from(sig.s());

            tracing::debug!(
                owner = %payment.owner,
                spender = %payment.spender,
                value = %payment.value,
                nonce = %payment.nonce,
                deadline = %payment.deadline.as_secs(),
                v = v,
                r = %r,
                s = %s,
                "permit parameters for EOA signature"
            );

            // Simulate executePermitTransfer via PermitExecutor
            executor
                .executePermitTransfer(
                    *contract.address(),
                    payment.owner,
                    payment.value,
                    U256::from(payment.deadline.as_secs()),
                    v,
                    r,
                    s,
                    pay_to,
                )
                .call()
                .into_future()
                .instrument(tracing::info_span!("verify_permit_via_executor",
                    owner = %payment.owner,
                    spender = %payment.spender,
                    value = %payment.value,
                    deadline = %payment.deadline.as_secs(),
                    pay_to = %pay_to,
                    token_contract = %contract.address(),
                    sig_kind = "EOA",
                    otel.kind = "client",
                ))
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "PermitExecutor simulation failed");
                    PaymentVerificationError::TransactionSimulation(e.to_string())
                })?;
        }
    }

    Ok(payer)
}

pub async fn settle_payment<P, E>(
    provider: &P,
    permit_executor: &Address,
    contract: &IEIP3009::IEIP3009Instance<&P::Inner>,
    payment: &PermitEvmPayment,
    eip712_domain: &Eip712Domain,
    pay_to: Address,
) -> Result<TxHash, Eip155ExactError>
where
    P: Eip155MetaTransactionProvider<Error = E>,
    Eip155ExactError: From<E>,
{
    let signed_message = SignedPermitMessage::extract(payment, eip712_domain)?;

    // Build calldata for PermitExecutor based on signature type
    let (calldata, sig_kind): (Bytes, &str) = match signed_message.signature {
        StructuredSignature::EIP6492 {
            factory,
            factory_calldata,
            inner,
            original: _,
        } => {
            let call = IPermitExecutor::executeCounterfactualPermitTransferCall {
                factory,
                factoryCalldata: factory_calldata,
                token: *contract.address(),
                tokenOwner: payment.owner,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                signature: inner,
                payTo: pay_to,
            };
            (call.abi_encode().into(), "EIP6492")
        }
        StructuredSignature::EIP1271(sig) => {
            let call = IPermitExecutor::executePermitTransferWithSignatureCall {
                token: *contract.address(),
                tokenOwner: payment.owner,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                signature: sig,
                payTo: pay_to,
            };
            (call.abi_encode().into(), "EIP1271")
        }
        StructuredSignature::EOA(sig) => {
            let v = 27 + (sig.v() as u8);
            let r = B256::from(sig.r());
            let s = B256::from(sig.s());

            let call = IPermitExecutor::executePermitTransferCall {
                token: *contract.address(),
                tokenOwner: payment.owner,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                v,
                r,
                s,
                payTo: pay_to,
            };
            (call.abi_encode().into(), "EOA")
        }
    };

    // Send transaction to PermitExecutor
    let receipt = Eip155MetaTransactionProvider::send_transaction(
        provider,
        MetaTransaction {
            to: *permit_executor,
            calldata,
            confirmations: 1,
        },
    )
    .instrument(tracing::info_span!("settle_permit_via_executor",
        owner = %payment.owner,
        spender = %payment.spender,
        value = %payment.value,
        deadline = %payment.deadline.as_secs(),
        pay_to = %pay_to,
        token_contract = %contract.address(),
        permit_executor = %permit_executor,
        sig_kind = sig_kind,
        otel.kind = "client",
    ))
    .await?;

    if receipt.status() {
        tracing::event!(Level::INFO,
            status = "ok",
            tx = %receipt.transaction_hash,
            "PermitExecutor settlement succeeded"
        );
        Ok(receipt.transaction_hash)
    } else {
        tracing::event!(
            Level::WARN,
            status = "failed",
            tx = %receipt.transaction_hash,
            "PermitExecutor settlement failed"
        );
        Err(Eip155ExactError::TransactionReverted(
            receipt.transaction_hash,
        ))
    }
}
