# Coding Agent Mission: Implement OpenAgents Web MVP (next phase, fully verified)

## Source of truth (read first; do not skip)
1) `apps/web/docs/MVP_SPEC.md` (what to build + acceptance criteria; complete in order)
2) `docs/local/README.md` (what’s in `docs/local/` and what must never be committed)
3) `docs/local/testing/agent-testing-runbook.md` (the required verification loop + deploy/testing commands)
4) `AGENTS.md` (repo contract: verification-first, no stubs, deploy guidance)

## Non‑negotiable rules
- **Verification-first:** don’t claim “done” until required tests + production checks pass and are logged.
- **No secrets committed:** `docs/local/` is gitignored for a reason. Never paste real secret values into committed docs/logs.
- **Deploy correctly:** for `apps/web`, use `cd apps/web && npm run deploy` (do **not** run raw `npx convex deploy`).
- **Fix-forward:** if you hit an error, document it, fix it, deploy it, and re-test. Don’t stop at diagnosis.

## What to implement (how to choose “next phase”)
- Open `apps/web/docs/MVP_SPEC.md` and work **top-to-bottom**.
- The “next phase” is: **the earliest milestone/section with unmet acceptance criteria**, starting with the **first unchecked requirement**.
- Implement that phase until its acceptance criteria are met (don’t partially implement a phase and move on).

## Required execution loop (you must do this)
Follow `docs/local/testing/agent-testing-runbook.md` §0 “MUST-DO LOOP”:
- Interact meaningfully with **openagents.com** (UI and API), including OpenClaw and social surfaces.
- On any error: append a detailed entry to `docs/local/testing/agent-testing-errors.md` (timestamp, URL/step, response, what you tried), then fix/deploy/retest.
- Repeat until the full loop completes with no errors.

## Local verification (minimum)
Run the relevant checks for what you changed, starting narrow then broad:
- **Web:** `cd apps/web && npm run test` then `npm run lint` then `npm run test:e2e` (auth tests require storage state; see runbook)
- **Rust (if touched):** from repo root `cargo test`
- Add/adjust tests when you change behavior (especially auth/provisioning/proxying).

## Deploys (as needed for testing)
Deploy whichever components your changes touched (see runbook “Deploys” table). Typical:
- Web: `cd apps/web && npm run deploy`
- API: `cd apps/api && npm run deploy`
- Runtime: `cd apps/openclaw-runtime && npm run deploy`
- Agent worker: `cd apps/agent-worker && npm run deploy`

## Production verification (required)
After typechecks/tests pass and deploys complete:
- Verify the affected flows on **production** (UI + API). At minimum:
  - `/hatchery` (access gating + provision + controls)
  - `/openclaw/chat` (streaming)
  - `/assistant` / `POST /chat` (tool calling + approvals)
  - Agent flows if touched (API key auth; agent UI parity if in scope)

## Documentation + handoff (must update before you finish)
- Update `apps/web/docs/MVP_SPEC.md`:
  - Check off what you completed.
  - Append a **Work Log** entry with:
    - date/time, branch, what you implemented, key files touched
    - tests run + results
    - deploys performed (which apps) + production checks performed
    - known issues / next “first unchecked item” to do next
- Update any other docs that the change impacts (auth, ops/env, troubleshooting). Don’t put secrets in committed docs.

## Git hygiene (must do)
- Ensure `git status` is clean except the intended changes.
- Commit with a message that names the phase (e.g. “MVP: agent login session + Convex custom auth”).
- Push the branch.
- If you had to change configs/env in production, include a redacted note in the Work Log describing *what* changed (never the secret values).

## “Done” means
- The chosen next phase’s acceptance criteria are satisfied.
- Tests + typechecks pass.
- Production verification is performed and logged.
- MVP_SPEC is updated with checkmarks + a Work Log entry.
- Changes are committed and pushed.
