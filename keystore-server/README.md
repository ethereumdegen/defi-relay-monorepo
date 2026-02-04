# Keystore Server

Secure encrypted blob storage service with SIWE (Sign-In With Ethereum) authentication and optional x402 micropayments.

## Overview

This service stores encrypted backups for wallet addresses. The server never sees decrypted data - it only stores and retrieves encrypted blobs keyed by wallet address. Perfect for backing up API keys, settings, and other sensitive configuration.

**Features:**
- SIWE (Sign-In With Ethereum) authentication
- ECIES encryption (client-side)
- Maximum 10MB storage per wallet
- One backup per wallet (upsert semantics)
- Optional x402 micropayments for backup storage
- Automatic cleanup of expired sessions/challenges

## Tech Stack

- **Framework:** Axum (Rust)
- **Database:** PostgreSQL with SQLx
- **Authentication:** SIWE (EIP-4361)
- **Payments:** x402 protocol (optional)

## Quick Start

```bash
# Clone and enter directory
cd keystore-server

# Copy environment file
cp .env.example .env

# Edit .env with your database URL and domain
vim .env

# Run locally
cargo run

# Or with Docker
docker build -t keystore-server .
docker run -p 3000:3000 --env-file .env keystore-server
```

## API Endpoints

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| GET | `/api/health` | No | Health check |
| POST | `/api/authorize` | No | Request SIWE challenge |
| POST | `/api/authorize/verify` | No | Verify signature, get session token |
| POST | `/api/store_keys` | Yes | Store encrypted backup (may require payment) |
| POST | `/api/get_keys` | Yes | Retrieve encrypted backup |
| POST | `/api/delete_keys` | Yes | Delete backup |
| POST | `/api/logout` | Yes | Invalidate session |

## Authentication Flow

```
1. POST /api/authorize
   Body: { "address": "0x..." }
   Response: { "success": true, "message": "Sign this message...", "nonce": "abc123" }

2. Sign the message with wallet private key

3. POST /api/authorize/verify
   Body: { "address": "0x...", "signature": "0x..." }
   Response: { "success": true, "token": "ks_xxxxx", "expires_at": "..." }

4. Use token for protected endpoints:
   Header: Authorization: Bearer ks_xxxxx
```

---

## Deployment

### DigitalOcean App Platform (Recommended)

The easiest way to deploy is using DigitalOcean App Platform with the included spec file.

#### Option 1: Via Dashboard

1. Go to [DigitalOcean Apps](https://cloud.digitalocean.com/apps)
2. Click "Create App"
3. Connect your GitHub repo
4. Select the `keystore-server` directory
5. App Platform will detect the Dockerfile
6. Add a PostgreSQL database component
7. Configure environment variables (see below)
8. Deploy

#### Option 2: Via CLI

```bash
# Install doctl
brew install doctl  # or apt/snap/etc

# Authenticate
doctl auth init

# Create app from spec
doctl apps create --spec .do/app.yaml

# Or update existing app
doctl apps update <app-id> --spec .do/app.yaml
```

#### Environment Variables for DigitalOcean

In the App Platform dashboard, set these environment variables:

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | Yes | PostgreSQL connection string (auto-set if using DO managed DB) |
| `KEYSTORE_DOMAIN` | Yes | Your domain (e.g., `keystore.yourdomain.com`) |
| `ALLOWED_ORIGINS` | Yes | CORS origins for your frontend |
| `SESSION_TTL_SECS` | No | Session duration (default: 3600) |
| `RUST_LOG` | No | Log level (default: info) |

**For x402 payments (optional):**

| Variable | Required | Description |
|----------|----------|-------------|
| `X402_WALLET_ADDRESS` | Enables x402 | Your wallet to receive payments |
| `X402_FACILITATOR_SIGNER` | Yes* | Facilitator signer address |
| `X402_PAYMENT_TOKEN_ADDRESS` | Yes* | Payment token contract |
| `X402_COST_PER_BACKUP` | No | Cost in wei (default: 1000 tokens) |
| `X402_PAYMENT_NETWORK` | No | Network (default: base-sepolia) |

*Required if `X402_WALLET_ADDRESS` is set

#### Database Setup

1. Add a PostgreSQL database component in App Platform
2. The `DATABASE_URL` will be auto-injected
3. Migrations run automatically on startup

### Other Platforms

#### Railway

```bash
# Install Railway CLI
npm install -g @railway/cli

# Login and init
railway login
railway init

# Add PostgreSQL
railway add --plugin postgresql

# Deploy
railway up
```

Set environment variables in Railway dashboard.

#### Fly.io

```bash
# Install flyctl
curl -L https://fly.io/install.sh | sh

# Launch (creates fly.toml)
fly launch

# Create PostgreSQL
fly postgres create

# Attach database
fly postgres attach <db-name>

# Deploy
fly deploy
```

#### Docker Compose (Self-hosted)

```yaml
version: '3.8'
services:
  keystore:
    build: .
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgres://postgres:password@db:5432/keystore
      - KEYSTORE_DOMAIN=keystore.yourdomain.com
      - ALLOWED_ORIGINS=https://yourdomain.com
    depends_on:
      - db

  db:
    image: postgres:15
    environment:
      - POSTGRES_DB=keystore
      - POSTGRES_PASSWORD=password
    volumes:
      - pgdata:/var/lib/postgresql/data

volumes:
  pgdata:
```

---

## Environment Variables Reference

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | `postgres://user:pass@host:5432/db` |

### Server Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | HTTP port |
| `KEYSTORE_DOMAIN` | `keystore.defirelay.com` | Domain for SIWE messages |
| `ALLOWED_ORIGINS` | `https://stark.defirelay.com` | CORS allowed origins (comma-separated) |

### Security

| Variable | Default | Description |
|----------|---------|-------------|
| `SESSION_TTL_SECS` | `3600` | Session token TTL (1 hour) |
| `CHALLENGE_TTL_SECS` | `300` | SIWE challenge TTL (5 minutes) |
| `MAX_ENCRYPTED_DATA_SIZE` | `10485760` | Max backup size (10MB) |

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
| `X402_COST_PER_BACKUP` | `1000000000000000000000` | Cost per backup in wei |
| `X402_PAYMENT_NETWORK` | `base-sepolia` | Blockchain network |
| `X402_PAYMENT_TOKEN_SYMBOL` | `STARKBOT` | Token symbol |
| `X402_PAYMENT_TOKEN_DECIMALS` | `18` | Token decimals |
| `X402_PAYMENT_TOKEN_NAME` | `StarkBot` | Token name |
| `X402_PAYMENT_TOKEN_VERSION` | `1` | Token version |

---

## Development

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Run with auto-reload
cargo install cargo-watch
cargo watch -x run

# Check compilation
cargo check

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

### Local PostgreSQL

```bash
# With Docker
docker run -d \
  --name keystore-postgres \
  -e POSTGRES_DB=keystore \
  -e POSTGRES_PASSWORD=password \
  -p 5432:5432 \
  postgres:15

# Set DATABASE_URL
export DATABASE_URL=postgres://postgres:password@localhost:5432/keystore
```

---

## Security Notes

1. **HTTPS Required**: SIWE requires HTTPS in production
2. **CORS**: Configure `ALLOWED_ORIGINS` strictly
3. **Domain Matching**: `KEYSTORE_DOMAIN` must match your actual domain
4. **Encryption**: All backup data is encrypted client-side with ECIES
5. **One Wallet, One Backup**: Each wallet can only have one backup (prevents spam)

---

## Monitoring

### Health Check

```bash
curl https://your-domain.com/api/health
# Response: { "status": "healthy", "database": "connected" }
```

### Logs

On DigitalOcean App Platform:
```bash
doctl apps logs <app-id> --follow
```

### Database

The server automatically:
- Runs migrations on startup
- Cleans up expired sessions every hour
- Cleans up expired challenges every hour

---

## Troubleshooting

### "SIWE message domain mismatch"

Set `KEYSTORE_DOMAIN` to match your actual deployed domain.

### "Connection refused" to database

1. Check `DATABASE_URL` is correct
2. Ensure database is accessible from your server
3. Check firewall/security groups

### CORS errors

Add your frontend domain to `ALLOWED_ORIGINS` (comma-separated).

### x402 payment failures

1. Verify `X402_FACILITATOR_SIGNER` is correct
2. Check token contract address
3. Ensure user has sufficient token balance
4. Verify network matches token deployment

---

## License

MIT
