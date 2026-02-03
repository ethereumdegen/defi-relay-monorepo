# Keystore Server

Secure encrypted blob storage service with SIWE (Sign-In With Ethereum) authentication.

## Overview

This service stores encrypted API key backups for wallet addresses. The server never sees decrypted data - it just stores and retrieves encrypted strings keyed by wallet address.

## Tech Stack

- **Framework:** Axum (Rust)
- **Database:** PostgreSQL with SQLx
- **Authentication:** SIWE (Sign-In With Ethereum)
- **Architecture:** Inspired by x402book backend patterns

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/health` | Health check |
| POST | `/api/authorize` | Request SIWE challenge |
| POST | `/api/authorize/verify` | Verify signature, get token |
| POST | `/api/store_keys` | Store encrypted backup (auth required) |
| POST | `/api/get_keys` | Retrieve encrypted backup (auth required) |

## Authentication Flow

```
1. POST /api/authorize { address: "0x..." }
   -> Returns SIWE message to sign

2. Sign message with wallet private key

3. POST /api/authorize/verify { address, signature }
   -> Returns session token (1 hour TTL)

4. Use token for protected endpoints:
   Authorization: Bearer ks_xxxxx
```

## Setup

1. Copy `.env.example` to `.env` and configure
2. Create PostgreSQL database
3. Run: `cargo run`

Migrations run automatically on startup.

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| DATABASE_URL | PostgreSQL connection string | required |
| PORT | Server port | 3000 |
| KEYSTORE_DOMAIN | Domain for SIWE | keystore.defirelay.com |
| SESSION_TTL_SECS | Session token TTL | 3600 (1 hour) |
| CHALLENGE_TTL_SECS | Challenge TTL | 300 (5 min) |
| MAX_ENCRYPTED_DATA_SIZE | Max data size (bytes) | 1048576 (1MB) |
| ALLOWED_ORIGINS | CORS origins | see .env.example |

## Development

```bash
# Run with auto-reload
cargo watch -x run

# Check compilation
cargo check

# Run tests
cargo test
```

## Deployment

Deploy to Railway, Fly.io, or similar. Requires:
- PostgreSQL database
- DNS configured for KEYSTORE_DOMAIN
- HTTPS enabled
