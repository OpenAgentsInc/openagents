-- Exa central metered search & fetch billing (#14114, #14115, docs/exa/SPEC.md).
--
-- OpenAgents holds a single central EXA_API_KEY on oa-api and meters every
-- /api/v1/search, /api/v1/fetch, and /api/v1/mcp/exa call against the caller's
-- prepaid Lightning/x402 credit (inference.credit) at Exa's exact reported
-- costDollars.total + OpenAgents' retail markup (default +25%, markup_bps=2500).

INSERT INTO inference.models
  (name, upstream, input_micros, output_micros,
   upstream_input_micros, upstream_cached_input_micros, upstream_output_micros,
   cached_input_micros, markup_bps, per_call_micros, kind)
VALUES
  -- Exa /search fallback: $0.005 upstream -> $0.00625 retail (+25%) when costDollars omitted.
  ('exa/search', 'https://api.exa.ai/search', 0, 0,
   0, 0, 0, 0, 2500, 6250, 'search'),
  -- Exa /contents fallback: $0.001 upstream -> $0.00125 retail (+25%) when costDollars omitted.
  ('exa/contents', 'https://api.exa.ai/contents', 0, 0,
   0, 0, 0, 0, 2500, 1250, 'search')
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

-- Admission check for Exa search/fetch calls: enforces prepaid credit > 0 (402),
-- fresh BTC/USD exchange rate (503), and per-account daily request/spend caps (429).
CREATE OR REPLACE FUNCTION inference.search_admit(
  who text, model_name text, cap_n int, cap_usd_micros bigint, free bool
) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m inference.models;
  d inference.account_day;
  c bigint;
  age double precision;
BEGIN
  SELECT * INTO m FROM inference.models WHERE name = model_name AND kind = 'search';
  IF NOT FOUND THEN
    RETURN '{"status":400,"error":"unknown search service"}';
  END IF;
  IF NOT free THEN
    SELECT msat INTO c FROM inference.credit WHERE pubkey = who;
    IF coalesce(c, 0) <= 0 THEN
      RETURN jsonb_build_object(
        'status', 402,
        'error', 'add credit: POST /api/v1/inference/credit',
        'credit', '/api/v1/inference/credit',
        'credit_msat', coalesce(c, 0)
      );
    END IF;
    age := inference.rate_age();
    IF age IS NULL OR age > 3600 THEN
      RETURN '{"status":503,"error":"no bitcoin price to charge at; try again soon"}';
    END IF;
  END IF;
  INSERT INTO inference.account_day AS a (pubkey, day, n)
  VALUES (who, current_date, 1)
  ON CONFLICT (pubkey, day) DO UPDATE SET n = a.n + 1
  RETURNING * INTO d;
  IF d.n > cap_n THEN
    RETURN jsonb_build_object(
      'status', 429,
      'error', format('this account''s %s requests for today are used up', cap_n)
    );
  END IF;
  IF d.usd_micros >= cap_usd_micros THEN
    RETURN jsonb_build_object(
      'status', 429,
      'error', format('this account''s $%s spend for today is used up', to_char(cap_usd_micros / 1000000.0, 'FM999999999990.00'))
    );
  END IF;
  RETURN jsonb_build_object(
    'status', 200,
    'model', m.name,
    'markup_bps', m.markup_bps,
    'per_call_micros', m.per_call_micros,
    'free', free
  );
END $$;

-- Record and bill a completed Exa search/fetch call from exact upstream_usd_micros
-- (parsed from Exa's costDollars.total) plus OpenAgents retail markup_bps.
CREATE OR REPLACE FUNCTION inference.search_finish(
  who text, model_name text, http int, upstream_usd_micros bigint, ms_taken int, free bool
) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m inference.models;
  up_cost bigint := 0;
  cost bigint := 0;
  rate bigint;
  msat_due bigint := 0;
  call_id bigint;
BEGIN
  SELECT * INTO m FROM inference.models WHERE name = model_name AND kind = 'search';
  IF FOUND AND http = 200 THEN
    IF coalesce(upstream_usd_micros, 0) > 0 THEN
      up_cost := upstream_usd_micros;
      cost := (up_cost * (10000 + greatest(m.markup_bps, 0)) + 9999) / 10000;
    ELSE
      cost := m.per_call_micros;
      up_cost := (m.per_call_micros * 10000) / (10000 + greatest(m.markup_bps, 0));
    END IF;
  END IF;
  INSERT INTO inference.calls
    (ms, pubkey, model, status, in_tokens, cached_in_tokens, out_tokens, usd_micros, upstream_usd_micros, ms_taken, kind)
  VALUES
    ((extract(epoch FROM clock_timestamp()) * 1000)::bigint, who, model_name, http, 0, 0, 0, cost, up_cost, greatest(coalesce(ms_taken, 0), 0), 'search')
  RETURNING id INTO call_id;
  IF cost > 0 THEN
    UPDATE inference.account_day SET usd_micros = usd_micros + cost WHERE pubkey = who AND day = current_date;
  END IF;
  IF NOT free AND cost > 0 THEN
    SELECT usd_per_btc INTO rate FROM inference.btc_usd;
    msat_due := CASE WHEN coalesce(rate, 0) > 0 THEN (cost * 100000 + rate - 1) / rate ELSE 0 END;
    UPDATE inference.credit SET msat = msat - msat_due, updated = now() WHERE pubkey = who;
    UPDATE inference.calls SET msat = msat_due WHERE id = call_id;
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
  RETURN jsonb_build_object(
    'model', model_name,
    'upstream_usd_micros', up_cost,
    'usd_micros', cost,
    'msat', msat_due,
    'free', free
  );
END $$;

-- Extend usage_summary with search breakdown alongside total and decisions.
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
    ),
    'search', (
      SELECT jsonb_build_object(
        'calls', count(*)::bigint,
        'usd_micros', coalesce(sum(usd_micros), 0)::bigint,
        'msat', coalesce(sum(msat), 0)::bigint
      ) FROM inference.calls WHERE pubkey = who AND kind = 'search'
    )
  )
$$;
