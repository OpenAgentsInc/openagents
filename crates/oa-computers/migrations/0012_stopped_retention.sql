-- New computers keep an immutable retention ceiling. Legacy rows retain the
-- existing operator policy rather than inventing an earlier admission promise.
ALTER TABLE computers.computers ADD COLUMN stopped_retention_ms bigint
    CHECK (stopped_retention_ms IS NULL OR stopped_retention_ms >= 0);
ALTER TABLE computers.computers ALTER COLUMN stopped_retention_ms SET DEFAULT 604800000;

CREATE FUNCTION computers.keep_stopped_retention() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.stopped_retention_ms IS DISTINCT FROM OLD.stopped_retention_ms THEN
        RAISE EXCEPTION 'The computer retention period cannot be changed';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER keep_stopped_retention
BEFORE UPDATE OF stopped_retention_ms ON computers.computers
FOR EACH ROW EXECUTE FUNCTION computers.keep_stopped_retention();
