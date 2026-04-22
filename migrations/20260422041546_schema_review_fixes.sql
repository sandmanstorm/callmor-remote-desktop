-- Schema review fixes based on DB audit

-- 1. Drop redundant index (UNIQUE constraint on machines.agent_token already creates one)
DROP INDEX IF EXISTS idx_machines_token;

-- 2. Make tenant cascades consistent — cascade from tenants to users/machines/sessions
--    (so DELETE FROM tenants works cleanly instead of failing with FK violation)
ALTER TABLE users DROP CONSTRAINT users_tenant_id_fkey;
ALTER TABLE users ADD CONSTRAINT users_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE;

ALTER TABLE machines DROP CONSTRAINT machines_tenant_id_fkey;
ALTER TABLE machines ADD CONSTRAINT machines_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE;

ALTER TABLE sessions DROP CONSTRAINT sessions_tenant_id_fkey;
ALTER TABLE sessions ADD CONSTRAINT sessions_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE;

-- sessions → machines / users: cascade from machines; users: SET NULL to preserve audit
ALTER TABLE sessions DROP CONSTRAINT sessions_machine_id_fkey;
ALTER TABLE sessions ADD CONSTRAINT sessions_machine_id_fkey
    FOREIGN KEY (machine_id) REFERENCES machines(id) ON DELETE CASCADE;

-- 3. Missing indexes for common queries
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_tenant_time ON sessions(tenant_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_active ON sessions(machine_id) WHERE ended_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_audit_tenant_type_time ON audit_events(tenant_id, event_type, created_at DESC);

-- 4. CHECK constraints on enum-like TEXT columns (catch typos + garbage)
ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role IN ('member', 'admin', 'owner'));
ALTER TABLE machines ADD CONSTRAINT machines_access_mode_check
    CHECK (access_mode IN ('public', 'restricted'));
ALTER TABLE sessions ADD CONSTRAINT sessions_permission_check
    CHECK (permission IN ('view_only', 'full_control'));
ALTER TABLE invitations ADD CONSTRAINT invitations_role_check
    CHECK (role IN ('member', 'admin', 'owner'));

-- 5. Temporal integrity: ended_at must not precede started_at
ALTER TABLE sessions ADD CONSTRAINT sessions_ended_after_started
    CHECK (ended_at IS NULL OR ended_at >= started_at);

-- 6. Prevent duplicate pending invitations for the same email in the same tenant
CREATE UNIQUE INDEX IF NOT EXISTS idx_invitations_unique_pending
    ON invitations(tenant_id, lower(email))
    WHERE accepted_at IS NULL;

-- 7. One recording per session (matches current upload logic)
ALTER TABLE recordings ADD CONSTRAINT recordings_session_unique UNIQUE (session_id);

-- 8. Tenant slug format: lowercase alphanumeric + dashes only, 1-64 chars
ALTER TABLE tenants ADD CONSTRAINT tenants_slug_format
    CHECK (slug ~ '^[a-z0-9][a-z0-9-]{0,63}$');
