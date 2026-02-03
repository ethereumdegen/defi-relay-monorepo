-- Keystore Server Initial Schema
-- Encrypted blob storage with SIWE authentication

-- Backups table: stores encrypted API key backups
CREATE TABLE IF NOT EXISTS backups (
    id SERIAL PRIMARY KEY,
    wallet_id VARCHAR(42) NOT NULL UNIQUE,  -- Ethereum address (0x..., lowercase)
    encrypted_data TEXT NOT NULL,            -- Hex-encoded ECIES encrypted blob
    key_count INTEGER NOT NULL DEFAULT 0,    -- Informational only
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_backups_wallet_id ON backups(wallet_id);

-- Sessions table: short-lived session tokens
CREATE TABLE IF NOT EXISTS sessions (
    id SERIAL PRIMARY KEY,
    token VARCHAR(64) NOT NULL UNIQUE,
    wallet_id VARCHAR(42) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);

-- Challenges table: pending SIWE challenges
CREATE TABLE IF NOT EXISTS challenges (
    id SERIAL PRIMARY KEY,
    wallet_id VARCHAR(42) NOT NULL,
    nonce VARCHAR(32) NOT NULL,
    message TEXT NOT NULL,              -- Full SIWE message to sign
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_challenges_wallet ON challenges(wallet_id);
CREATE INDEX IF NOT EXISTS idx_challenges_expires ON challenges(expires_at);
