-- Invitations: users invited to join a tenant
CREATE TABLE invitations (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    email       TEXT NOT NULL,
    role        TEXT NOT NULL DEFAULT 'member',
    token_hash  TEXT NOT NULL UNIQUE,
    invited_by  UUID NOT NULL REFERENCES users(id),
    expires_at  TIMESTAMPTZ NOT NULL,
    accepted_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_invitations_tenant ON invitations(tenant_id);
CREATE INDEX idx_invitations_email ON invitations(email);

-- Per-machine access control (only consulted when machine.access_mode = 'restricted')
CREATE TABLE machine_access (
    tenant_id   UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    machine_id  UUID NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, machine_id, user_id)
);

CREATE INDEX idx_machine_access_user ON machine_access(user_id);

-- Machines: add access_mode (public or restricted)
ALTER TABLE machines ADD COLUMN access_mode TEXT NOT NULL DEFAULT 'public';
