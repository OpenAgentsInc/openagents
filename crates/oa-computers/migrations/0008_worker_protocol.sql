-- Old workers must not claim, renew, or reschedule jobs after the upgrade.
-- The row revision checks also catch stale writes from marked connections.
CREATE OR REPLACE FUNCTION computers.require_usage_writer() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF current_setting('openagents.usage_writer', true) IS DISTINCT FROM 'v1' THEN
        RETURN NULL;
    END IF;
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
