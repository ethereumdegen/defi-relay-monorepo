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
//! - Atomic settlement via Multicall3 (permit + transferFrom)
//!
//! # Settlement Flow
//!
//! ```text
//! Multicall3.aggregate3([
//!     Call3 { target: token, callData: permit(owner, spender, value, deadline, sig) },
//!     Call3 { target: token, callData: transferFrom(owner, pay_to, value) }
//! ])
//! ```
//!
//! For EIP-6492 counterfactual wallets (3 calls):
//! ```text
//! Multicall3.aggregate3([
//!     Call3 { target: factory, callData: deploy_wallet, allowFailure: true },
//!     Call3 { target: token, callData: permit(...) },
//!     Call3 { target: token, callData: transferFrom(...) }
//! ])
//! ```

use alloy_primitives::{Address, Bytes, TxHash, U256};
use alloy_provider::bindings::IMulticall3;
use alloy_provider::{MULTICALL3_ADDRESS, Provider};
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
        _config: Option<serde_json::Value>,
    ) -> Result<Box<dyn X402SchemeFacilitator>, Box<dyn std::error::Error>> {
        Ok(Box::new(V1Eip155PermitFacilitator::new(provider)))
    }
}

pub struct V1Eip155PermitFacilitator<P> {
    provider: P,
}

impl<P> V1Eip155PermitFacilitator<P> {
    /// Creates a new facilitator with the given provider.
    pub fn new(provider: P) -> Self {
        Self { provider }
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

/// A fully specified EIP-2612 permit payload for EVM settlement.
#[derive(Debug)]
pub struct PermitEvmPayment {
    /// Token owner (payer) who grants the approval.
    pub owner: Address,
    /// Spender who is authorized to transfer (must be facilitator).
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
/// - Spender must be the facilitator.
/// - Nonce matches on-chain nonce.
/// - Valid deadline (not expired).
/// - Correct EIP-712 domain construction.
/// - Sufficient on-chain balance.
/// - Sufficient value in payload.
#[instrument(skip_all, err)]
async fn assert_valid_payment<'a, P: Provider>(
    provider: &'a P,
    chain: &Eip155ChainReference,
    facilitator_signers: &[String],
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

    // Verify spender is the facilitator
    let spender_str = authorization.spender.to_string();
    let is_valid_spender = facilitator_signers.iter().any(|s| s.eq_ignore_ascii_case(&spender_str));
    if !is_valid_spender {
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

/// Check whether contract code is present at `address`.
async fn is_contract_deployed<P: Provider>(
    provider: &P,
    address: &Address,
) -> Result<bool, alloy_transport::TransportError> {
    let bytes = provider
        .get_code_at(*address)
        .into_future()
        .instrument(tracing::info_span!("get_code_at",
            address = %address,
            otel.kind = "client",
        ))
        .await?;
    Ok(!bytes.is_empty())
}

pub async fn verify_payment<P: Provider>(
    provider: &P,
    contract: &IEIP3009::IEIP3009Instance<&P>,
    payment: &PermitEvmPayment,
    eip712_domain: &Eip712Domain,
    pay_to: Address,
) -> Result<Address, Eip155ExactError> {
    let signed_message = SignedPermitMessage::extract(payment, eip712_domain)?;

    let payer = signed_message.address;
    let hash = signed_message.hash;

    match signed_message.signature {
        StructuredSignature::EIP6492 {
            ref inner,
            ref original,
            ..
        } => {
            // Prepare the call to validate EIP-6492 signature
            let validator6492 = Validator6492::new(VALIDATOR_ADDRESS, provider);
            let is_valid_signature_call =
                validator6492.isValidSigWithSideEffects(payer, hash, original.clone());

            // Build permit call with inner signature
            let permit_call = contract.permit_0(
                payment.owner,
                payment.spender,
                payment.value,
                U256::from(payment.deadline.as_secs()),
                inner.clone(),
            );

            // Build transferFrom call
            let transfer_call = contract.transferFrom(payment.owner, pay_to, payment.value);

            // Execute all calls in a single transaction simulation
            let (is_valid_signature_result, _permit_result, _transfer_result) = provider
                .multicall()
                .add(is_valid_signature_call)
                .add(permit_call)
                .add(transfer_call)
                .aggregate3()
                .instrument(tracing::info_span!("verify_permit_eip6492",
                    owner = %payment.owner,
                    spender = %payment.spender,
                    value = %payment.value,
                    deadline = %payment.deadline.as_secs(),
                    token_contract = %contract.address(),
                    otel.kind = "client",
                ))
                .await?;

            let is_valid_signature_result = is_valid_signature_result
                .map_err(|e| PaymentVerificationError::InvalidSignature(e.to_string()))?;
            if !is_valid_signature_result {
                return Err(PaymentVerificationError::InvalidSignature(
                    "Chain reported signature to be invalid".to_string(),
                )
                .into());
            }
            _permit_result.map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?;
            _transfer_result.map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?;
        }
        StructuredSignature::EIP1271(ref sig) => {
            // Build permit call with EIP-1271 signature
            let permit_call = contract.permit_0(
                payment.owner,
                payment.spender,
                payment.value,
                U256::from(payment.deadline.as_secs()),
                sig.clone(),
            );

            // Build transferFrom call
            let transfer_call = contract.transferFrom(payment.owner, pay_to, payment.value);

            // Simulate permit + transferFrom via multicall
            let (_permit_result, _transfer_result) = provider
                .multicall()
                .add(permit_call)
                .add(transfer_call)
                .aggregate3()
                .instrument(tracing::info_span!("verify_permit_transferFrom",
                    owner = %payment.owner,
                    spender = %payment.spender,
                    value = %payment.value,
                    deadline = %payment.deadline.as_secs(),
                    pay_to = %pay_to,
                    token_contract = %contract.address(),
                    sig_kind = "EIP1271",
                    otel.kind = "client",
                ))
                .await?;

            _permit_result.map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?;
            _transfer_result.map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?;
        }
        StructuredSignature::EOA(ref sig) => {
            // Build permit call with EOA signature (v, r, s)
            let v = 27 + (sig.v() as u8);
            let r = alloy_primitives::B256::from(sig.r());
            let s = alloy_primitives::B256::from(sig.s());

            let permit_call = contract.permit_1(
                payment.owner,
                payment.spender,
                payment.value,
                U256::from(payment.deadline.as_secs()),
                v,
                r,
                s,
            );

            // Build transferFrom call
            let transfer_call = contract.transferFrom(payment.owner, pay_to, payment.value);

            // Simulate permit + transferFrom via multicall
            let (_permit_result, _transfer_result) = provider
                .multicall()
                .add(permit_call)
                .add(transfer_call)
                .aggregate3()
                .instrument(tracing::info_span!("verify_permit_transferFrom",
                    owner = %payment.owner,
                    spender = %payment.spender,
                    value = %payment.value,
                    deadline = %payment.deadline.as_secs(),
                    pay_to = %pay_to,
                    token_contract = %contract.address(),
                    sig_kind = "EOA",
                    otel.kind = "client",
                ))
                .await?;

            _permit_result.map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?;
            _transfer_result.map_err(|e| PaymentVerificationError::TransactionSimulation(e.to_string()))?;
        }
    }

    Ok(payer)
}

/// Build permit calldata based on signature type
fn build_permit_calldata(payment: &PermitEvmPayment, signature: &StructuredSignature) -> Bytes {
    match signature {
        StructuredSignature::EOA(sig) => {
            let v = 27 + (sig.v() as u8);
            let r = alloy_primitives::B256::from(sig.r());
            let s = alloy_primitives::B256::from(sig.s());
            IEIP3009::permit_1Call {
                owner: payment.owner,
                spender: payment.spender,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                v,
                r,
                s,
            }
            .abi_encode()
            .into()
        }
        StructuredSignature::EIP1271(sig) => {
            IEIP3009::permit_0Call {
                owner: payment.owner,
                spender: payment.spender,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                signature: sig.clone(),
            }
            .abi_encode()
            .into()
        }
        StructuredSignature::EIP6492 { inner, .. } => {
            // For EIP-6492, use the inner signature with the bytes variant
            IEIP3009::permit_0Call {
                owner: payment.owner,
                spender: payment.spender,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                signature: inner.clone(),
            }
            .abi_encode()
            .into()
        }
    }
}

pub async fn settle_payment<P, E>(
    provider: &P,
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
    let payer = payment.owner;

    // Build permit calldata
    let permit_calldata = build_permit_calldata(payment, &signed_message.signature);

    // Build transferFrom calldata
    let transfer_from_call = IEIP3009::transferFromCall {
        from: payment.owner,
        to: pay_to,
        value: payment.value,
    };
    let transfer_calldata: Bytes = transfer_from_call.abi_encode().into();

    let transaction_receipt_fut = match signed_message.signature {
        StructuredSignature::EIP6492 {
            factory,
            factory_calldata,
            inner,
            original: _,
        } => {
            let is_contract_deployed = is_contract_deployed(provider.inner(), &payer).await?;

            // Build permit call with inner signature
            let inner_permit_calldata: Bytes = IEIP3009::permit_0Call {
                owner: payment.owner,
                spender: payment.spender,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                signature: inner,
            }
            .abi_encode()
            .into();

            if is_contract_deployed {
                // Wallet already deployed: permit + transferFrom
                let permit_call = IMulticall3::Call3 {
                    allowFailure: false,
                    target: *contract.address(),
                    callData: inner_permit_calldata,
                };
                let transfer_call = IMulticall3::Call3 {
                    allowFailure: false,
                    target: *contract.address(),
                    callData: transfer_calldata,
                };
                let aggregate_call = IMulticall3::aggregate3Call {
                    calls: vec![permit_call, transfer_call],
                };
                Eip155MetaTransactionProvider::send_transaction(
                    provider,
                    MetaTransaction {
                        to: MULTICALL3_ADDRESS,
                        calldata: aggregate_call.abi_encode().into(),
                        confirmations: 1,
                    },
                )
                .instrument(
                    tracing::info_span!("settle_permit_transferFrom",
                        owner = %payment.owner,
                        spender = %payment.spender,
                        value = %payment.value,
                        deadline = %payment.deadline.as_secs(),
                        pay_to = %pay_to,
                        token_contract = %contract.address(),
                        sig_kind="EIP6492.deployed",
                        otel.kind = "client",
                    ),
                )
            } else {
                // Deploy wallet, permit, transferFrom
                let deployment_call = IMulticall3::Call3 {
                    allowFailure: true,
                    target: factory,
                    callData: factory_calldata,
                };
                let permit_call = IMulticall3::Call3 {
                    allowFailure: false,
                    target: *contract.address(),
                    callData: inner_permit_calldata,
                };
                let transfer_call = IMulticall3::Call3 {
                    allowFailure: false,
                    target: *contract.address(),
                    callData: transfer_calldata,
                };
                let aggregate_call = IMulticall3::aggregate3Call {
                    calls: vec![deployment_call, permit_call, transfer_call],
                };
                Eip155MetaTransactionProvider::send_transaction(
                    provider,
                    MetaTransaction {
                        to: MULTICALL3_ADDRESS,
                        calldata: aggregate_call.abi_encode().into(),
                        confirmations: 1,
                    },
                )
                .instrument(
                    tracing::info_span!("settle_permit_transferFrom",
                        owner = %payment.owner,
                        spender = %payment.spender,
                        value = %payment.value,
                        deadline = %payment.deadline.as_secs(),
                        pay_to = %pay_to,
                        token_contract = %contract.address(),
                        sig_kind="EIP6492.counterfactual",
                        otel.kind = "client",
                    ),
                )
            }
        }
        StructuredSignature::EIP1271(_) | StructuredSignature::EOA(_) => {
            // permit + transferFrom via multicall
            let permit_call = IMulticall3::Call3 {
                allowFailure: false,
                target: *contract.address(),
                callData: permit_calldata,
            };
            let transfer_call = IMulticall3::Call3 {
                allowFailure: false,
                target: *contract.address(),
                callData: transfer_calldata,
            };
            let aggregate_call = IMulticall3::aggregate3Call {
                calls: vec![permit_call, transfer_call],
            };
            let sig_kind = match signed_message.signature {
                StructuredSignature::EOA(_) => "EOA",
                StructuredSignature::EIP1271(_) => "EIP1271",
                _ => unreachable!(),
            };
            Eip155MetaTransactionProvider::send_transaction(
                provider,
                MetaTransaction {
                    to: MULTICALL3_ADDRESS,
                    calldata: aggregate_call.abi_encode().into(),
                    confirmations: 1,
                },
            )
            .instrument(tracing::info_span!("settle_permit_transferFrom",
                owner = %payment.owner,
                spender = %payment.spender,
                value = %payment.value,
                deadline = %payment.deadline.as_secs(),
                pay_to = %pay_to,
                token_contract = %contract.address(),
                sig_kind = sig_kind,
                otel.kind = "client",
            ))
        }
    };

    let receipt = transaction_receipt_fut.await?;
    let success = receipt.status();
    if success {
        tracing::event!(Level::INFO,
            status = "ok",
            tx = %receipt.transaction_hash,
            "permit + transferFrom succeeded"
        );
        Ok(receipt.transaction_hash)
    } else {
        tracing::event!(
            Level::WARN,
            status = "failed",
            tx = %receipt.transaction_hash,
            "permit + transferFrom failed"
        );
        Err(Eip155ExactError::TransactionReverted(
            receipt.transaction_hash,
        ))
    }
}
