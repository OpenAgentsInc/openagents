-- Usage evidence is independent of prices and the purchased compute ledger.
-- Never infer billable seconds from a lifecycle timestamp.
ALTER TABLE computers.computers ADD COLUMN usage_revision bigint NOT NULL DEFAULT -1;
ALTER TABLE computers.computers ADD COLUMN usage_intent_generation bigint NOT NULL DEFAULT -1;

-- An overlapping old service must not allocate before recording a reservation.
CREATE FUNCTION computers.require_usage_writer() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.usage_revision <> NEW.revision OR NEW.usage_intent_generation <> NEW.intent_generation THEN
            RETURN NULL;
        END IF;
    ELSE
        IF (NEW.record IS DISTINCT FROM OLD.record AND NEW.usage_revision <> NEW.revision)
           OR (NEW.intent_generation <> OLD.intent_generation
               AND NEW.usage_intent_generation <> NEW.intent_generation) THEN
            RETURN NULL;
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER computers_require_usage_writer BEFORE INSERT OR UPDATE ON computers.computers
FOR EACH ROW EXECUTE FUNCTION computers.require_usage_writer();

CREATE TABLE computers.usage_events (
    sequence bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    computer text NOT NULL REFERENCES computers.computers(id),
    owner text NOT NULL,
    revision bigint NOT NULL,
    intent_generation bigint NOT NULL,
    kind text NOT NULL CHECK (kind IN ('accepted', 'observed', 'meter', 'resolution')),
    event_key text NOT NULL,
    deployment text,
    driver text NOT NULL,
    size text NOT NULL,
    shape jsonb,
    resource text,
    operation text,
    turn_generation bigint NOT NULL DEFAULT 0,
    boot integer,
    phase text NOT NULL,
    desired text NOT NULL,
    observed_ms bigint NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    evidence jsonb NOT NULL,
    UNIQUE(computer, event_key)
);
CREATE INDEX usage_by_account ON computers.usage_events(owner, sequence);

CREATE FUNCTION computers.keep_usage_event() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Usage evidence cannot be changed.';
END;
$$;
CREATE TRIGGER immutable_usage_event BEFORE UPDATE OR DELETE ON computers.usage_events
FOR EACH ROW EXECUTE FUNCTION computers.keep_usage_event();

-- These are operator allocation limits, not a price book or customer charges.
CREATE TABLE computers.usage_limits (
    scope text NOT NULL CHECK(scope IN ('account', 'service')),
    subject text NOT NULL,
    computer_count bigint CHECK(computer_count >= 0),
    running_vcpus bigint CHECK(running_vcpus >= 0),
    running_memory_mib bigint CHECK(running_memory_mib >= 0),
    boot_cpu_ms bigint CHECK(boot_cpu_ms >= 0),
    turn_seconds integer NOT NULL DEFAULT 3600 CHECK(turn_seconds BETWEEN 1 AND 3600),
    stopped_retention_seconds bigint CHECK(stopped_retention_seconds >= 0),
    CHECK(scope = 'account' OR subject = ''),
    PRIMARY KEY(scope, subject)
);

-- One allocation allowance per boot. An unresolved allowance never expires.
-- The allowance is the boot's maximum CPU milliseconds, not metered usage.
CREATE TABLE computers.usage_reservations (
    computer text NOT NULL REFERENCES computers.computers(id),
    owner text NOT NULL,
    boot integer NOT NULL CHECK(boot > 0),
    maximum_cpu_ms bigint CHECK(maximum_cpu_ms >= 0),
    state text NOT NULL DEFAULT 'held' CHECK(state IN ('held', 'unknown', 'settled')),
    used_cpu_ms bigint CHECK(used_cpu_ms >= 0 AND used_cpu_ms <= maximum_cpu_ms),
    resolution text,
    created_at timestamptz NOT NULL DEFAULT now(),
    settled_at timestamptz,
    PRIMARY KEY(computer, boot),
    CHECK((state = 'settled') = (used_cpu_ms IS NOT NULL)),
    CHECK((state = 'settled') = (settled_at IS NOT NULL)),
    CHECK(state <> 'settled' OR maximum_cpu_ms IS NOT NULL)
);
CREATE INDEX reservations_by_account ON computers.usage_reservations(owner);

-- Old workers can execute on an already-awake computer without changing its
-- lifecycle record. Reject their queue writes too, before any provider call.
CREATE FUNCTION computers.require_usage_queue_writer() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF current_setting('openagents.usage_writer', true) IS DISTINCT FROM 'v1' THEN
        RAISE EXCEPTION 'The computer service must be updated before accepting commands.';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER turns_require_usage_writer BEFORE INSERT OR UPDATE ON computers.turns
FOR EACH ROW EXECUTE FUNCTION computers.require_usage_queue_writer();
CREATE TRIGGER commands_require_usage_writer BEFORE INSERT OR UPDATE ON computers.commands
FOR EACH ROW EXECUTE FUNCTION computers.require_usage_queue_writer();

-- Existing allocations have no historical meter evidence. Keep them unknown;
-- today's shape or provider defaults cannot fill in their past usage.
INSERT INTO computers.usage_events
    (computer, owner, revision, intent_generation, kind, event_key, deployment,
     driver, size, shape, resource, boot, phase, desired, observed_ms, evidence)
SELECT id, owner, revision, intent_generation, 'observed', 'migration:7',
       binding->>'deployment', driver, size, binding->'shape',
       record->'boots'->-1->>'resource',
       (record->'boots'->-1->>'number')::integer, phase, desired,
       (extract(epoch FROM clock_timestamp()) * 1000)::bigint,
       '{"source":"migration","history":"unknown","provider_seconds":null,"customer_charge":null}'::jsonb
FROM computers.computers;

INSERT INTO computers.usage_reservations(computer, owner, boot, state)
SELECT id, owner, greatest(1, jsonb_array_length(COALESCE(record->'boots', '[]'::jsonb))), 'unknown'
FROM computers.computers WHERE NOT done;
