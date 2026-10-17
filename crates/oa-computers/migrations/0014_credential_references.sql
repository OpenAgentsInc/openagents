-- Credential values and caller keys never enter this table.
CREATE SCHEMA IF NOT EXISTS computer_credentials;
CREATE TABLE IF NOT EXISTS computer_credentials.bindings (
    id uuid PRIMARY KEY,
    owner text NOT NULL,
    computer text NOT NULL,
    request_key text NOT NULL,
    fingerprint text NOT NULL,
    binding jsonb NOT NULL,
    UNIQUE(owner, computer, request_key)
);
CREATE INDEX IF NOT EXISTS credential_native_scope
    ON computer_credentials.bindings ((binding->>'native_resource'),(binding->>'node'));

-- Revocation commits in the same transaction as the lifecycle change, including
-- an automatic checkpoint, uncertain state, wake, or deletion.
CREATE OR REPLACE FUNCTION computer_credentials.revoke_computer() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    UPDATE computer_credentials.bindings
    SET binding=jsonb_set(binding,'{state}',to_jsonb('revoked'::text))
    WHERE computer=OLD.id AND binding->>'state'<>'revoked';
    RETURN NULL;
END $$;
DROP TRIGGER IF EXISTS revoke_computer_credentials ON computers.computers;
CREATE CONSTRAINT TRIGGER revoke_computer_credentials
AFTER UPDATE ON computers.computers
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW WHEN (OLD.intent_generation IS DISTINCT FROM NEW.intent_generation
    OR NEW.desired<>'running' OR NEW.phase NOT IN ('awake','turn'))
EXECUTE FUNCTION computer_credentials.revoke_computer();
DROP TRIGGER IF EXISTS delete_computer_credentials ON computers.computers;
CREATE CONSTRAINT TRIGGER delete_computer_credentials AFTER DELETE ON computers.computers
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION computer_credentials.revoke_computer();

CREATE OR REPLACE FUNCTION computer_credentials.revoke_command() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    UPDATE computer_credentials.bindings
    SET binding=jsonb_set(binding,'{state}',to_jsonb('revoked'::text))
    WHERE computer=NEW.computer
        AND binding->>'command_key'=NEW.request_key AND binding->>'state'<>'revoked';
    RETURN NULL;
END $$;
DROP TRIGGER IF EXISTS revoke_command_credentials ON computers.commands;
CREATE TRIGGER revoke_command_credentials
AFTER UPDATE OF status,cancel_requested ON computers.commands
FOR EACH ROW WHEN (NEW.cancel_requested OR NEW.status NOT IN ('queued','dispatching','running'))
EXECUTE FUNCTION computer_credentials.revoke_command();
