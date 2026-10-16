CREATE TABLE IF NOT EXISTS blob_gc (
    object TEXT PRIMARY KEY,
    queued_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS blob_gc_age ON blob_gc(queued_at);
CREATE OR REPLACE FUNCTION coder_sync_retire_blob() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' OR OLD.object IS DISTINCT FROM NEW.object THEN
        INSERT INTO blob_gc(object) VALUES(OLD.object) ON CONFLICT DO NOTHING;
    END IF;
    RETURN OLD;
END $$;
CREATE TRIGGER retire_blob AFTER DELETE OR UPDATE OF object ON blobs
    FOR EACH ROW EXECUTE FUNCTION coder_sync_retire_blob();
INSERT INTO coder_sync_schema(version) VALUES(5) ON CONFLICT DO NOTHING;
