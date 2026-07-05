# codex-fleet — "Codex-subscription, PR-per-agent" fleet runner

A small, dependency-light runner that turns **one non-green product promise** into
**one `codex exec` coding agent** running on the **OpenAgents ChatGPT/Codex
subscription**, working in **one isolated git worktree**, on **one branch**, that
opens **one PR** for human review.

It is the active subscription-backed fleet lane. Workers produce
`codex-fleet/<promise>` PR branches, so the existing merge gate
`/tmp/fleet-merge.sh` still gates it unchanged.

The reason for the swap: Anthropic-Claude-on-Vertex is a third-party SKU **not
covered by the GFS credit**, so every Vertex batch was direct card spend. Codex
runs on our **existing ChatGPT/Codex subscription** — no per-token card spend,
just subscription quota.

## Guardrails (enforced by the scripts)

- **PR-per-agent only.** Workers push `codex-fleet/<promise>` branches and open
  PRs via `gh`. They **never** push to `main`.
- **No green flips.** The task brief forbids editing the product-promise registry
  or changing any promise state. Agents build the missing *piece*.
- **check:deploy is the merge gate.** Each worker runs `bun run check:deploy` and
  records pass/fail on the PR. A failing check is reported, not hidden.
- **No secrets printed or committed.** The auth blob, OAuth tokens, account ids,
  the admin token, and the agent token are never printed. Only public refs,
  lengths, booleans, statuses, and env-var *names* appear.
- **Per-promise isolation.** Each worker gets its own `CODEX_HOME` and leases one
  subscription account, released after the run, so concurrent workers never
  share or overwrite auth material.
- **Bounded ripgrep searches.** Workers install an `rg` wrapper on the Codex
  process `PATH` that strips unrestricted traversal flags (`-u`, `--no-ignore`,
  `--hidden`, `--follow`) and always excludes `node_modules`, `.git`, `dist`,
  and `build`.

## Components

| File | What it does |
|------|--------------|
| `assign.mjs` | Fetches the public product-promise registry (`https://openagents.com/api/public/product-promises`, browser UA), selects N non-green promises with **buildable, non-owner-gated** blockers, and emits one task brief each. `--priority business` front-loads business-fulfillment promises. Open-PR dedup matches the `codex-fleet/<promise>` branch prefix. |
| `fetch-codex-auth.mjs` | **The auth crux.** Pulls a Codex OAuth blob from the central device-flow provider-account store and materializes a codex-native `auth.json` under an isolated `CODEX_HOME`. Subcommands: `lease`, `release`, `sanity-all`. |
| `install-rg-guard.mjs` | Installs the per-run `rg` wrapper used by `worker.sh` so agent searches respect ignore files and cannot traverse heavy generated directories. |
| `worker.sh` | Given one assignment: `git worktree add` from `origin/main`, `bun install`, fetch central Codex auth into a per-promise `CODEX_HOME`, run `codex exec "<brief>" -m gpt-5.5 -c model_reasoning_effort=xhigh --dangerously-bypass-approvals-and-sandbox --json`, release the lease, run `check:deploy`, commit to branch `codex-fleet/<promise>`, push, open a PR. Emits a one-line JSON result (incl. token usage). |
| `run.sh` | Orchestrator. Runs a few workers (sequential by default; `--parallel` opt-in), then prints PR URLs + per-worker `check:deploy` status + total tokens. |

## Usage

```bash
# from the repo root of an openagents checkout, with the two tokens in env:
set -a
. /Users/christopherdavid/work/.secrets/vortex-admin.env              # OPENAGENTS_ADMIN_API_TOKEN
. /Users/christopherdavid/work/.secrets/openagents-codex-loopwright-agent.env  # OPENAGENTS_AGENT_TOKEN
set +a

bash scripts/codex-fleet/run.sh --count 3

# pick specific promises
bash scripts/codex-fleet/run.sh --ids energy.flexible_load_proof.v1,autopilot.decision_queue.v1

# preview selection + briefs without spending quota (no auth fetch, no codex exec)
bash scripts/codex-fleet/run.sh --count 3 --dry-run

# build + check:deploy but don't open PRs
bash scripts/codex-fleet/run.sh --count 2 --no-pr
```

Flags: `--count N`, `--state red|yellow|planned|any`, `--model <codex model>`,
`--ids a,b,c`, `--priority business|any`, `--parallel`, `--dry-run`, `--no-pr`.

Results are written to `/tmp/cf-results.jsonl`; per-worker logs to
`/tmp/cf-logs/<promise>.agent.log`; assignments + briefs to
`/tmp/cf-assignments/`; full `codex exec --json` traces to
`~/work/codex-fleet-traces/<date>/` (indexed in `index.jsonl`).

## Auth: the central device-flow provider-account store (the crux)

Codex normally authenticates one of two ways: an `OPENAI_API_KEY`, or a ChatGPT
login that writes `~/.codex/auth.json` (`auth_mode: "chatgpt"`,
`tokens.{id_token, access_token, refresh_token, account_id}`). The ChatGPT path
is normally **interactive** (`codex login`, browser device-code page).

We do **not** want a per-machine interactive login on every cloud worker. Instead
we reuse the **central device-flow provider-account system already built into
openagents.com** (operator runbook:
`apps/openagents.com/docs/2026-06-05-chatgpt-device-login-operator-runbook.md`).
The owner connects ChatGPT/Codex accounts **once**, via the device-code ceremony
(`scripts/provider-chatgpt-device-login.mjs start/poll`); the worker then pulls a
short-lived auth blob over HTTP. No browser, no `codex login`, on the worker.

### How `fetch-codex-auth.mjs lease` works (no secrets)

Two server tokens, two stages, then a local translation:

1. **ADMIN token** → `POST /api/operator/provider-accounts/chatgpt-codex/leases`
   `{ requestedAction, email, assignmentId, runId }`. The central lease selector
   picks the connected + healthy account with the fewest active leases and returns
   `{ leaseRef, providerAccountRef }` (public refs only).
2. **ADMIN token** → `POST /api/operator/provider-accounts/chatgpt-codex/leases/grant`
   `{ leaseRef, email, runId }`. Issues a short-lived, runner-scoped grant for the
   leased account and returns `{ grant: { grantRef, expiresAt, ... } }`.
3. **AGENT token** (programmatic-agent bearer) →
   `POST /api/provider-accounts/chatgpt-codex/grants/resolve`
   `{ grantRef, providerAccountRef, includeAuthMaterial: true, runId }`. Resolves
   the grant **with** auth material behind the server-side secret boundary and
   returns
   `{ authMaterial: { authContentEnv: "OPENCODE_AUTH_CONTENT", authContentJson } }`.
   `authContentJson` is the OpenCode/openauth `auth.json`:
   `{ openai: { type: "oauth", access, refresh, expires, accountId?, idToken? } }`.
4. **Local translation** into the codex-CLI-native shape and write to
   `CODEX_HOME/auth.json` (0600):
   `{ auth_mode: "chatgpt", OPENAI_API_KEY: null,
      tokens: { id_token, access_token, refresh_token, account_id },
      last_refresh }`.
   (This mirrors the server's own `codexOAuthAuthFromAuthMaterial` extraction in
   `operator-provider-account-routes.ts`.)

`codex exec` then reads `CODEX_HOME/auth.json` and runs on the subscription. The
worker **releases the lease** as soon as the agent finishes.

### Required env (names only — no values)

```
OPENAGENTS_ADMIN_API_TOKEN   # admin/operator bearer (lease + grant issue)
OPENAGENTS_AGENT_TOKEN       # programmatic-agent bearer (grant resolve w/ material)
OPENAGENTS_BASE_URL          # default https://openagents.com
OPENAGENTS_FLEET_EMAIL       # default chris@openagents.com (target user for the account fleet)
CODEX_HOME                   # per-promise auth.json home (worker.sh sets this)
```

On this Mac the two tokens live in ignored workspace secret files:

```
/Users/christopherdavid/work/.secrets/vortex-admin.env                       # OPENAGENTS_ADMIN_API_TOKEN
/Users/christopherdavid/work/.secrets/openagents-codex-loopwright-agent.env  # OPENAGENTS_AGENT_TOKEN
```

### Account readiness (owner-gated)

A lease only resolves usable material for a **connected + healthy** Codex account.
If `grants/resolve` returns `provider_account_auth_material_unavailable`, the
selected account's stored refresh token was invalidated (`requires_reauth`) and
the owner must reconnect it:

```bash
# inspect fleet health (counts + classes only)
node scripts/codex-fleet/fetch-codex-auth.mjs sanity-all --email chris@openagents.com

# reconnect an account (owner completes the browser device-code page)
node scripts/provider-chatgpt-device-login.mjs start --email chris@openagents.com --label "codex 1" --create-new
node scripts/provider-chatgpt-device-login.mjs poll <attempt-id>
node scripts/provider-chatgpt-device-login.mjs sanity <provider-account-ref>
```

## Cost note (subscription, not pay-per-token)

Unlike the retired Vertex fleet (Google Cloud per-token billing, direct card
spend), this fleet runs on the **ChatGPT/Codex subscription**. There is no
per-token card charge, but there **is** shared subscription quota and rate
limiting, and each worker pins one account via a lease for the duration of the
run. Keep waves modest and prefer `--parallel` only up to the number of healthy
connected accounts.

## Concurrency reality

Each worker leases **one** account for its run and gets its **own** `CODEX_HOME`.
With `--parallel`, keep `--count` at or below the number of healthy connected
Codex accounts (check `fetch-codex-auth.mjs sanity-all`) so workers don't contend
for the same seat. The lease selector spreads load by fewest-active-leases.
