-- The page is /login: the QR's and LINK A PHONE's codes point there
-- (ORIGIN/login#c=CHALLENGE, ORIGIN/login#link=CHALLENGE). Phone builds from
-- 1058 read both; /signin redirects to /login.
ALTER FUNCTION signin.api(text, jsonb, jsonb) RENAME TO api_0022;
CREATE FUNCTION signin.api(op text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
BEGIN
  IF op = 'challenge' THEN RETURN replace(signin.api_0022(op, body, who)::text, '/signin#c=', '/login#c=')::jsonb; END IF;
  RETURN signin.api_0022(op, body, who);
END $$;
ALTER FUNCTION projects.api(text, text, jsonb, jsonb) RENAME TO api_0023;
CREATE FUNCTION projects.api(op text, want text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
BEGIN
  IF op = 'link' THEN RETURN replace(projects.api_0023(op, want, body, who)::text, '/signin#link=', '/login#link=')::jsonb; END IF;
  RETURN projects.api_0023(op, want, body, who);
END $$;
