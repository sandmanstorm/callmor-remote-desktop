-- Ad-hoc (login-less) machines: ScreenConnect-style code+pin flow.
-- Completely separate from `machines` so all existing tenant-scoped queries
-- stay correct and nothing leaks across. Ownership is only transferred when a
-- tenant user calls /machines/claim, at which point we insert into `machines`
-- and delete the adhoc row.

CREATE TABLE adhoc_machines (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- human-friendly 9-char code shown to the user (e.g. "K7F3-9QPZ"). Hyphen is
    -- cosmetic; we store the canonical 8-char form here (base32, no O/0/I/1).
    access_code         TEXT NOT NULL UNIQUE,
    -- 4-digit PIN also shown on the remote screen. Prevents drive-by connect
    -- if someone learns only the code.
    pin                 TEXT NOT NULL,
    -- permanent per-agent token this machine uses over websocket + API
    agent_token         TEXT NOT NULL UNIQUE,
    hostname            TEXT NOT NULL,
    os                  TEXT NOT NULL CHECK (os IN ('linux','windows','macos')),
    last_seen           TIMESTAMPTZ NOT NULL DEFAULT now(),
    online              BOOLEAN NOT NULL DEFAULT false,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- auto-expire: anything idle for 24h is purged by the sweep task
    expires_at          TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '24 hours'),
    -- set when /machines/claim moves this into a tenant; for audit trail
    claimed_at          TIMESTAMPTZ,
    claimed_into_tenant UUID REFERENCES tenants(id) ON DELETE SET NULL
);

CREATE INDEX idx_adhoc_machines_access_code ON adhoc_machines(access_code);
CREATE INDEX idx_adhoc_machines_expires_at ON adhoc_machines(expires_at);
CREATE INDEX idx_adhoc_machines_last_seen ON adhoc_machines(last_seen);
