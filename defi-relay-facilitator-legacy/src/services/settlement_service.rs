use std::sync::Arc;

use ethers::prelude::*;
use ethers::providers::{Http, Provider};

use crate::config::{usdc_address, BASE_CHAIN_ID};
use crate::error::FacilitatorError;
use crate::types::Eip3009Payload;

// Generate USDC contract bindings
abigen!(
    USDC,
    r#"[
        function receiveWithAuthorization(address from, address to, uint256 value, uint256 validAfter, uint256 validBefore, bytes32 nonce, uint8 v, bytes32 r, bytes32 s) external
        function authorizationState(address authorizer, bytes32 nonce) external view returns (bool)
    ]"#
);

pub struct SettlementService {
    provider: Arc<Provider<Http>>,
    wallet: LocalWallet,
    usdc_address: Address,
}

impl SettlementService {
    pub fn new(rpc_url: &str, private_key: &str) -> Result<Self, FacilitatorError> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .map_err(|e| FacilitatorError::RpcError(e.to_string()))?;

        let private_key = private_key.strip_prefix("0x").unwrap_or(private_key);
        let wallet: LocalWallet = private_key
            .parse::<LocalWallet>()
            .map_err(|e| FacilitatorError::ConfigError(format!("Invalid private key: {}", e)))?
            .with_chain_id(BASE_CHAIN_ID);

        Ok(Self {
            provider: Arc::new(provider),
            wallet,
            usdc_address: usdc_address(),
        })
    }

    pub fn facilitator_address(&self) -> Address {
        self.wallet.address()
    }

    /// Check if a nonce has already been used
    pub async fn is_nonce_used(
        &self,
        authorizer: Address,
        nonce: [u8; 32],
    ) -> Result<bool, FacilitatorError> {
        let contract = USDC::new(self.usdc_address, self.provider.clone());

        let is_used = contract
            .authorization_state(authorizer, nonce)
            .call()
            .await
            .map_err(|e| FacilitatorError::RpcError(format!("Failed to check nonce: {}", e)))?;

        Ok(is_used)
    }

    /// Submit receiveWithAuthorization transaction
    pub async fn settle(
        &self,
        payload: &Eip3009Payload,
        signature_hex: &str,
    ) -> Result<String, FacilitatorError> {
        // Parse signature
        let sig_hex = signature_hex.strip_prefix("0x").unwrap_or(signature_hex);
        let sig_bytes = hex::decode(sig_hex)
            .map_err(|e| FacilitatorError::InvalidSignature(format!("Invalid hex: {}", e)))?;

        if sig_bytes.len() != 65 {
            return Err(FacilitatorError::InvalidSignature(format!(
                "Invalid signature length: expected 65, got {}",
                sig_bytes.len()
            )));
        }

        let r: [u8; 32] = sig_bytes[0..32]
            .try_into()
            .map_err(|_| FacilitatorError::InvalidSignature("Invalid r".to_string()))?;
        let s: [u8; 32] = sig_bytes[32..64]
            .try_into()
            .map_err(|_| FacilitatorError::InvalidSignature("Invalid s".to_string()))?;
        let v = sig_bytes[64];

        // Create signed client
        let client = SignerMiddleware::new(self.provider.clone(), self.wallet.clone());
        let client = Arc::new(client);

        let contract = USDC::new(self.usdc_address, client);

        // Convert payload values
        let from = payload.from.inner();
        let to = payload.to.inner();
        let value = payload.value.inner();
        let valid_after = payload.valid_after.inner();
        let valid_before = payload.valid_before.inner();
        let nonce = payload.nonce.inner();

        // Build and send transaction
        let tx = contract.receive_with_authorization(
            from,
            to,
            value,
            valid_after,
            valid_before,
            nonce,
            v,
            r,
            s,
        );

        let pending_tx = tx
            .send()
            .await
            .map_err(|e| FacilitatorError::SettlementFailed(format!("Failed to send tx: {}", e)))?;

        log::info!("Settlement tx sent: {:?}", pending_tx.tx_hash());

        // Wait for confirmation
        let receipt = pending_tx
            .await
            .map_err(|e| {
                FacilitatorError::SettlementFailed(format!("Failed to confirm tx: {}", e))
            })?
            .ok_or_else(|| {
                FacilitatorError::SettlementFailed("Transaction dropped from mempool".to_string())
            })?;

        let tx_hash = format!("{:?}", receipt.transaction_hash);
        log::info!("Settlement confirmed: {}", tx_hash);

        Ok(tx_hash)
    }
}
