-- Audit log. Created in M0 because write_tx cannot exist without it;
-- M1 owns the audit features that read and populate it meaningfully.
--
-- STRICT rejects values of the wrong type at write time rather than
-- silently coercing them. All ids are UUIDv7 as BLOB(16); all timestamps
-- are ISO-8601 UTC.
CREATE TABLE audit_logs (
    id          BLOB NOT NULL PRIMARY KEY,
    user_id     BLOB,
    action      TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id   BLOB,
    before_json TEXT,
    after_json  TEXT,
    created_at  TEXT NOT NULL
) STRICT;

CREATE INDEX idx_audit_logs_created_at ON audit_logs (created_at);
CREATE INDEX idx_audit_logs_entity ON audit_logs (entity_type, entity_id);
