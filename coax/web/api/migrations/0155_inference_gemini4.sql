-- Gemini 4 Argon and Carbon on our inference route (inference.c, /api/v1/inference and /chat's door
-- /api/chat/complete), beside Gemini 3.8 Flash: the names a request gives, what Vertex is asked for, and
-- the card's rates in millionths of a dollar per million tokens (coax/lib/gateway/card.coax: $4 in, $20 out).
INSERT INTO inference.models VALUES
  ('google/gemini-4-argon', 'google/gemini-4-argon', 4000000, 20000000),
  ('google/gemini-4-carbon', 'google/gemini-4-carbon', 4000000, 20000000)
ON CONFLICT (name) DO NOTHING;
