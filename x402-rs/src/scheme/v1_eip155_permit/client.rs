//! Client-side payment signing for the V1 EIP-155 "permit" scheme.
//!
//! This module provides [`V1Eip155PermitClient`] for signing EIP-2612
//! `permit` payments on EVM chains.
//!
//! # Usage
//!
//! ```ignore
//! use x402::scheme::v1_eip155_permit::client::V1Eip155PermitClient;
//! use alloy_signer_local::PrivateKeySigner;
//!
//! let signer = PrivateKeySigner::random();
//! let client = V1Eip155PermitClient::new(signer);
//! ```

use crate::chain::ChainId;
use crate::chain::eip155::Eip155ChainReference;
use crate::proto::PaymentRequired;
use crate::proto::v1::X402Version1;
use crate::scheme::client::{
    PaymentCandidate, PaymentCandidateSigner, X402Error, X402SchemeClient,
};
use crate::scheme::v1_eip155_exact::client::SignerLike;
use crate::scheme::v1_eip155_permit::{
    Permit, PermitEvmPayload, PermitEvmPayloadAuthorization, PermitScheme, PaymentRequirementsExtra,
    types,
};
use crate::scheme::{V1Eip155Permit, X402SchemeId};
use crate::timestamp::UnixTimestamp;
use crate::util::Base64Bytes;
use alloy_primitives::{Address, U256};
use alloy_sol_types::{SolStruct, eip712_domain};
use async_trait::async_trait;

#[derive(Debug)]
#[allow(dead_code)] // Public for consumption by downstream crates.
pub struct V1Eip155PermitClient<S> {
    signer: S,
}

#[allow(dead_code)] // Public for consumption by downstream crates.
impl<S> V1Eip155PermitClient<S> {
    pub fn new(signer: S) -> Self {
        Self { signer }
    }
}

impl<S> X402SchemeId for V1Eip155PermitClient<S> {
    fn namespace(&self) -> &str {
        V1Eip155Permit.namespace()
    }

    fn scheme(&self) -> &str {
        V1Eip155Permit.scheme()
    }
}

impl<S> X402SchemeClient for V1Eip155PermitClient<S>
where
    S: SignerLike + Clone + Send + Sync + 'static,
{
    fn accept(&self, payment_required: &PaymentRequired) -> Vec<PaymentCandidate> {
        let payment_required = match payment_required {
            PaymentRequired::V1(payment_required) => payment_required,
            PaymentRequired::V2(_) => {
                return vec![];
            }
        };
        payment_required
            .accepts
            .iter()
            .filter_map(|v| {
                let requirements: types::PaymentRequirements = v.as_concrete()?;
                let chain_id = ChainId::from_network_name(&requirements.network)?;
                let chain_reference = Eip155ChainReference::try_from(chain_id.clone()).ok()?;
                let candidate = PaymentCandidate {
                    chain_id,
                    asset: requirements.asset.to_string(),
                    amount: requirements.max_amount_required,
                    scheme: self.scheme().to_string(),
                    x402_version: self.x402_version(),
                    pay_to: requirements.pay_to.to_string(),
                    signer: Box::new(PayloadSigner {
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

/// Shared EIP-712 signing parameters for EIP-2612 permit authorization.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Public for consumption by downstream crates.
pub struct Eip2612SigningParams {
    /// The EIP-155 chain ID (numeric)
    pub chain_id: u64,
    /// The token contract address (verifying contract for EIP-712)
    pub asset_address: Address,
    /// The spender address (facilitator) who will be approved to transfer
    pub spender: Address,
    /// The amount to approve
    pub amount: U256,
    /// The current nonce (must be fetched from contract)
    pub nonce: U256,
    /// Maximum timeout in seconds for the deadline
    pub max_timeout_seconds: u64,
    /// Optional EIP-712 domain name and version override
    pub extra: Option<PaymentRequirementsExtra>,
}

/// Signs an EIP-2612 Permit using EIP-712.
///
/// This is the shared signing logic used by both v1 and v2 EIP-155 permit scheme clients.
/// It constructs the EIP-712 domain, builds the permit struct with appropriate
/// deadline, and signs the resulting hash.
///
/// # Note
///
/// The caller must provide the correct `nonce` fetched from the token contract.
/// EIP-2612 uses sequential nonces, not random ones like EIP-3009.
///
/// # EIP-712 Domain
///
/// The `extra` field MUST contain the token's EIP-712 domain name and version.
/// These values must match what the token contract uses for its domain separator.
/// If not provided, this function will return an error.
///
/// For most EIP-2612 tokens:
/// - `name` should match the token's `name()` function
/// - `version` is typically "1" (many tokens don't expose a `version()` function)
#[allow(dead_code)] // Public for consumption by downstream crates.
pub async fn sign_eip2612_permit<S: SignerLike + Sync>(
    signer: &S,
    params: &Eip2612SigningParams,
) -> Result<PermitEvmPayload, X402Error> {
    // Extract name/version from extra - these MUST be provided for permit scheme
    // since the EIP-712 domain must match what the token contract uses.
    let (name, version) = match &params.extra {
        None => {
            return Err(X402Error::SigningError(
                "Permit scheme requires 'extra' field with EIP-712 domain 'name' and 'version'. \
                 These must match the token contract's domain separator.".to_string()
            ));
        }
        Some(extra) => (extra.name.clone(), extra.version.clone()),
    };

    // Build EIP-712 domain
    let domain = eip712_domain! {
        name: name,
        version: version,
        chain_id: params.chain_id,
        verifying_contract: params.asset_address,
    };

    // Build permit with deadline
    let now = UnixTimestamp::now();
    let deadline = now + params.max_timeout_seconds;

    let authorization = PermitEvmPayloadAuthorization {
        owner: signer.address(),
        spender: params.spender,
        value: params.amount,
        nonce: params.nonce,
        deadline,
    };

    // Create the EIP-712 struct for signing
    // IMPORTANT: The values here MUST match the authorization struct exactly,
    // as the facilitator will reconstruct this struct from the authorization
    // to verify the signature.
    let permit = Permit {
        owner: authorization.owner,
        spender: authorization.spender,
        value: authorization.value,
        nonce: authorization.nonce,
        deadline: U256::from(authorization.deadline.as_secs()),
    };

    let eip712_hash = permit.eip712_signing_hash(&domain);
    let signature = signer
        .sign_hash(&eip712_hash)
        .await
        .map_err(|e| X402Error::SigningError(format!("{e:?}")))?;

    Ok(PermitEvmPayload {
        signature: signature.as_bytes().into(),
        authorization,
    })
}

#[allow(dead_code)] // Public for consumption by downstream crates.
struct PayloadSigner<S> {
    signer: S,
    chain_reference: Eip155ChainReference,
    requirements: types::PaymentRequirements,
}

#[async_trait]
impl<S> PaymentCandidateSigner for PayloadSigner<S>
where
    S: SignerLike + Sync,
{
    async fn sign_payment(&self) -> Result<String, X402Error> {
        // NOTE: For permit scheme, the client needs to know the facilitator's address
        // and fetch the current nonce from the contract. This is typically done by:
        // 1. Getting the facilitator address from the payment requirements or a separate API
        // 2. Calling token.nonces(owner) to get the current nonce
        //
        // For now, we return an error since the client needs additional context
        // that isn't available in the standard payment requirements.
        //
        // In practice, clients using the permit scheme should:
        // - Use a specialized client that has access to an RPC provider
        // - Fetch the nonce before signing
        // - Know the facilitator's address (spender)

        // This is a limitation of the current client interface.
        // A real implementation would need to be extended to support async nonce fetching.
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
            asset_address: self.requirements.asset,
            spender: self.spender,
            amount: self.requirements.max_amount_required,
            nonce: self.nonce,
            max_timeout_seconds: self.requirements.max_timeout_seconds,
            extra: self.requirements.extra.clone(),
        };

        let evm_payload = sign_eip2612_permit(&self.signer, &params).await?;

        // Build the payment payload
        let payload = types::PaymentPayload {
            x402_version: X402Version1,
            scheme: PermitScheme,
            network: self.requirements.network.clone(),
            payload: evm_payload,
        };
        let json = serde_json::to_vec(&payload)?;
        let b64 = Base64Bytes::encode(&json);

        Ok(b64.to_string())
    }
}
