use ethers::types::{Address, U256};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Wrapper for Ethereum addresses with hex string serialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainEthAddress(pub Address);

impl DomainEthAddress {
    pub fn inner(&self) -> Address {
        self.0
    }
}

impl From<Address> for DomainEthAddress {
    fn from(addr: Address) -> Self {
        DomainEthAddress(addr)
    }
}

impl Serialize for DomainEthAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:?}", self.0))
    }
}

impl<'de> Deserialize<'de> for DomainEthAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let addr: Address = s.parse().map_err(serde::de::Error::custom)?;
        Ok(DomainEthAddress(addr))
    }
}

/// Wrapper for 32-byte values with hex string serialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainBytes32(pub [u8; 32]);

impl DomainBytes32 {
    pub fn inner(&self) -> [u8; 32] {
        self.0
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, &'static str> {
        if slice.len() != 32 {
            return Err("Invalid length for bytes32");
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(slice);
        Ok(DomainBytes32(arr))
    }
}

impl Serialize for DomainBytes32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(self.0)))
    }
}

impl<'de> Deserialize<'de> for DomainBytes32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
        DomainBytes32::from_slice(&bytes).map_err(serde::de::Error::custom)
    }
}

/// Wrapper for U256 with decimal string serialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainUint256(pub U256);

impl DomainUint256 {
    pub fn inner(&self) -> U256 {
        self.0
    }
}

impl From<U256> for DomainUint256 {
    fn from(val: U256) -> Self {
        DomainUint256(val)
    }
}

impl From<u64> for DomainUint256 {
    fn from(val: u64) -> Self {
        DomainUint256(U256::from(val))
    }
}

impl Serialize for DomainUint256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for DomainUint256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let val = U256::from_dec_str(&s).map_err(serde::de::Error::custom)?;
        Ok(DomainUint256(val))
    }
}
