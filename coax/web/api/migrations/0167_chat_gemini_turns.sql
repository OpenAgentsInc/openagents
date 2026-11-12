-- /chat's Gemini models as hosted turns (docs/repos/2026-11-11-indexed-repos.md, "The web chat";
-- #13326): a turn on one of them runs Microcoder with the repo tool host on the runner
-- (coax/web/agent/chat-micro.coax), which reads the repository through oa-repo and asks the model
-- through our inference route for the turn's asker (POST /api/chat/runner/complete).
CREATE OR REPLACE FUNCTION chat.model_ok(m text) RETURNS boolean LANGUAGE sql IMMUTABLE AS
  $$ SELECT m IN ('claude-opus-5-5', 'claude-sonnet-5-5', 'claude-haiku-5-5',
                  'google/gemini-3.8-flash', 'google/gemini-4-argon', 'google/gemini-4-carbon') $$;
