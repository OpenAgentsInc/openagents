Fix: Clean Chat mode, remove startup summarizer, add server logs; default to main app

Summary
- Default desktop binary: ensure `cargo tauri dev` launches the main app (`default-run = "openagents"`).
- Robust dev startup: add root `dev.sh` + `src-tauri/dev.sh` to preflight Trunk, reuse/guard port 1420, and fail with clear errors.
- Window + UX: increase initial size to 1280×900 (min 1024×720); event log shown by default.
- Chat vs Task modes: add a Mode toggle. Chat creates clean sessions; Task routes Send into Master Task (create → plan → run).
- Clean Chat sessions: launch codex with `history.persistence=none` and inject per‑turn `<user_instructions>` so we get a plain answer (no project/task bleed).
- Remove confusing fallback: do not render `task_complete.last_agent_message` (prevents stale lists showing as replies).
- Startup correctness: stop auto‑summarizing chat titles on load (no off‑record prompts at startup).
- Server logging: print proto spawn args (cwd, model, effort, history), stdin writes (`proto >> …`), stdout lines (`proto << …`), stderr, and an 8s watchdog on silence.
- Anchor session cwd: pass `-c cwd=<repo root>` to codex proto to avoid `cwd not set` and keep context stable.
- Event mapping: support `token_count` top‑level fields; map `stream_error`/`error` to System notes.
- Minor: New Task panel reactive; various UI robustness and error surfacing.

Testing
- `cargo check --target wasm32-unknown-unknown` (UI): OK
- `cd src-tauri && cargo check` (Tauri): OK
- `trunk build` preflight runs via dev.sh; successful.
- Manual run: `cargo tauri dev` prints server logs and Chat mode returns a plain assistant reply.

Notes
- This PR intentionally avoids model traffic on startup.
- Chat mode no longer renders `task_complete.last_agent_message` to prevent stale content.
- All changes are scoped to dev/runtime; production packaging unaffected.
