-- Only workers that recover a terminal command's generation may take jobs.
CREATE OR REPLACE FUNCTION computers.require_usage_writer() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF current_setting('openagents.usage_writer', true) IS DISTINCT FROM 'v2' THEN
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

CREATE OR REPLACE FUNCTION computers.require_usage_queue_writer() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF current_setting('openagents.usage_writer', true) IS DISTINCT FROM 'v2' THEN
        RAISE EXCEPTION 'The computer service must be updated before accepting commands.';
    END IF;
    RETURN NEW;
END;
$$;
