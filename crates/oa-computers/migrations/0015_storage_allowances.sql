-- Storage holds are operator allowances, independent of prices and metered bytes.
ALTER TABLE computers.usage_limits
    ADD COLUMN private_storage_bytes bigint CHECK(private_storage_bytes >= 0),
    ADD COLUMN private_checkpoint_count bigint CHECK(private_checkpoint_count >= 0),
    ADD COLUMN storage_bytes_per_boot bigint CHECK(storage_bytes_per_boot > 0),
    ADD COLUMN checkpoints_per_boot bigint CHECK(checkpoints_per_boot > 0),
    ADD CONSTRAINT paired_storage_allowance CHECK(
        (storage_bytes_per_boot IS NULL) = (checkpoints_per_boot IS NULL));

CREATE TABLE computers.account_cleanup (
    owner text PRIMARY KEY,
    requested_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    next_attempt_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    completed_at timestamptz
);

CREATE TABLE computers.storage_resolutions (
    computer text PRIMARY KEY REFERENCES computers.computers(id),
    owner text NOT NULL,
    proof_sha256 text NOT NULL CHECK(proof_sha256 ~ '^[0-9a-f]{64}$'),
    proof jsonb NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TRIGGER immutable_storage_resolution BEFORE UPDATE OR DELETE ON computers.storage_resolutions
FOR EACH ROW EXECUTE FUNCTION computers.keep_usage_event();

CREATE TABLE computers.storage_reservations (
    computer text NOT NULL REFERENCES computers.computers(id),
    owner text NOT NULL,
    boot integer NOT NULL CHECK(boot >= 0),
    create_ordinal integer NOT NULL CHECK(create_ordinal >= 0),
    binding_fingerprint text,
    operation text,
    resource text,
    maximum_bytes bigint CHECK(maximum_bytes > 0),
    maximum_checkpoints bigint CHECK(maximum_checkpoints > 0),
    state text NOT NULL DEFAULT 'held' CHECK(state IN ('held','released')),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    released_at timestamptz,
    PRIMARY KEY(computer,boot,create_ordinal),
    CHECK((state='released') = (released_at IS NOT NULL)),
    CHECK((boot=0) = (create_ordinal=0))
);
CREATE INDEX storage_reservations_by_account ON computers.storage_reservations(owner,state);

-- Past deletion and an absent VM cannot establish private storage cleanup.
INSERT INTO computers.storage_reservations(computer,owner,boot,create_ordinal,binding_fingerprint)
SELECT id,owner,0,0,binding->>'fingerprint' FROM computers.computers;

CREATE FUNCTION computers.require_storage_writer() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF current_setting('openagents.storage_writer',true) IS DISTINCT FROM 'v1'
       AND (EXISTS(SELECT 1 FROM computers.account_cleanup WHERE owner=NEW.owner)
         OR EXISTS(SELECT 1 FROM computers.usage_limits
            WHERE (scope='service' OR subject=NEW.owner)
              AND (private_storage_bytes IS NOT NULL OR private_checkpoint_count IS NOT NULL))) THEN
        RAISE EXCEPTION 'The computer service must be updated before accepting work for this account.';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER computers_require_storage_writer BEFORE INSERT OR UPDATE ON computers.computers
FOR EACH ROW EXECUTE FUNCTION computers.require_storage_writer();
CREATE TRIGGER turns_require_storage_writer BEFORE INSERT OR UPDATE ON computers.turns
FOR EACH ROW EXECUTE FUNCTION computers.require_storage_writer();
CREATE TRIGGER commands_require_storage_writer BEFORE INSERT OR UPDATE ON computers.commands
FOR EACH ROW EXECUTE FUNCTION computers.require_storage_writer();
-- An old create can insert before the policy change and commit afterward.
CREATE CONSTRAINT TRIGGER computers_storage_commit_fence AFTER INSERT OR UPDATE ON computers.computers
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION computers.require_storage_writer();
CREATE CONSTRAINT TRIGGER turns_storage_commit_fence AFTER INSERT OR UPDATE ON computers.turns
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION computers.require_storage_writer();
CREATE CONSTRAINT TRIGGER commands_storage_commit_fence AFTER INSERT OR UPDATE ON computers.commands
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION computers.require_storage_writer();

-- Uncapped old writers remain compatible but leave their storage explicitly unknown.
CREATE FUNCTION computers.capture_legacy_storage() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF current_setting('openagents.storage_writer',true) IS DISTINCT FROM 'v1' THEN
        INSERT INTO computers.storage_reservations(computer,owner,boot,create_ordinal,binding_fingerprint)
        VALUES(NEW.id,NEW.owner,0,0,NEW.binding->>'fingerprint') ON CONFLICT DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER computers_capture_legacy_storage AFTER INSERT OR UPDATE ON computers.computers
FOR EACH ROW EXECUTE FUNCTION computers.capture_legacy_storage();
