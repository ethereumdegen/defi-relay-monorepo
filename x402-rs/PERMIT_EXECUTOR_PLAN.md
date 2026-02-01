# PermitExecutor Implementation Plan

## Problem Summary

The facilitator's permit scheme uses Multicall3 to batch `permit()` + `transferFrom()`.
Inside Multicall3 calls, `msg.sender = Multicall3`, not the facilitator, causing
`transferFrom()` to fail with `ERC20InsufficientAllowance`.

## Solution

Deploy `PermitExecutor` contract owned by the facilitator. Permits use `PermitExecutor`
as the spender instead of the facilitator EOA.

---

## Phase 1: Deploy Contract

Deploy `contracts/PermitExecutor.sol` on each chain with `owner = facilitator EOA`.

| Chain | Facilitator EOA | PermitExecutor Address |
|-------|-----------------|------------------------|
| Base  | 0x7eD34056DE24DEed07C2b78712Ae491f7072C981 | TBD |
| Ethereum | 0x7eD34056DE24DEed07C2b78712Ae491f7072C981 | TBD |

---

## Phase 2: Config Updates

### Add PermitExecutor addresses to facilitator config

The facilitator needs to know the PermitExecutor address for each chain. Add to config:

```toml
[permit_executor]
base = "0x..."
ethereum = "0x..."
```

Or via environment variables:
```
PERMIT_EXECUTOR_BASE=0x...
PERMIT_EXECUTOR_ETHEREUM=0x...
```

---

## Phase 3: Rust Code Changes

### 3.1 Add PermitExecutor interface

**File: `src/scheme/v1_eip155_permit/mod.rs`**

Add after the existing `sol!` macro for `Permit`:

```rust
sol! {
    /// PermitExecutor contract interface
    #[allow(dead_code)]
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
```

### 3.2 Update V1Eip155PermitFacilitator struct

**File: `src/scheme/v1_eip155_permit/mod.rs`**

```rust
pub struct V1Eip155PermitFacilitator<P> {
    provider: P,
    permit_executor: Address,  // NEW: PermitExecutor contract address
}

impl<P> V1Eip155PermitFacilitator<P> {
    pub fn new(provider: P, permit_executor: Address) -> Self {
        Self { provider, permit_executor }
    }
}
```

### 3.3 Update builder to accept PermitExecutor address

**File: `src/scheme/v1_eip155_permit/mod.rs`**

Update `X402SchemeFacilitatorBuilder` impl to extract PermitExecutor from config:

```rust
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
        let permit_executor = config
            .as_ref()
            .and_then(|c| c.get("permitExecutor"))
            .and_then(|v| v.as_str())
            .ok_or("permitExecutor address required in config")?
            .parse::<Address>()?;

        Ok(Box::new(V1Eip155PermitFacilitator::new(provider, permit_executor)))
    }
}
```

### 3.4 Update assert_valid_payment - validate spender

**File: `src/scheme/v1_eip155_permit/mod.rs`**

Change spender validation to check against PermitExecutor (not facilitator signers):

```rust
async fn assert_valid_payment<'a, P: Provider>(
    provider: &'a P,
    chain: &Eip155ChainReference,
    permit_executor: &Address,  // CHANGED: was facilitator_signers
    payload: &types::PaymentPayload,
    requirements: &types::PaymentRequirements,
) -> Result<...> {
    // ...

    // Verify spender is the PermitExecutor
    if authorization.spender != *permit_executor {
        return Err(PaymentVerificationError::InvalidSpender.into());
    }

    // ...
}
```

### 3.5 Update verify_payment - use PermitExecutor for simulation

**File: `src/scheme/v1_eip155_permit/mod.rs`**

Replace Multicall3 simulation with PermitExecutor call simulation:

```rust
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
        StructuredSignature::EIP6492 { factory, factory_calldata, inner, .. } => {
            // Simulate executeCounterfactualPermitTransfer
            executor.executeCounterfactualPermitTransfer(
                factory,
                factory_calldata,
                *contract.address(),
                payment.owner,
                payment.value,
                U256::from(payment.deadline.as_secs()),
                inner,
                pay_to,
            ).call().await?;
        }
        StructuredSignature::EIP1271(sig) => {
            // Simulate executePermitTransferWithSignature
            executor.executePermitTransferWithSignature(
                *contract.address(),
                payment.owner,
                payment.value,
                U256::from(payment.deadline.as_secs()),
                sig,
                pay_to,
            ).call().await?;
        }
        StructuredSignature::EOA(sig) => {
            let v = 27 + (sig.v() as u8);
            let r = B256::from(sig.r());
            let s = B256::from(sig.s());

            // Simulate executePermitTransfer
            executor.executePermitTransfer(
                *contract.address(),
                payment.owner,
                payment.value,
                U256::from(payment.deadline.as_secs()),
                v,
                r,
                s,
                pay_to,
            ).call().await?;
        }
    }

    Ok(payer)
}
```

### 3.6 Update settle_payment - use PermitExecutor for settlement

**File: `src/scheme/v1_eip155_permit/mod.rs`**

Replace Multicall3 transaction with PermitExecutor call:

```rust
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

    let calldata: Bytes = match signed_message.signature {
        StructuredSignature::EIP6492 { factory, factory_calldata, inner, .. } => {
            IPermitExecutor::executeCounterfactualPermitTransferCall {
                factory,
                factoryCalldata: factory_calldata,
                token: *contract.address(),
                tokenOwner: payment.owner,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                signature: inner,
                payTo: pay_to,
            }.abi_encode().into()
        }
        StructuredSignature::EIP1271(sig) => {
            IPermitExecutor::executePermitTransferWithSignatureCall {
                token: *contract.address(),
                tokenOwner: payment.owner,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                signature: sig,
                payTo: pay_to,
            }.abi_encode().into()
        }
        StructuredSignature::EOA(sig) => {
            let v = 27 + (sig.v() as u8);
            let r = B256::from(sig.r());
            let s = B256::from(sig.s());

            IPermitExecutor::executePermitTransferCall {
                token: *contract.address(),
                tokenOwner: payment.owner,
                value: payment.value,
                deadline: U256::from(payment.deadline.as_secs()),
                v,
                r,
                s,
                payTo: pay_to,
            }.abi_encode().into()
        }
    };

    let receipt = Eip155MetaTransactionProvider::send_transaction(
        provider,
        MetaTransaction {
            to: *permit_executor,
            calldata,
            confirmations: 1,
        },
    ).await?;

    if receipt.status() {
        Ok(receipt.transaction_hash)
    } else {
        Err(Eip155ExactError::TransactionReverted(receipt.transaction_hash))
    }
}
```

### 3.7 Update supported() - return PermitExecutor as signer

**File: `src/scheme/v1_eip155_permit/mod.rs`**

Update the `supported()` method to return PermitExecutor address:

```rust
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
        signers.insert(chain_id, vec![self.permit_executor.to_string()]);
        signers
    };
    Ok(proto::SupportedResponse {
        kinds,
        extensions: Vec::new(),
        signers,
    })
}
```

---

## Phase 4: V2 Permit Scheme

Apply the same changes to `src/scheme/v2_eip155_permit/mod.rs`.

---

## Phase 5: Testing

1. Deploy PermitExecutor on Base testnet (Base Sepolia)
2. Update facilitator config with testnet address
3. Test permit verification with new spender
4. Test permit settlement
5. Test counterfactual wallet flow (EIP-6492)

---

## Client Impact

Clients using the permit scheme must:

1. Fetch `/supported` endpoint to get PermitExecutor address
2. Use PermitExecutor address as `spender` when signing permits (not facilitator EOA)

**Before:**
```json
{
  "spender": "0x7eD34056DE24DEed07C2b78712Ae491f7072C981"  // facilitator EOA
}
```

**After:**
```json
{
  "spender": "0x<PermitExecutor address>"  // from /supported endpoint
}
```

---

## Rollout Strategy

1. Deploy PermitExecutor contracts
2. Deploy updated facilitator with feature flag (support both old and new)
3. Notify clients to update spender addresses
4. After transition period, remove old Multicall3 code path
