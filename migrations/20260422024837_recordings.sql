-- Per-tenant recording toggle (default OFF, respects consent/privacy by default)
ALTER TABLE tenants ADD COLUMN recording_enabled BOOLEAN NOT NULL DEFAULT false;

-- Session recordings stored in MinIO
CREATE TABLE recordings (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    session_id   UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    machine_id   UUID NOT NULL REFERENCES machines(id) ON DELETE CASCADE,
    object_key   TEXT NOT NULL,        -- MinIO object path
    size_bytes   BIGINT NOT NULL,
    duration_ms  BIGINT,
    content_type TEXT NOT NULL DEFAULT 'video/mp4',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_recordings_tenant ON recordings(tenant_id, created_at DESC);
CREATE INDEX idx_recordings_session ON recordings(session_id);
