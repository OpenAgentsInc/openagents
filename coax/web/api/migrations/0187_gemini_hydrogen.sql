-- Gemini 4 Hydrogen replaces Gemini 3.8 Flash as the lightweight Gemini (#13479). Our inference route
-- (inference.c, /api/v1/inference and /chat's door /api/chat/complete) serves it at its published
-- price: $1.50 in and $7.50 out a million tokens, thinking counted as output (Vertex's on-demand global
-- SKUs F4D7-3BA1-107D and E576-C50D-41EF and aistudio.google.com/docs/pricing, read 2026-11-13).
INSERT INTO inference.models VALUES ('google/gemini-4-hydrogen', 'google/gemini-4-hydrogen', 1500000, 7500000)
ON CONFLICT (name) DO UPDATE SET upstream = EXCLUDED.upstream, input_micros = EXCLUDED.input_micros, output_micros = EXCLUDED.output_micros;

-- The old name stays a name the route answers, so a client or a saved choice that still sends it is
-- not refused: it is asked of Hydrogen and metered at Hydrogen's price.
UPDATE inference.models SET upstream = 'google/gemini-4-hydrogen', input_micros = 1500000, output_micros = 7500000
WHERE name = 'google/gemini-3.8-flash';

-- /chat's hosted turns: Hydrogen is offered; the old name is still accepted (turns already kept name it,
-- and the runner runs it on Hydrogen, chat-runner.c chat_model). Conversations that chose it now name Hydrogen.
CREATE OR REPLACE FUNCTION chat.model_ok(m text) RETURNS boolean LANGUAGE sql IMMUTABLE AS
  $$ SELECT m IN ('claude-opus-5-5', 'claude-sonnet-5-5', 'claude-haiku-5-5',
                  'google/gemini-4-hydrogen', 'google/gemini-4-argon', 'google/gemini-4-carbon',
                  'google/gemini-3.8-flash') $$;
UPDATE chat.conversations SET model = 'google/gemini-4-hydrogen' WHERE model = 'google/gemini-3.8-flash';
