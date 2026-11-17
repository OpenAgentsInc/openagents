-- A day's limit says its number (GameBuilder issue 15: gamebuilder.org's chat is for every account, and
-- a limit it hits is said in words with its number). inference.request's two 429s now name the cap
-- they were asked with: "this account's 500 requests for today are used up" and "this account's
-- $2.00 spend for today is used up". Nothing else in the function changes: the caps, what is counted
-- and what goes to Vertex are 0059's.
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
  SELECT * INTO m FROM inference.models WHERE name = b->>'model';
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
                        CASE WHEN jsonb_typeof(b->'max_completion_tokens') = 'number' THEN (b->>'max_completion_tokens')::numeric END, 8192), 8192)::int;
  b := (b - 'store' - 'max_completion_tokens' - 'user') || jsonb_build_object('model', m.upstream, 'max_tokens', greatest(cap, 1));
  IF stream THEN b := b || '{"stream_options":{"include_usage":true}}'; END IF;
  RETURN jsonb_build_object('status', 200, 'request', b, 'model', m.name, 'stream', stream);
END $$;
