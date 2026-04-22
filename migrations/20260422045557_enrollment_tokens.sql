-- Per-tenant enrollment token. Baked into installers so agents can self-register.

ALTER TABLE tenants ADD COLUMN enrollment_token TEXT;

-- Backfill existing tenants with a random token (36 chars: "cle_" + 32 hex)
UPDATE tenants SET enrollment_token = 'cle_' || encode(gen_random_bytes(16), 'hex')
WHERE enrollment_token IS NULL;

ALTER TABLE tenants ALTER COLUMN enrollment_token SET NOT NULL;
ALTER TABLE tenants ADD CONSTRAINT tenants_enrollment_token_key UNIQUE (enrollment_token);
