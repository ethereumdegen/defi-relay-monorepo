use ethers::types::{Address, Signature, U256};
use tiny_keccak::{Hasher, Keccak};

use crate::config::{BASE_CHAIN_ID, USDC_NAME, USDC_VERSION};
use crate::error::FacilitatorError;
use crate::types::Eip3009Payload;

/// EIP-712 signature verifier for USDC ReceiveWithAuthorization
pub struct Eip712Verifier {
    domain_separator: [u8; 32],
    type_hash: [u8; 32],
}

impl Eip712Verifier {
    pub fn new(usdc_address: Address) -> Self {
        let domain_separator = Self::compute_domain_separator(usdc_address);
        let type_hash = Self::compute_type_hash();

        Self {
            domain_separator,
            type_hash,
        }
    }

    /// Compute EIP-712 domain separator for USDC on Base
    fn compute_domain_separator(verifying_contract: Address) -> [u8; 32] {
        // EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)
        let domain_type_hash = keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
        );

        let name_hash = keccak256(USDC_NAME.as_bytes());
        let version_hash = keccak256(USDC_VERSION.as_bytes());

        let mut chain_id_bytes = [0u8; 32];
        U256::from(BASE_CHAIN_ID).to_big_endian(&mut chain_id_bytes);

        let mut address_bytes = [0u8; 32];
        address_bytes[12..32].copy_from_slice(verifying_contract.as_bytes());

        // Encode and hash: type_hash || name_hash || version_hash || chainId || verifyingContract
        let mut encoded = Vec::with_capacity(160);
        encoded.extend_from_slice(&domain_type_hash);
        encoded.extend_from_slice(&name_hash);
        encoded.extend_from_slice(&version_hash);
        encoded.extend_from_slice(&chain_id_bytes);
        encoded.extend_from_slice(&address_bytes);

        keccak256(&encoded)
    }

    /// Compute type hash for ReceiveWithAuthorization
    fn compute_type_hash() -> [u8; 32] {
        keccak256(
            b"ReceiveWithAuthorization(address from,address to,uint256 value,uint256 validAfter,uint256 validBefore,bytes32 nonce)"
        )
    }

    /// Compute struct hash for ReceiveWithAuthorization parameters
    fn compute_struct_hash(&self, payload: &Eip3009Payload) -> [u8; 32] {
        let mut from_bytes = [0u8; 32];
        from_bytes[12..32].copy_from_slice(payload.from.inner().as_bytes());

        let mut to_bytes = [0u8; 32];
        to_bytes[12..32].copy_from_slice(payload.to.inner().as_bytes());

        let mut value_bytes = [0u8; 32];
        payload.value.inner().to_big_endian(&mut value_bytes);

        let mut valid_after_bytes = [0u8; 32];
        payload.valid_after.inner().to_big_endian(&mut valid_after_bytes);

        let mut valid_before_bytes = [0u8; 32];
        payload.valid_before.inner().to_big_endian(&mut valid_before_bytes);

        // Encode: type_hash || from || to || value || validAfter || validBefore || nonce
        let mut encoded = Vec::with_capacity(224);
        encoded.extend_from_slice(&self.type_hash);
        encoded.extend_from_slice(&from_bytes);
        encoded.extend_from_slice(&to_bytes);
        encoded.extend_from_slice(&value_bytes);
        encoded.extend_from_slice(&valid_after_bytes);
        encoded.extend_from_slice(&valid_before_bytes);
        encoded.extend_from_slice(&payload.nonce.inner());

        keccak256(&encoded)
    }

    /// Compute the full EIP-712 digest
    pub fn compute_digest(&self, payload: &Eip3009Payload) -> [u8; 32] {
        let struct_hash = self.compute_struct_hash(payload);

        // "\x19\x01" || domainSeparator || structHash
        let mut message = Vec::with_capacity(66);
        message.push(0x19);
        message.push(0x01);
        message.extend_from_slice(&self.domain_separator);
        message.extend_from_slice(&struct_hash);

        keccak256(&message)
    }

    /// Verify EIP-712 signature and recover signer
    pub fn verify_signature(
        &self,
        payload: &Eip3009Payload,
        signature_hex: &str,
    ) -> Result<Address, FacilitatorError> {
        let sig_hex = signature_hex.strip_prefix("0x").unwrap_or(signature_hex);
        let sig_bytes = hex::decode(sig_hex)
            .map_err(|e| FacilitatorError::InvalidSignature(format!("Invalid hex: {}", e)))?;

        if sig_bytes.len() != 65 {
            return Err(FacilitatorError::InvalidSignature(format!(
                "Invalid signature length: expected 65, got {}",
                sig_bytes.len()
            )));
        }

        let signature = Signature {
            r: U256::from_big_endian(&sig_bytes[0..32]),
            s: U256::from_big_endian(&sig_bytes[32..64]),
            v: sig_bytes[64] as u64,
        };

        let digest = self.compute_digest(payload);

        let recovered = signature
            .recover(digest)
            .map_err(|e| FacilitatorError::InvalidSignature(format!("Recovery failed: {}", e)))?;

        // Verify the recovered address matches the 'from' address
        if recovered != payload.from.inner() {
            return Err(FacilitatorError::InvalidSignature(format!(
                "Signer mismatch: expected {:?}, recovered {:?}",
                payload.from.inner(),
                recovered
            )));
        }

        Ok(recovered)
    }
}

/// Keccak256 hash helper
fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    let mut output = [0u8; 32];
    hasher.update(data);
    hasher.finalize(&mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::usdc_address;
    use crate::types::{DomainBytes32, DomainEthAddress, DomainUint256};

    #[test]
    fn test_domain_separator_computation() {
        let verifier = Eip712Verifier::new(usdc_address());
        // Domain separator should be deterministic
        let separator = verifier.domain_separator;
        assert_eq!(separator.len(), 32);
    }

    #[test]
    fn test_type_hash_computation() {
        let type_hash = Eip712Verifier::compute_type_hash();
        assert_eq!(type_hash.len(), 32);
    }

    #[test]
    fn test_digest_computation() {
        let verifier = Eip712Verifier::new(usdc_address());

        let payload = Eip3009Payload {
            from: DomainEthAddress("0x1234567890123456789012345678901234567890".parse().unwrap()),
            to: DomainEthAddress("0x0987654321098765432109876543210987654321".parse().unwrap()),
            value: DomainUint256::from(1000000u64),
            valid_after: DomainUint256::from(0u64),
            valid_before: DomainUint256::from(u64::MAX),
            nonce: DomainBytes32([1u8; 32]),
        };

        let digest = verifier.compute_digest(&payload);
        assert_eq!(digest.len(), 32);
    }
}
