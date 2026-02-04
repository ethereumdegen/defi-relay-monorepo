-- Add payment audit trail to backups
ALTER TABLE backups ADD COLUMN last_payment_tx VARCHAR(66);  -- Transaction hash (0x + 64 hex chars)
ALTER TABLE backups ADD COLUMN last_payment_at TIMESTAMPTZ;
