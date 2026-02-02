# DMCA response: remove “clawdbot” from user‑facing surfaces (2026‑02‑02)

This document records what happened during the OpenClaw managed MVP work session, where we needed to remove **user-facing** occurrences of the string **"clawdbot"** due to a DMCA takedown request.

**Key constraint (explicit):**
- It is acceptable for **internal** references (env vars, internal paths, logs, upstream package name) to still contain `clawdbot`.
- The goal is to prevent `clawdbot` appearing in **user-facing** UI/admin surfaces and docs that we ship.

---

## Summary

A coding agent attempted to satisfy the rename requirement by making a large invasive change in the upstream `cloudflare/moltworker` repo clone, including runtime API changes and env renames, and deployed those changes. That caused regressions (notably R2 persistence showing “not configured” and repeated gateway starts).

We:
1) **Reset** `~/code/moltworker` back to upstream (`origin/main`).
2) Implemented a **minimal** fix that removes `clawdbot` from user-facing admin surfaces by introducing an `openclaw` wrapper command inside the container.
3) **Deployed** the minimal fix to the `moltbot-sandbox` Worker.
4) Committed the minimal patch locally in `~/code/moltworker`.

---

## What the coding agent did (the problematic moltworker change)

The agent created and deployed a commit in `~/code/moltworker`:
- Commit: `e2a822b` — `Rename runtime envs and add OpenClaw service API`

The agent’s stated intent was:
- Remove all legacy `clawdbot` strings.
- Introduce service-token-gated internal runtime endpoints for OpenAgents managed OpenClaw.

### Why this was a problem

1) **Wrong repo + wrong remote**
- `~/code/moltworker` remote was:
  - `origin https://github.com/cloudflare/moltworker.git`
- Push failed with: `could not read Username for 'https://github.com'`.
- We should not be pushing to upstream Cloudflare’s repo.

2) **Regressions after deploy**
From Wrangler tail logs (example: `/home/christopherdavid/.config/.wrangler/logs/wrangler-2026-02-02_18-06-44_266.log`):
- Cron backup failing:
  - `Backup sync failed: R2 storage is not configured`
- Repeated messages:
  - `R2 storage not configured (missing R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY, or CF_ACCOUNT_ID)`
- Frequent gateway restarts / multiple processes.

Notably, `wrangler secret list` still showed the R2 secrets present, suggesting the code change altered how env was read/passed.

---

## Decision: keep internal references; only remove user-facing `clawdbot`

Confirmed requirement:
- “Anywhere user-facing is fine, internal stuff and logs is fine.”

So we chose a strategy that:
- Minimizes risk (no env renames, no major runtime changes)
- Avoids showing `clawdbot` in user-facing places
- Keeps upstream behavior intact (especially R2 persistence)

---

## Actions taken (the fix)

### 1) Revert moltworker to upstream
We hard-reset local repo to upstream:
```bash
cd ~/code/moltworker
git reset --hard origin/main
```
This removed the invasive runtime changes.

### 2) Minimal patch: introduce `openclaw` wrapper
Instead of renaming internal paths/envs or rewriting the runtime, we created a wrapper command:
- Install upstream CLI as-is: `clawdbot@...`
- Add a symlink:
  - `/usr/local/bin/openclaw -> $(command -v clawdbot)`

Then update user-facing command invocations in admin/debug routes to use `openclaw`.

#### Files changed in `~/code/moltworker`
- `Dockerfile`
  - create `openclaw` symlink
- `src/routes/api.ts`
  - admin device list/approve uses `openclaw devices ...`
- `src/routes/debug.ts`
  - defaults for debug CLI use `openclaw --help` / `openclaw --version`
- `src/gateway/process.ts`
  - recognize both `openclaw ...` and `clawdbot ...` processes (backwards compatible)
- `src/gateway/process.test.ts`
  - updated tests accordingly
- `AGENTS.md`
  - updated docs to instruct using `openclaw` wrapper

#### Local commit in moltworker
- Commit: `cb37619`
  - `chore: use openclaw wrapper in user-facing admin surfaces`

### 3) Deploy
We ran:
```bash
cd ~/code/moltworker
npm run deploy
```
Wrangler deployments now show a new upload:
- `Created: 2026-02-02T20:36:49.886Z`
- `Version(s): 4eb353b6-741a-421b-88b4-42d0317383f1`

Note: Some tail sessions / long-running exec sessions were terminated with `SIGKILL` by the host (OpenClaw process supervision), but the deploy itself completed and shows in `wrangler deployments list`.

---

## Validation checks

### Cloudflare Access behavior
Direct `curl` to endpoints like `/api/status` or `/_admin/` returns a 302 to Cloudflare Access login.
This is expected; it also means user-facing strings primarily show up after successful Access login.

### “clawdbot” in built admin UI assets
We confirmed `dist/client` does not contain `clawdbot` in the static admin UI bundle after the patch.
Internal server bundle still contains some `clawdbot` strings related to internal paths/errors, which is acceptable per requirements.

---

## Follow-ups / open items

1) **Push the moltworker patch to an OpenAgents-owned fork**
- Current repo remote points to Cloudflare upstream.
- We should create/use an OpenAgents fork (or a dedicated runtime template repo) and push commit `cb37619` there.

2) **Avoid repeating the earlier mistake**
Guidance for future agents:
- Do not make large architectural changes to moltworker to satisfy string-removal requirements.
- Keep env/path semantics stable.
- Prefer wrapper indirection (like the `openclaw` wrapper) for user-facing strings.

---

## References

- Wrangler tail log referenced above:
  - `/home/christopherdavid/.config/.wrangler/logs/wrangler-2026-02-02_18-06-44_266.log`
- This repo session log (managed OpenClaw):
  - `apps/website/docs/openclaw-managed-session-log-2026-02-02.md`
