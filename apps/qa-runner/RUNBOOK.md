# Autonomous QA — runbook (how we run it)

Concise internal runbook for `@openagentsinc/qa-runner`. It drives a **real browser**
against a target, verifies a check, and emits a **video + trace + pass/fail verdict** —
then optionally publishes a shareable `/trace/{uuid}`. (Public quickstart for outside
users: `QA-RUNNER.md` + `https://openagents.com/docs/autonomous-qa`.)

## Prereqs (once)

```sh
cd apps/qa-runner
bun install
bunx playwright install chromium   # the real browser the runner drives
```

## 1. Run a verification against prod (the dogfood)

```sh
bun run src/demo-login.ts --url https://openagents.com --out ./runs/login
#   --headed   watch the browser
#   --wrong    point at a deliberately-wrong assertion to prove it FAILS honestly
```

Drives a real Chrome over `openagents.com`: opens `/login`, asserts it stays at `/login`
and shows "Log in to OpenAgents", asserts `/gym/oss` redirects when logged out. Prints a
per-check PASS/FAIL list and writes:

- `runs/login/result.json` — the verdict (`status: pass|fail`)
- `runs/login/session.mp4` — video of the session
- `runs/login/trace.zip` + `*.png` screenshots

```sh
cat runs/login/result.json | python3 -c "import json,sys;print(json.load(sys.stdin)['status'])"
```

## 2. Publish the run as a shareable trace (`/trace/{uuid}`)

Env-armed (honest no-op if unset). The token is the registered Khala/agent bearer; the
endpoint is the live trace ingest. Redaction runs automatically before upload.

```sh
export QA_TRACE_PUBLISH_URL="https://openagents.com/api/traces"
export QA_TRACE_PUBLISH_VISIBILITY="public"        # or unlisted / owner_only
export OPENAGENTS_AGENT_PENDING_TOKEN="…"           # from .secrets/openagents-zeratul-agent.env
```

then call `publishRunDir({ runDir, sessionId, shareBaseUrl: "https://openagents.com" })`
from `src/publish-trace.ts` (Effect) — it converts the run → ATIF → redacts → POSTs →
prints `https://openagents.com/trace/{uuid}`. A finished `control.ts`/`pr-comment` run
publishes automatically when these env vars are set.

**Proof it works (2026-06-24):** a real run of the above against prod returned all PASS and
published <https://openagents.com/trace/db838bdc-3bc6-48a5-8715-a6669f6b10c5> (11 steps,
`openagents/khala`, public, with video).

## 3. Other lanes

- **Khala dogfood / any target:** mint a free key with
  `curl -X POST https://openagents.com/api/keys/free`, export
  `QA_API_KEY` from `.credential.token`, then run
  `bun run src/byo.ts run --url <url> --out ./runs/x`. The default model/base
  are `openagents/khala` and `https://openagents.com/api/v1`; the runner sends
  public-safe `internal` / `qa-runner` attribution headers only to the
  OpenAgents endpoint so served-token analytics can split first-party QA
  dogfood from external demand.
- **BYO override / any target:** `bun run src/byo.ts run --url <url> --model <id> --base-url <url> --api-key <key> --out ./runs/x`
  (bring-your-own model, no OpenAgents login).
- **Compare configs ("chill-eval"):** `bun run src/pr-comment-run.ts --changed "<paths>" --out ./runs/pr-eval`
  → comparison table + a `/trace/compare` link. **Agent-triggered** (no GitHub Actions — per the no-GHA invariant): an agent runs this and posts the PR comment itself, e.g. PR #6224.
- **Import an existing Claude Code / Codex session → trace:**
  `bun run src/trace-import.ts <session.jsonl>` (detect → convert → redact → publish).
- **Distill a session → committed e2e test:** the distiller (`src/distiller.ts`) lowers a
  recorded session into a re-runnable `*.e2e.test.ts` (the review artifact).

## Honest defaults

- No publish, no spend, no network beyond the target unless explicitly armed.
- `--wrong` (or any real deviation) yields an honest FAIL with the failure visible in the
  video — never a fake green.
- Secrets/PII/paths are redacted before any trace upload; the ingest tripwire rejects
  real leaked values.
