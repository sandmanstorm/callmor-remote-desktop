-- Platform-wide audit log
CREATE TABLE audit_events (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    UUID REFERENCES tenants(id) ON DELETE CASCADE,
    actor_id     UUID REFERENCES users(id) ON DELETE SET NULL,
    actor_email  TEXT,
    event_type   TEXT NOT NULL,
    entity_type  TEXT,
    entity_id    UUID,
    metadata     JSONB NOT NULL DEFAULT '{}'::jsonb,
    ip_address   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_tenant ON audit_events(tenant_id, created_at DESC);
CREATE INDEX idx_audit_event_type ON audit_events(event_type);
CREATE INDEX idx_audit_created ON audit_events(created_at DESC);
