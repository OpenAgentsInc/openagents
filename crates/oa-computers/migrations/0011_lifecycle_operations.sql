-- Retain lifecycle mutations independently of a provider's idempotency expiry.
CREATE TABLE computers.lifecycle_operations (
    operation text PRIMARY KEY,
    resource text NOT NULL,
    action text NOT NULL CHECK (action IN ('suspend', 'wake')),
    disposition text NOT NULL DEFAULT 'requested'
        CHECK (disposition IN ('requested', 'accepted', 'rejected')),
    provider_operation text,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((disposition = 'accepted') = (provider_operation IS NOT NULL))
);

CREATE FUNCTION computers.keep_lifecycle_operation_identity() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.operation <> OLD.operation OR NEW.resource <> OLD.resource
        OR NEW.action <> OLD.action OR NEW.created_at <> OLD.created_at
        OR (OLD.disposition <> 'requested' AND NEW IS DISTINCT FROM OLD)
    THEN
        RAISE EXCEPTION 'A lifecycle operation cannot change identity or outcome';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER keep_lifecycle_operation_identity
BEFORE UPDATE ON computers.lifecycle_operations
FOR EACH ROW EXECUTE FUNCTION computers.keep_lifecycle_operation_identity();
