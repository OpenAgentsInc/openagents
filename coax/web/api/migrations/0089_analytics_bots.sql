-- Robots are not visitors (2026-10-31: ShapBot, "Mozilla/5.0 (compatible; ShapBot/0.1.0)", made 33 of
-- 121 "visitor" sessions). analytics.bot_agent() missed it: it knew names and a bare "bot" word, not
-- "<name>bot/<version>" or the "(compatible; <name>bot ...)" and "+http://..." forms every polite
-- crawler uses. Now it does, and a robot's batch is marked auto = 'bot' at the source
-- (analytics.c ingest), so every count that leaves automation out (auto IS NULL, admin.c KEPT,
-- /api/analytics/realms) leaves robots out too. The rows already kept are backfilled here.
CREATE OR REPLACE FUNCTION analytics.bot_agent(ua text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(left(regexp_replace(lower(coalesce(m[1], m[2], m[3])), '[^a-z0-9-]', '', 'g'), 32),
                  CASE WHEN coalesce(ua, '') ~* '\+https?://' THEN 'bot' END)
  FROM (SELECT regexp_match(coalesce(ua, ''),
    '(headlesschrome|headless|phantomjs|puppeteer|playwright|selenium|webdriver|lighthouse|'
    'facebookexternalhit|facebot|meta-externalagent|twitterbot|slackbot|discordbot|linkedinbot|'
    'telegrambot|googlebot|google-inspectiontool|adsbot|bingbot|applebot|yandexbot|'
    'baiduspider|duckduckbot|petalbot|bytespider|gptbot|claudebot|ccbot|crawler|spider|'
    'python-requests|python-urllib|curl/|wget/|go-http-client|okhttp|\ybot\y)'
    '|\y([a-z0-9_-]*(?:bot|crawl|spider|scraper|fetcher))/'
    '|compatible; *([a-z0-9_ .-]*(?:bot|crawl|spider|scan|scraper|fetcher|preview))', 'i') AS m) x
$$;

UPDATE analytics.sessions SET agent = analytics.bot_agent(ua) WHERE agent IS NULL AND analytics.bot_agent(ua) IS NOT NULL;
UPDATE analytics.events e SET auto = 'bot' FROM analytics.sessions x
  WHERE x.sid = e.sid AND e.auto IS NULL AND analytics.bot_agent(x.ua) IS NOT NULL;
UPDATE analytics.playtesters p SET auto = 'bot' FROM analytics.sessions x
  WHERE x.sid = p.sid AND p.auto IS NULL AND analytics.bot_agent(x.ua) IS NOT NULL;
UPDATE analytics.survey_answers a SET auto = 'bot' FROM analytics.sessions x
  WHERE x.sid = a.sid AND a.auto IS NULL AND analytics.bot_agent(x.ua) IS NOT NULL;
