# defi-relay-quoter

Pay-per-use [0x Swap API](https://0x.org/docs/api) proxy powered by the [x402](https://www.x402.org) payment protocol. Clients pay a small USDC fee per request to get token swap prices and quotes across multiple EVM chains.

## How it works

```
Client                    Quoter                   Facilitator         0x API
  │                         │                          │                  │
  │── GET /swap/... ───────>│                          │                  │
  │   (no X-PAYMENT)        │                          │                  │
  │<── 402 Payment Required │                          │                  │
  │   (PAYMENT-REQUIRED hdr)│                          │                  │
  │                         │                          │                  │
  │── GET /swap/... ───────>│                          │                  │
  │   (X-PAYMENT: <base64>) │── verify ───────────────>│                  │
  │                         │<── is_valid: true ───────│                  │
  │                         │── settle ───────────────>│                  │
  │                         │<── success: true ────────│                  │
  │                         │── forward request ──────────────────────────>│
  │<── 200 + quote data ────│<── quote response ──────────────────────────│
```

1. A request without payment gets back `402 Payment Required` with a `PAYMENT-REQUIRED` header describing what to pay
2. The client constructs an [EIP-3009](https://eips.ethereum.org/EIPS/eip-3009) `transferWithAuthorization` signature for USDC on Base and sends it as a base64-encoded `X-PAYMENT` header
3. The quoter verifies the payment via the x402 facilitator, then settles (collects) the funds **before** proxying the request to 0x
4. The 0x response is returned to the client along with a `PAYMENT-RESPONSE` header confirming success

## Endpoints

### Swap (paid, protected by x402 middleware)

| Endpoint | Cost | Description |
|---|---|---|
| `GET /swap/permit2/price` | $0.0005 | Indicative price (lightweight) |
| `GET /swap/permit2/quote` | $0.001 | Full quote with transaction data |
| `GET /swap/allowance-holder/price` | $0.0005 | Indicative price (recommended) |
| `GET /swap/allowance-holder/quote` | $0.001 | Full quote with transaction data (recommended) |

**Required query parameters:** `chainId`, `sellToken`, `buyToken`, `sellAmount` (or `buyAmount`), `taker`

**Optional:** `slippageBps`, `excludedSources`, `includedSources`

### Public

| Endpoint | Description |
|---|---|
| `GET /` | Usage instructions |
| `GET /health` | Health check |
| `GET /agent.json` | [EIP-8004](https://eips.ethereum.org/EIPS/eip-8004) agent metadata |
| `GET /.well-known/x402` | x402 discovery document |

### Permit2 vs AllowanceHolder

- **AllowanceHolder** (recommended): Single signature, better UX, lower gas
- **Permit2**: Universal standard, shared approvals across apps, requires two signatures

## Supported chains

Ethereum (1), Base (8453), Arbitrum (42161), Optimism (10), Polygon (137), Avalanche (43114), BSC (56)

## Configuration

Copy `.env.example` to `.env` and configure:

| Variable | Required | Default | Description |
|---|---|---|---|
| `WALLET_ADDRESS` | Yes | | Ethereum address to receive USDC payments |
| `FACILITATOR_URL` | Yes | | x402 facilitator URL (e.g. `https://facilitator.x402.org`) |
| `ZEROX_API_KEY` | Yes | | Your [0x API key](https://0x.org/docs/introduction/getting-started) |
| `PORT` | No | `8080` | Server port |
| `COST_PER_PRICE` | No | `500` | Cost per price request in raw USDC (6 decimals, 500 = $0.0005) |
| `COST_PER_QUOTE` | No | `1000` | Cost per quote request in raw USDC (6 decimals, 1000 = $0.001) |
| `BASE_URL` | No | | Base URL for discovery documents |
| `ZEROX_BASE_URL` | No | `https://api.0x.org` | 0x API base URL |

## Run locally

```bash
cp .env.example .env
# edit .env with your values

cargo run
```

The server starts on `http://localhost:8080`.

## Docker

```bash
docker build -t defi-relay-quoter .
docker run -p 8080:8080 --env-file .env defi-relay-quoter
```

## Example request

```bash
# First request without payment returns 402
curl -i 'http://localhost:8080/swap/allowance-holder/quote?chainId=1&sellToken=0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee&buyToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&sellAmount=1000000000000000000&taker=0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045'
# HTTP/1.1 402 Payment Required
# payment-required: <base64-encoded payment requirements>
```

An x402-compatible client will automatically read the `PAYMENT-REQUIRED` header, construct a signed USDC transfer authorization, and retry with the `X-PAYMENT` header.

## Architecture

```
src/
├── main.rs                    # Server setup, routing
├── config.rs                  # Environment configuration
├── error.rs                   # Error types (AppError)
├── handlers/
│   ├── quote.rs               # Swap price/quote handlers (proxy to 0x)
│   ├── agent_info.rs          # EIP-8004 agent metadata
│   └── x402_discovery.rs      # .well-known/x402 discovery document
├── middleware/
│   └── x402.rs                # x402 payment verification & settlement
├── models/
│   ├── domains.rs             # Domain types (EthAddress, Uint256, Bytes32)
│   └── x402.rs                # x402 protocol types (PaymentRequired, PaymentPayload, etc.)
└── services/
    ├── facilitator.rs         # Facilitator client (verify + settle with retries)
    ├── zerox_client.rs        # 0x API client
    └── nonce_tracker.rs       # Replay attack prevention (in-memory TTL cache)
```

## Security

- **Nonce tracking**: Each payment nonce is tracked in an in-memory cache (10 min TTL, 100k capacity) to prevent replay attacks
- **Settle-before-serve**: Payment is settled (funds collected) before the upstream request is processed
- **Exponential backoff**: Transient facilitator errors are retried up to 3 times with exponential backoff
