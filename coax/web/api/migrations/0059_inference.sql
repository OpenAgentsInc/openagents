-- Our inference route (gw-ours; inference.c answers POST /v1/inference):
-- the Mac app's Lab sends Chat Completions requests here, signed NIP-98 by
-- the user's Nostr key, and the service calls Vertex AI with its own
-- service account, so the app carries no provider key. Every request is
-- metered by account (the signing pubkey): a day's requests and spend are
-- capped per account, and every call is kept with its tokens and its cost
-- at the card's rates (coax/lib/gateway/rates.coax, Vertex's row).
CREATE SCHEMA IF NOT EXISTS inference;

-- The models this route serves: what the request names, what Vertex is
-- asked for, and the card's rates in millionths of a dollar per million tokens.
CREATE TABLE inference.models (
  name text PRIMARY KEY,
  upstream text NOT NULL,
  input_micros bigint NOT NULL,
  output_micros bigint NOT NULL
);
INSERT INTO inference.models VALUES ('google/gemini-3.8-flash', 'google/gemini-3.8-flash', 750000, 3750000);

CREATE TABLE inference.account_day (
  pubkey text NOT NULL,
  day date NOT NULL,
  n int NOT NULL DEFAULT 0,
  usd_micros bigint NOT NULL DEFAULT 0,
  PRIMARY KEY (pubkey, day)
);

CREATE TABLE inference.calls (
  id bigserial PRIMARY KEY,
  ms bigint NOT NULL,
  pubkey text NOT NULL,
  model text NOT NULL,
  status int NOT NULL,           -- Vertex's HTTP status; 0 when it was not reached
  in_tokens int NOT NULL DEFAULT 0,
  out_tokens int NOT NULL DEFAULT 0,
  usd_micros bigint NOT NULL DEFAULT 0,
  ms_taken int
);
CREATE INDEX inference_calls_pubkey ON inference.calls (pubkey, ms);

-- May this account ask now, and what goes to Vertex: {"status": 200,
-- "request": the body with Vertex's model name, no store, the output capped
-- at 8192 tokens and usage on a stream, "model", "stream"}, or {"status",
-- "error"}: not a JSON object or no messages (400), a model this route does
-- not serve (400), or the account's day used up (429): its requests (cap_n)
-- or its spend (cap_usd_micros).
CREATE FUNCTION inference.request(who text, body text, cap_n int, cap_usd_micros bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  b jsonb;
  m inference.models;
  d inference.account_day;
  stream bool;
  cap int;
BEGIN
  BEGIN b := body::jsonb; EXCEPTION WHEN others THEN RETURN '{"status":400,"error":"not JSON"}'; END;
  IF jsonb_typeof(b) IS DISTINCT FROM 'object' OR jsonb_typeof(b->'messages') IS DISTINCT FROM 'array' OR jsonb_array_length(b->'messages') = 0 THEN
    RETURN '{"status":400,"error":"a Chat Completions request needs messages"}';
  END IF;
  SELECT * INTO m FROM inference.models WHERE name = b->>'model';
  IF NOT FOUND THEN RETURN '{"status":400,"error":"this route does not serve that model"}'; END IF;
  INSERT INTO inference.account_day AS a (pubkey, day, n) VALUES (who, current_date, 1)
  ON CONFLICT (pubkey, day) DO UPDATE SET n = a.n + 1 RETURNING * INTO d;
  IF d.n > cap_n THEN RETURN '{"status":429,"error":"this account''s requests for today are used up"}'; END IF;
  IF d.usd_micros >= cap_usd_micros THEN RETURN '{"status":429,"error":"this account''s spend for today is used up"}'; END IF;
  stream := coalesce(b->'stream' = 'true'::jsonb, false);
  cap := least(coalesce(CASE WHEN jsonb_typeof(b->'max_tokens') = 'number' THEN (b->>'max_tokens')::numeric END,
                        CASE WHEN jsonb_typeof(b->'max_completion_tokens') = 'number' THEN (b->>'max_completion_tokens')::numeric END, 8192), 8192)::int;
  b := (b - 'store' - 'max_completion_tokens' - 'user') || jsonb_build_object('model', m.upstream, 'max_tokens', greatest(cap, 1));
  IF stream THEN b := b || '{"stream_options":{"include_usage":true}}'; END IF;
  RETURN jsonb_build_object('status', 200, 'request', b, 'model', m.name, 'stream', stream);
END $$;

-- Keeps one call and charges its cost to the account's day: the cost in millionths of a dollar.
CREATE FUNCTION inference.finish(who text, model text, http int, in_tokens int, out_tokens int, ms_taken int) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE
  m inference.models;
  cost bigint := 0;
BEGIN
  SELECT * INTO m FROM inference.models WHERE name = model;
  IF FOUND THEN
    cost := (greatest(in_tokens, 0)::bigint * m.input_micros + greatest(out_tokens, 0)::bigint * m.output_micros + 999999) / 1000000;
  END IF;
  INSERT INTO inference.calls (ms, pubkey, model, status, in_tokens, out_tokens, usd_micros, ms_taken)
  VALUES ((extract(epoch FROM clock_timestamp()) * 1000)::bigint, who, model, http, greatest(in_tokens, 0), greatest(out_tokens, 0), cost, ms_taken);
  UPDATE inference.account_day SET usd_micros = usd_micros + cost WHERE pubkey = who AND day = current_date;
  RETURN cost;
END $$;
