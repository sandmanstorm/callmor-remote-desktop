-- Platform-level super-admin flag (cross-tenant admin access)
ALTER TABLE users ADD COLUMN is_superadmin BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX idx_users_superadmin ON users(is_superadmin) WHERE is_superadmin = true;
