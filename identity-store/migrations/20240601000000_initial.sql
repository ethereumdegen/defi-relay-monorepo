-- Identity Store Initial Schema
-- EIP-8004 agent identity JSON storage with SIWE authentication
-- Shares sessions/challenges tables with keystore-server (same DB)

-- Identities table: stores identity JSON documents
CREATE TABLE IF NOT EXISTS identities (
    id SERIAL PRIMARY KEY,
    wallet_id VARCHAR(42) NOT NULL UNIQUE,    -- Ethereum address (0x..., lowercase), one identity per wallet
    identity_json TEXT NOT NULL,              -- Raw JSON identity document
    content_hash VARCHAR(64) NOT NULL UNIQUE, -- SHA256 hash of identity_json (for public lookups)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_payment_tx VARCHAR(66),             -- x402 payment tx hash
    last_payment_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_identities_wallet_id ON identities(wallet_id);
CREATE INDEX IF NOT EXISTS idx_identities_content_hash ON identities(content_hash);

-- Shared sessions table (created by keystore-server, safe to re-declare)
CREATE TABLE IF NOT EXISTS sessions (
    id SERIAL PRIMARY KEY,
    token VARCHAR(64) NOT NULL UNIQUE,
    wallet_id VARCHAR(42) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);

-- Shared challenges table (created by keystore-server, safe to re-declare)
CREATE TABLE IF NOT EXISTS challenges (
    id SERIAL PRIMARY KEY,
    wallet_id VARCHAR(42) NOT NULL UNIQUE,
    nonce VARCHAR(32) NOT NULL,
    message TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_challenges_wallet ON challenges(wallet_id);
CREATE INDEX IF NOT EXISTS idx_challenges_expires ON challenges(expires_at);

-- Lowercase enforcement (only for identities - sessions/challenges constraints already exist from keystore)
DO $$ BEGIN
    ALTER TABLE identities ADD CONSTRAINT chk_identities_wallet_lowercase
        CHECK (wallet_id = LOWER(wallet_id));
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;
