-- A new attempt needs explicit operator authority after a definitive refusal.
-- The original operation and every accepted/uncertain child remain unchanged.
CREATE TABLE computers.lifecycle_retries (
    operation text PRIMARY KEY REFERENCES computers.lifecycle_operations(operation),
    next_operation text UNIQUE NOT NULL CHECK(next_operation ~ '^lifecycle-retry:[0-9a-f]{64}$'),
    root_operation text NOT NULL REFERENCES computers.lifecycle_operations(operation),
    owner text NOT NULL,
    computer text NOT NULL REFERENCES computers.computers(id),
    binding_fingerprint text NOT NULL,
    resource text NOT NULL,
    action text NOT NULL CHECK(action IN ('suspend','wake','delete')),
    reason text NOT NULL CHECK(length(reason) BETWEEN 1 AND 512),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK(operation<>next_operation)
);
CREATE INDEX lifecycle_retries_by_root ON computers.lifecycle_retries(root_operation);
CREATE TRIGGER immutable_lifecycle_retry BEFORE UPDATE OR DELETE ON computers.lifecycle_retries
FOR EACH ROW EXECUTE FUNCTION computers.keep_usage_event();
