-- Add unique constraint on wallet_id for atomic challenge upsert
-- First delete any duplicate challenges (keep most recent)
DELETE FROM challenges a USING challenges b
WHERE a.id < b.id AND a.wallet_id = b.wallet_id;

-- Add unique constraint
ALTER TABLE challenges ADD CONSTRAINT challenges_wallet_id_unique UNIQUE (wallet_id);

-- Add lowercase check constraints for wallet_id columns
ALTER TABLE backups ADD CONSTRAINT chk_backups_wallet_lowercase
    CHECK (wallet_id = LOWER(wallet_id));
ALTER TABLE sessions ADD CONSTRAINT chk_sessions_wallet_lowercase
    CHECK (wallet_id = LOWER(wallet_id));
ALTER TABLE challenges ADD CONSTRAINT chk_challenges_wallet_lowercase
    CHECK (wallet_id = LOWER(wallet_id));
