-- Email through Resend (email.c, svc-email-resend). One row per message,
-- keyed by its idempotency key: a key already sent is never sent again. The
-- log keeps the key's hash, how many recipients, the attempts, Resend's id and
-- status: never an address, a subject, a body or the API key.
CREATE SCHEMA IF NOT EXISTS email;
CREATE TABLE IF NOT EXISTS email.sends (
  key_sha256 text PRIMARY KEY,
  recipients int NOT NULL,
  attachments int NOT NULL,
  state text NOT NULL CHECK (state IN ('sending', 'sent', 'failed')),
  attempts int NOT NULL DEFAULT 0,
  http_status int,
  resend_id text,
  created_ms bigint NOT NULL,
  updated_ms bigint NOT NULL
);

-- A message to send ({to: text or [text], subject, text, html?, idempotency_key,
-- attachments?: [{filename, content (base64)}], unsubscribe?}), with the staging
-- allowlist (comma separated addresses, '' for none). Returns {state: 'send',
-- key, payload} with Resend's request body, {state: 'sent', id} for a key sent
-- before, or {state: 'refused', error}.
CREATE OR REPLACE FUNCTION email.begin(m jsonb, allow text, now_ms bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  rcpt jsonb := CASE jsonb_typeof(m->'to') WHEN 'string' THEN jsonb_build_array(m->'to') WHEN 'array' THEN m->'to' ELSE NULL END;
  atts jsonb := coalesce(m->'attachments', '[]'::jsonb);
  k text := m->>'idempotency_key';
  h text;
  prior email.sends;
  bad text;
BEGIN
  IF rcpt IS NULL OR jsonb_array_length(rcpt) NOT BETWEEN 1 AND 50 THEN RETURN jsonb_build_object('state', 'refused', 'error', 'to: one to fifty addresses'); END IF;
  SELECT x INTO bad FROM jsonb_array_elements_text(rcpt) x WHERE x !~ '^[^@\s<>,"]+@[^@\s<>,"]+\.[^@\s<>,"]+$' LIMIT 1;
  IF FOUND THEN RETURN jsonb_build_object('state', 'refused', 'error', 'to: not an address'); END IF;
  IF allow <> '' THEN
    SELECT x INTO bad FROM jsonb_array_elements_text(rcpt) x
      WHERE lower(x) NOT IN (SELECT lower(trim(a)) FROM unnest(string_to_array(allow, ',')) a) LIMIT 1;
    IF FOUND THEN RETURN jsonb_build_object('state', 'refused', 'error', 'to: not on this service''s allowlist'); END IF;
  END IF;
  IF coalesce(m->>'subject', '') = '' OR length(m->>'subject') > 200 THEN RETURN jsonb_build_object('state', 'refused', 'error', 'subject: one to 200 characters'); END IF;
  IF coalesce(m->>'text', '') = '' THEN RETURN jsonb_build_object('state', 'refused', 'error', 'text: a plain-text body is required'); END IF;
  IF k IS NULL OR length(k) NOT BETWEEN 8 AND 256 THEN RETURN jsonb_build_object('state', 'refused', 'error', 'idempotency_key: 8 to 256 characters'); END IF;
  IF jsonb_typeof(atts) <> 'array' OR jsonb_array_length(atts) > 10
     OR EXISTS (SELECT 1 FROM jsonb_array_elements(atts) a WHERE coalesce(a->>'filename', '') !~ '^[A-Za-z0-9._ -]{1,100}$'
                                                              OR coalesce(a->>'content', '') !~ '^[A-Za-z0-9+/]+=*$') THEN
    RETURN jsonb_build_object('state', 'refused', 'error', 'attachments: up to ten, each a filename and base64 content');
  END IF;
  h := encode(sha256(convert_to(k, 'UTF8')), 'hex');
  INSERT INTO email.sends (key_sha256, recipients, attachments, state, created_ms, updated_ms)
    VALUES (h, jsonb_array_length(rcpt), jsonb_array_length(atts), 'sending', now_ms, now_ms)
    ON CONFLICT (key_sha256) DO NOTHING;
  IF NOT FOUND THEN
    SELECT * INTO prior FROM email.sends WHERE key_sha256 = h FOR UPDATE;
    IF prior.state = 'sent' THEN RETURN jsonb_build_object('state', 'sent', 'id', prior.resend_id, 'key', h); END IF;
    IF prior.state = 'sending' AND prior.updated_ms > now_ms - 120000 THEN
      RETURN jsonb_build_object('state', 'refused', 'error', 'this message is being sent now');
    END IF;
    UPDATE email.sends SET state = 'sending', updated_ms = now_ms WHERE key_sha256 = h;   -- a failed or stalled send goes again (Resend dedupes the key too)
  END IF;
  RETURN jsonb_build_object('state', 'send', 'key', h, 'payload', jsonb_strip_nulls(jsonb_build_object(
    'from', 'OpenAgents <no-reply@openagents.com>',
    'to', rcpt,
    'subject', m->>'subject',
    'text', m->>'text',
    'html', m->>'html',
    'headers', jsonb_build_object('List-Unsubscribe', coalesce(m->>'unsubscribe', '<mailto:no-reply@openagents.com?subject=unsubscribe>')),
    'attachments', CASE WHEN jsonb_array_length(atts) > 0 THEN
      (SELECT jsonb_agg(jsonb_build_object('filename', a->>'filename', 'content', a->>'content')) FROM jsonb_array_elements(atts) a) END)));
END $$;

CREATE OR REPLACE FUNCTION email.finish(h text, ok boolean, tries int, status int, id text, now_ms bigint) RETURNS void LANGUAGE sql AS $$
  UPDATE email.sends SET state = CASE WHEN ok THEN 'sent' ELSE 'failed' END, attempts = attempts + tries,
                         http_status = status, resend_id = id, updated_ms = now_ms
   WHERE key_sha256 = h
$$;
