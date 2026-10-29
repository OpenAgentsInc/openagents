-- The Sandbox (rl-sandbox, apps/openagents-web/realm/sandbox/index.md): one
-- shared world, `sandbox`, that everyone builds on. Like the Haunted House it
-- is a built-in world with no owner row, so only the site owns it; here it is
-- opened to direct publishing, so anyone the server knows (a player key or an
-- account) publishes their edits to it, and every publish is in its log. The
-- site can close it again (POST /api/playtest/world/sandbox/open {open: false}).
INSERT INTO verse.world_state (world, open_ms) VALUES ('sandbox', verse.now_ms())
ON CONFLICT (world) DO UPDATE SET open_ms = coalesce(verse.world_state.open_ms, EXCLUDED.open_ms);
