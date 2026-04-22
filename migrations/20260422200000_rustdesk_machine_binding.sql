-- Bind RustDesk IDs + passwords to tenant-owned machines.
--
-- RustDesk becomes the connection transport; Callmor remains the multi-tenant
-- management plane. A tenant user installs Callmor-RustDesk on a machine,
-- notes the 9-digit ID + sets a permanent password, then pastes both into
-- the web portal. Authorized users in that tenant can then click Connect to
-- launch the native Callmor-RustDesk client via `rustdesk://<id>?password=<pwd>`.

ALTER TABLE machines
    ADD COLUMN rustdesk_id TEXT,
    ADD COLUMN rustdesk_password TEXT,
    ADD COLUMN connection_type TEXT NOT NULL DEFAULT 'rustdesk';

-- Unique index on rustdesk_id — a RustDesk ID globally identifies one
-- physical machine on the RustDesk network; we enforce that a given ID is
-- bound to at most one Callmor machine row. NULL allowed (legacy WebRTC
-- machines don't have one). The UNIQUE constraint is partial on non-null.
CREATE UNIQUE INDEX idx_machines_rustdesk_id_unique
    ON machines(rustdesk_id)
    WHERE rustdesk_id IS NOT NULL;

-- Existing machines without a rustdesk_id default to 'webrtc_legacy' so the
-- frontend can tag them appropriately.
UPDATE machines
   SET connection_type = 'webrtc_legacy'
 WHERE rustdesk_id IS NULL;

-- Going forward the default for new rows is 'rustdesk'.
