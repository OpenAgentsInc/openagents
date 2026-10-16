-- A resource and an operation are meaningful only in this saved deployment.
-- Old rows stay unassigned until the operator's explicit legacy mapping pins
-- them. Do not infer their deployment from a changed process default.
ALTER TABLE computers.computers ADD COLUMN binding jsonb;
ALTER TABLE computers.computers ADD COLUMN binding_lease_until timestamptz;
ALTER TABLE computers.computers ADD CONSTRAINT computers_binding_shape CHECK (
    binding IS NULL OR COALESCE((
        jsonb_typeof(binding) = 'object'
        AND binding->>'schema' = 'openagents.computers.binding.v1'
        AND binding->>'owner' = owner
        AND binding->>'driver' = driver
        AND binding->>'size' = size
        AND length(binding->>'deployment') > 0
        AND binding->>'fingerprint' ~ '^[0-9a-f]{64}$'
    ), false)
);
CREATE INDEX computers_active_deployment
    ON computers.computers ((binding->>'deployment')) WHERE NOT done;

-- Once assigned, even a stale service revision cannot move a computer.
CREATE FUNCTION computers.keep_deployment_binding() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.binding IS NOT NULL AND NEW.binding IS DISTINCT FROM OLD.binding THEN
        RAISE EXCEPTION 'The computer service assignment cannot be changed.';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER computers_keep_deployment_binding
    BEFORE UPDATE OF binding ON computers.computers
    FOR EACH ROW EXECUTE FUNCTION computers.keep_deployment_binding();

-- An overlapping old worker does not know the saved operation namespace.
-- It cannot claim a bound row: only this protocol sets both lease timestamps.
-- Matching the new expiry also fences workers that reuse a holder name.
CREATE FUNCTION computers.require_deployment_worker() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.binding IS NOT NULL AND NEW.lease_holder IS NOT NULL
       AND NEW.binding_lease_until IS DISTINCT FROM NEW.lease_until THEN
        RETURN NULL;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER computers_require_deployment_worker
    BEFORE UPDATE OF lease_holder ON computers.computers
    FOR EACH ROW EXECUTE FUNCTION computers.require_deployment_worker();
