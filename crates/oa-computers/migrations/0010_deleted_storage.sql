-- Storage reads continue after lifecycle deletion, under independent leases.
CREATE TABLE computers.deleted_storage_jobs (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    computer text NOT NULL REFERENCES computers.computers(id),
    binding_fingerprint text NOT NULL CHECK(binding_fingerprint ~ '^[0-9a-f]{64}$'),
    binding_snapshot jsonb NOT NULL,
    create_operation text NOT NULL,
    resource text NOT NULL,
    deletion_fingerprint text NOT NULL CHECK(deletion_fingerprint ~ '^[0-9a-f]{64}$'),
    context jsonb NOT NULL,
    lease_generation bigint NOT NULL DEFAULT 0,
    lease_holder text,
    lease_until timestamptz,
    next_attempt_at timestamptz NOT NULL DEFAULT now(),
    attempts integer NOT NULL DEFAULT 0 CHECK(attempts >= 0),
    last_status text NOT NULL DEFAULT 'unknown' CHECK(last_status IN ('unknown','done','invalid')),
    UNIQUE(computer, binding_fingerprint, create_operation, resource),
    CHECK(length(create_operation) BETWEEN 1 AND 1024),
    CHECK(length(resource) BETWEEN 1 AND 256)
);
CREATE INDEX deleted_storage_due ON computers.deleted_storage_jobs(next_attempt_at,id);

CREATE FUNCTION computers.keep_deleted_storage_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (NEW.computer,NEW.binding_fingerprint,NEW.binding_snapshot,NEW.create_operation,
        NEW.resource,NEW.deletion_fingerprint,NEW.context) IS DISTINCT FROM
       (OLD.computer,OLD.binding_fingerprint,OLD.binding_snapshot,OLD.create_operation,
        OLD.resource,OLD.deletion_fingerprint,OLD.context) THEN
        RAISE EXCEPTION 'The storage observation assignment cannot be changed.';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER deleted_storage_identity BEFORE UPDATE ON computers.deleted_storage_jobs
FOR EACH ROW EXECUTE FUNCTION computers.keep_deleted_storage_identity();

-- One bounded scan advances independently of provider reads and wraps at EOF.
CREATE TABLE computers.deleted_storage_scan (
    singleton boolean PRIMARY KEY CHECK(singleton),
    after_computer text NOT NULL DEFAULT ''
);
INSERT INTO computers.deleted_storage_scan(singleton) VALUES(true);
