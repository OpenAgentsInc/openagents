-- OpenAgents Cloud retail pricing, Gemini 4 Neon, cached input tokens,
-- 65536 max_tokens, decision-model billing, and pricing/usage read APIs (#14069).
--
-- OpenAgents bills cloud usage at OpenAgents' own retail rates (25% default
-- upcharge over upstream Vertex list prices) while recording upstream cost
-- alongside billed cost. Local model usage (Inkling 2 Mini / Psionic Metal)
-- is always $0 and never billed.

ALTER TABLE inference.models
  ADD COLUMN IF NOT EXISTS upstream_input_micros bigint NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS upstream_cached_input_micros bigint NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS upstream_output_micros bigint NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS cached_input_micros bigint NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS markup_bps int NOT NULL DEFAULT 2500,
  ADD COLUMN IF NOT EXISTS per_call_micros bigint NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS kind text NOT NULL DEFAULT 'chat';

ALTER TABLE inference.calls
  ADD COLUMN IF NOT EXISTS cached_in_tokens int NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS upstream_usd_micros bigint NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS kind text NOT NULL DEFAULT 'chat';

-- Seed Gemini 4 Neon (canonical + short alias) and OpenAgents router decision model.
-- Upstream Gemini 4 Neon: $3.00 in / $0.15 cached / $15.00 out per 1M tokens.
-- OpenAgents retail (+25%): $3.75 in / $0.1875 cached / $18.75 out per 1M tokens.
INSERT INTO inference.models
  (name, upstream, input_micros, output_micros,
   upstream_input_micros, upstream_cached_input_micros, upstream_output_micros,
   cached_input_micros, markup_bps, per_call_micros, kind)
VALUES
  ('google/gemini-4-neon', 'google/gemini-4-neon', 3750000, 18750000,
   3000000, 150000, 15000000, 187500, 2500, 0, 'chat'),
  ('gemini-4-neon', 'google/gemini-4-neon', 3750000, 18750000,
   3000000, 150000, 15000000, 187500, 2500, 0, 'chat'),
  ('openagents/router-decision', 'google/gemini-decisions-v1', 0, 0,
   0, 0, 0, 0, 2500, 250, 'decision')
ON CONFLICT (name) DO UPDATE SET
  upstream = EXCLUDED.upstream,
  input_micros = EXCLUDED.input_micros,
  output_micros = EXCLUDED.output_micros,
  upstream_input_micros = EXCLUDED.upstream_input_micros,
  upstream_cached_input_micros = EXCLUDED.upstream_cached_input_micros,
  upstream_output_micros = EXCLUDED.upstream_output_micros,
  cached_input_micros = EXCLUDED.cached_input_micros,
  markup_bps = EXCLUDED.markup_bps,
  per_call_micros = EXCLUDED.per_call_micros,
  kind = EXCLUDED.kind;

-- Update existing Gemini 4 Carbon & Argon ($4.00/$0.20/$20.00 -> $5.00/$0.25/$25.00).
UPDATE inference.models SET
  upstream_input_micros = 4000000,
  upstream_cached_input_micros = 200000,
  upstream_output_micros = 20000000,
  input_micros = 5000000,
  cached_input_micros = 250000,
  output_micros = 25000000,
  markup_bps = 2500,
  per_call_micros = 0,
  kind = 'chat'
WHERE name IN ('google/gemini-4-carbon', 'google/gemini-4-argon');

-- Update existing Gemini 4 Hydrogen & legacy 3.8 Flash alias ($1.50/$0.075/$7.50 -> $1.875/$0.09375/$9.375).
UPDATE inference.models SET
  upstream_input_micros = 1500000,
  upstream_cached_input_micros = 75000,
  upstream_output_micros = 7500000,
  input_micros = 1875000,
  cached_input_micros = 93750,
  output_micros = 9375000,
  markup_bps = 2500,
  per_call_micros = 0,
  kind = 'chat'
WHERE name IN ('google/gemini-4-hydrogen', 'google/gemini-3.8-flash');

-- Lift max_tokens cap from 8192 to 65536 and restrict chat route to kind = 'chat'.
CREATE OR REPLACE FUNCTION inference.request(who text, body text, cap_n int, cap_usd_micros bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
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
  SELECT * INTO m FROM inference.models WHERE name = b->>'model' AND kind = 'chat';
  IF NOT FOUND THEN RETURN '{"status":400,"error":"this route does not serve that model"}'; END IF;
  INSERT INTO inference.account_day AS a (pubkey, day, n) VALUES (who, current_date, 1)
  ON CONFLICT (pubkey, day) DO UPDATE SET n = a.n + 1 RETURNING * INTO d;
  IF d.n > cap_n THEN
    RETURN jsonb_build_object('status', 429, 'error', format('this account''s %s requests for today are used up', cap_n));
  END IF;
  IF d.usd_micros >= cap_usd_micros THEN
    RETURN jsonb_build_object('status', 429, 'error', format('this account''s $%s spend for today is used up', to_char(cap_usd_micros / 1000000.0, 'FM999999999990.00')));
  END IF;
  stream := coalesce(b->'stream' = 'true'::jsonb, false);
  cap := least(coalesce(CASE WHEN jsonb_typeof(b->'max_tokens') = 'number' THEN (b->>'max_tokens')::numeric END,
                        CASE WHEN jsonb_typeof(b->'max_completion_tokens') = 'number' THEN (b->>'max_completion_tokens')::numeric END, 8192), 65536)::int;
  b := (b - 'store' - 'max_completion_tokens' - 'user') || jsonb_build_object('model', m.upstream, 'max_tokens', greatest(cap, 1));
  IF stream THEN b := b || '{"stream_options":{"include_usage":true}}'; END IF;
  RETURN jsonb_build_object('status', 200, 'request', b, 'model', m.name, 'stream', stream);
END $$;

-- Core call recording with cached vs uncached input tokens and upstream cost tracking.
CREATE OR REPLACE FUNCTION inference.finish_call(
  who text, model text, http int, in_tokens int, cached_in_tokens int, out_tokens int, ms_taken int
) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE
  m inference.models;
  tot_in int := greatest(in_tokens, 0);
  cached int := least(greatest(cached_in_tokens, 0), tot_in);
  plain int := tot_in - cached;
  out_t int := greatest(out_tokens, 0);
  cost bigint := 0;
  up_cost bigint := 0;
BEGIN
  SELECT * INTO m FROM inference.models WHERE name = model;
  IF FOUND THEN
    cost := (plain::bigint * m.input_micros + cached::bigint * m.cached_input_micros + out_t::bigint * m.output_micros + 999999) / 1000000
          + CASE WHEN http = 200 THEN m.per_call_micros ELSE 0 END;
    up_cost := (plain::bigint * m.upstream_input_micros + cached::bigint * m.upstream_cached_input_micros + out_t::bigint * m.upstream_output_micros + 999999) / 1000000;
  END IF;
  INSERT INTO inference.calls
    (ms, pubkey, model, status, in_tokens, cached_in_tokens, out_tokens, usd_micros, upstream_usd_micros, ms_taken, kind)
  VALUES
    ((extract(epoch FROM clock_timestamp()) * 1000)::bigint, who, model, http, tot_in, cached, out_t, cost, up_cost, ms_taken, coalesce(m.kind, 'chat'));
  UPDATE inference.account_day SET usd_micros = usd_micros + cost WHERE pubkey = who AND day = current_date;
  RETURN cost;
END $$;

-- Backwards-compatible 6-arg finish.
CREATE OR REPLACE FUNCTION inference.finish(who text, model text, http int, in_tokens int, out_tokens int, ms_taken int) RETURNS bigint LANGUAGE sql AS $$
  SELECT inference.finish_call(who, model, http, in_tokens, 0, out_tokens, ms_taken)
$$;

-- 8-arg finish with cached_in_tokens and prepaid credit debit in msat.
CREATE OR REPLACE FUNCTION inference.finish(
  who text, model text, http int, in_tokens int, cached_in_tokens int, out_tokens int, ms_taken int, free bool
) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE
  cost bigint; rate bigint; m bigint := 0;
BEGIN
  cost := inference.finish_call(who, model, http, in_tokens, cached_in_tokens, out_tokens, ms_taken);
  IF NOT free AND cost > 0 THEN
    SELECT usd_per_btc INTO rate FROM inference.btc_usd;
    m := CASE WHEN coalesce(rate, 0) > 0 THEN (cost * 100000 + rate - 1) / rate ELSE 0 END;
    UPDATE inference.credit SET msat = msat - m, updated = now() WHERE pubkey = who;
    UPDATE inference.calls SET msat = m WHERE id = (SELECT max(id) FROM inference.calls WHERE pubkey = who);
    IF m > 0 THEN
      INSERT INTO auth.credit_spends AS c (account, hour, msat)
      SELECT a.id, date_trunc('hour', now()), m
        FROM auth.accounts a
        LEFT JOIN auth.nostr_keys k ON k.account = a.id
       WHERE a.kind = 'person' AND (k.pubkey = who OR a.id = who)
       GROUP BY a.id
      ON CONFLICT (account, hour) DO UPDATE SET msat = c.msat + EXCLUDED.msat;
    END IF;
  END IF;
  RETURN m;
END $$;

-- Backwards-compatible 7-arg finish (cached_in_tokens = 0).
CREATE OR REPLACE FUNCTION inference.finish(
  who text, model text, http int, in_tokens int, out_tokens int, ms_taken int, free bool
) RETURNS bigint LANGUAGE sql AS $$
  SELECT inference.finish(who, model, http, in_tokens, 0, out_tokens, ms_taken, free)
$$;

-- Meter and bill a System One / router decision-model call (POST /api/chat/route/v1).
CREATE OR REPLACE FUNCTION inference.decision_record(who text, http int, ms_taken int, free bool) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE
  m inference.models;
  cost bigint := 0;
  up_cost bigint := 0;
  rate bigint;
  msat_due bigint := 0;
BEGIN
  SELECT * INTO m FROM inference.models WHERE name = 'openagents/router-decision';
  IF FOUND AND http = 200 THEN
    cost := m.per_call_micros;
    up_cost := (m.per_call_micros * 10000) / (10000 + greatest(m.markup_bps, 0));
  END IF;
  INSERT INTO inference.account_day AS a (pubkey, day, n, usd_micros)
  VALUES (who, current_date, 1, cost)
  ON CONFLICT (pubkey, day) DO UPDATE SET n = a.n + 1, usd_micros = a.usd_micros + cost;
  INSERT INTO inference.calls
    (ms, pubkey, model, status, in_tokens, cached_in_tokens, out_tokens, usd_micros, upstream_usd_micros, ms_taken, kind)
  VALUES
    ((extract(epoch FROM clock_timestamp()) * 1000)::bigint, who, 'openagents/router-decision', http, 0, 0, 0, cost, up_cost, ms_taken, 'decision');
  IF NOT free AND cost > 0 THEN
    SELECT usd_per_btc INTO rate FROM inference.btc_usd;
    msat_due := CASE WHEN coalesce(rate, 0) > 0 THEN (cost * 100000 + rate - 1) / rate ELSE 0 END;
    UPDATE inference.credit SET msat = msat - msat_due, updated = now() WHERE pubkey = who;
    UPDATE inference.calls SET msat = msat_due WHERE id = (SELECT max(id) FROM inference.calls WHERE pubkey = who);
    IF msat_due > 0 THEN
      INSERT INTO auth.credit_spends AS c (account, hour, msat)
      SELECT a.id, date_trunc('hour', now()), msat_due
        FROM auth.accounts a
        LEFT JOIN auth.nostr_keys k ON k.account = a.id
       WHERE a.kind = 'person' AND (k.pubkey = who OR a.id = who)
       GROUP BY a.id
      ON CONFLICT (account, hour) DO UPDATE SET msat = c.msat + EXCLUDED.msat;
    END IF;
  END IF;
  RETURN msat_due;
END $$;

-- Authoritative OpenAgents Cloud retail pricing schedule (GET /api/v1/inference/pricing).
CREATE OR REPLACE FUNCTION inference.pricing() RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'currency', 'USD',
    'unit', 'micros_per_million_tokens',
    'local_cost_usd_micros', 0,
    'models', coalesce((
      SELECT jsonb_agg(jsonb_build_object(
        'name', m.name,
        'kind', m.kind,
        'input_micros', m.input_micros,
        'cached_input_micros', m.cached_input_micros,
        'output_micros', m.output_micros,
        'per_call_micros', m.per_call_micros,
        'markup_bps', m.markup_bps
      ) ORDER BY m.kind, m.name)
      FROM inference.models m
    ), '[]'::jsonb)
  )
$$;

-- Authenticated user's cloud usage, decision-model usage, and credit balance (GET /api/v1/inference/usage).
CREATE OR REPLACE FUNCTION inference.usage_summary(who text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'who', who,
    'credit_msat', coalesce((SELECT msat FROM inference.credit WHERE pubkey = who), 0),
    'local_usd_micros', 0,
    'today', jsonb_build_object(
      'requests', coalesce((SELECT n FROM inference.account_day WHERE pubkey = who AND day = current_date), 0),
      'usd_micros', coalesce((SELECT usd_micros FROM inference.account_day WHERE pubkey = who AND day = current_date), 0)
    ),
    'total', (
      SELECT jsonb_build_object(
        'calls', count(*)::bigint,
        'in_tokens', coalesce(sum(in_tokens), 0)::bigint,
        'cached_in_tokens', coalesce(sum(cached_in_tokens), 0)::bigint,
        'out_tokens', coalesce(sum(out_tokens), 0)::bigint,
        'usd_micros', coalesce(sum(usd_micros), 0)::bigint,
        'msat', coalesce(sum(msat), 0)::bigint
      ) FROM inference.calls WHERE pubkey = who
    ),
    'decisions', (
      SELECT jsonb_build_object(
        'calls', count(*)::bigint,
        'usd_micros', coalesce(sum(usd_micros), 0)::bigint,
        'msat', coalesce(sum(msat), 0)::bigint
      ) FROM inference.calls WHERE pubkey = who AND kind = 'decision'
    )
  )
$$;
