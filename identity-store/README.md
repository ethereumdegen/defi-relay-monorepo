# Identity Store

EIP-8004 agent identity JSON storage service with SIWE authentication and optional x402 micropayments.

## Overview

This service stores EIP-8004 identity registration files for AI agents. Agents upload their identity JSON, get a content-addressable URL back, and use that URL when registering on-chain via the **StarkLicense** contract.

**StarkLicense Contract (Base Mainnet):**
[`0xa23a42D266653846e05d8F356a52298844537472`](https://basescan.org/address/0xa23a42d266653846e05d8f356a52298844537472#code) (UUPS Proxy)

**Features:**
- SIWE (Sign-In With Ethereum) authentication
- Stores raw JSON identity documents (up to 256KB)
- Content-addressable: each identity gets a SHA256 hash URL for public lookups
- Public read endpoint (no auth) for agent/registry discovery
- One identity per wallet (upsert semantics)
- Optional x402 micropayments for uploads
- Shares sessions/challenges tables with keystore-server (same database)

## How It Fits Together

```
1. Agent creates IDENTITY.json (EIP-8004 schema)
2. Agent uploads to this service  -->  gets URL: /api/identity/<hash>
3. Agent calls StarkLicense.register(url) on Base  -->  burns 1000 STARKBOT, mints NFT
4. Other agents/registries fetch identity via the public URL
```

The StarkLicense contract at [`0xa23a42D266653846e05d8F356a52298844537472`](https://basescan.org/address/0xa23a42d266653846e05d8f356a52298844537472#code) stores the identity URL on-chain as the agent's registration URI. This service is the backing store for those URLs.

## Tech Stack

- **Framework:** Axum (Rust)
- **Database:** PostgreSQL with SQLx (shares DB with keystore-server)
- **Authentication:** SIWE (EIP-4361)
- **Payments:** x402 protocol (optional)

## Quick Start

```bash
cd identity-store

# Copy environment file
cp .env.example .env

# Edit .env — point DATABASE_URL to same DB as keystore-server
vim .env

# Run locally
cargo run

# Or with Docker
docker build -t identity-store .
docker run -p 8081:8080 --env-file .env identity-store
```

## API Endpoints

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| GET | `/api/health` | No | Health check |
| POST | `/api/authorize` | No | Request SIWE challenge |
| POST | `/api/authorize/verify` | No | Verify signature, get session token |
| POST | `/api/store_identity` | Yes (+ x402) | Store/update identity JSON |
| POST | `/api/get_identity` | Yes | Retrieve your own identity |
| GET | `/api/identity/:hash` | **No (public)** | Get any identity by content hash |
| POST | `/api/delete_identity` | Yes | Delete your identity |
| POST | `/api/logout` | Yes | Invalidate session |

## Authentication Flow

```
1. POST /api/authorize
   Body: { "address": "0x..." }
   Response: { "success": true, "message": "Sign this message...", "nonce": "abc123" }

2. Sign the message with wallet private key

3. POST /api/authorize/verify
   Body: { "address": "0x...", "signature": "0x..." }
   Response: { "success": true, "token": "id_xxxxx", "expires_at": "..." }

4. Use token for protected endpoints:
   Header: Authorization: Bearer id_xxxxx
```

## Store & Retrieve Identity

### Store

```bash
curl -X POST https://identity.defirelay.com/api/store_identity \
  -H "Authorization: Bearer id_xxxxx" \
  -H "Content-Type: application/json" \
  -d '{
    "identity_json": "{\"type\":\"https://eips.ethereum.org/EIPS/eip-8004#registration-v1\",\"name\":\"TradeBot\",\"description\":\"Autonomous DeFi trading agent\",\"active\":true}"
  }'
```

Response:
```json
{
  "success": true,
  "message": "Identity stored",
  "content_hash": "a1b2c3d4...",
  "url": "https://identity.defirelay.com/api/identity/a1b2c3d4...",
  "updated_at": "2025-02-08T..."
}
```

### Public Read (no auth)

```bash
curl https://identity.defirelay.com/api/identity/a1b2c3d4...
```

This is the URL you pass to `StarkLicense.register(url)` on Base.

## Shared Database with Keystore Server

This service is designed to share a PostgreSQL database with keystore-server. The `sessions` and `challenges` tables are identical across both services — a session token created by either service works for both.

**Table ownership:**
| Table | Owner | Notes |
|-------|-------|-------|
| `backups` | keystore-server | Encrypted key storage |
| `identities` | identity-store | Identity JSON documents |
| `sessions` | shared | Auth sessions (same schema) |
| `challenges` | shared | SIWE challenges (same schema) |

Migrations use `CREATE TABLE IF NOT EXISTS` so they're safe to run against a DB that already has the shared tables.

## Environment Variables

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string (same as keystore-server) | `postgres://user:pass@host:5432/db` |

### Server Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8080` | HTTP port |
| `IDENTITY_DOMAIN` | `identity.defirelay.com` | Domain for SIWE messages |
| `PUBLIC_URL` | `https://{IDENTITY_DOMAIN}` | Base URL for identity URLs in responses |
| `ALLOWED_ORIGINS` | `https://stark.defirelay.com` | CORS allowed origins (comma-separated) |

### Security

| Variable | Default | Description |
|----------|---------|-------------|
| `SESSION_TTL_SECS` | `3600` | Session token TTL (1 hour) |
| `CHALLENGE_TTL_SECS` | `300` | SIWE challenge TTL (5 minutes) |
| `MAX_IDENTITY_JSON_SIZE` | `262144` | Max identity size (256KB) |

### Logging

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level (error/warn/info/debug/trace) |

### x402 Payments (Optional)

| Variable | Default | Description |
|----------|---------|-------------|
| `X402_WALLET_ADDRESS` | - | Enables x402 if set |
| `X402_FACILITATOR_URL` | `https://pay2.defirelay.com` | Facilitator endpoint |
| `X402_FACILITATOR_SIGNER` | - | Facilitator signer (required with wallet) |
| `X402_PAYMENT_TOKEN_ADDRESS` | - | Token contract (required with wallet) |
| `X402_COST_PER_UPLOAD` | `1000000000000000000000` | Cost per upload in wei |
| `X402_PAYMENT_NETWORK` | `base-sepolia` | Blockchain network |
| `X402_PAYMENT_TOKEN_SYMBOL` | `STARKBOT` | Token symbol |
| `X402_PAYMENT_TOKEN_DECIMALS` | `18` | Token decimals |
| `X402_PAYMENT_TOKEN_NAME` | `StarkBot` | Token name |
| `X402_PAYMENT_TOKEN_VERSION` | `1` | Token version |

## StarkLicense Contract

The on-chain component that consumes identity URLs from this service.

| Detail | Value |
|--------|-------|
| **Contract** | [`0xa23a42D266653846e05d8F356a52298844537472`](https://basescan.org/address/0xa23a42d266653846e05d8f356a52298844537472#code) |
| **Network** | Base Mainnet (Chain ID: 8453) |
| **Standard** | EIP-8004 Identity Registry |
| **Token** | ERC-721 (STARKBOT Agent License / STARK-LICENSE) |
| **Registration Fee** | 1,000 STARKBOT (burned) |
| **Payment Token** | STARKBOT (`0x587Cd533F418825521f3A1daa7CCd1E7339A1B07`) |

## Development

```bash
# Run with auto-reload
cargo install cargo-watch
cargo watch -x run

# Check compilation
cargo check

# Run migrations manually
cargo run --bin migrate
```

## License

MIT
