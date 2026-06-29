# Pylon Multi-Session Agent Runbook

Status: operational handoff, 2026-06-13.

Use this when a fresh coding agent needs to drive Pylon as a local coding
orchestrator: inspect connected Codex/ChatGPT and Claude accounts, run one or
many bounded subagent sessions across separate workspaces, retain proof
artifacts, and optionally control a running Pylon node over the loopback
control API.

This runbook describes the source checkout on `main`, not the older published
`@openagentsinc/pylon` `latest` package. Run commands from the repository root
unless a command explicitly says `cd apps/pylon`.

## Non-Negotiables

- Read the repository `AGENTS.md` and `INVARIANTS.md` before editing code.
- Do not print raw auth files, bearer tokens, refresh tokens, local credential
  paths, prompts from private work, or provider payloads into commits, issues,
  docs, logs, or public artifacts.
- Keep Git operations scoped to the `openagents` repo. Use `main` when clean.
  If other work is present on `main`, create an isolated worktree only when
  the owner explicitly allows branch/worktree work for the task.
- Pylon proof and control-session artifacts are designed to be public-safe:
  account identity appears as hashed refs, workspace identity appears as refs,
  and redaction scans block retained artifacts that contain private material.
- Refreshing usage with `--refresh` performs one minimal provider inference per
  selected account and may consume paid provider tokens.

## 1. Confirm The Local Pylon Surface

```sh
git status --short --branch
bun --cwd apps/pylon test tests/account-usage.test.ts
```

If dependencies are missing, run:

```sh
bun install --cwd apps/pylon
```

Prefer source commands while dogfooding:

```sh
cd apps/pylon
bun src/index.ts context --json
bun src/index.ts accounts list --json
```

`accounts list` should show each provider, readiness state, hashed
`accountRefHash`, hashed `homeRef`, and blocker refs. It must not show raw
credential paths.

## 2. Inspect Connected Accounts And Usage

From `apps/pylon`:

```sh
bun src/index.ts accounts list --json
bun src/index.ts accounts usage --json
bun src/index.ts accounts usage --account codex --json
bun src/index.ts accounts usage --provider claude --json
```

Supported default selectors:

- `--account codex`, `--account chatgpt`, `--provider codex`: unnamed default
  Codex/ChatGPT home.
- `--account claude`, `--account claude_agent`, `--provider claude`: unnamed
  default Claude home.
- `--account <registered-ref>`: a named account from `dev.accounts`.
- `--all`: every discovered registered and default account.

To capture fresh local-session usage for only the default Codex/ChatGPT home:

```sh
bun src/index.ts accounts usage --account codex --refresh --json
```

Expected result:

- `refresh.performed: true`
- exactly one `accounts[]` entry for provider `codex`
- `truth.localSession.state: "available"`
- token totals under `truth.localSession.usage`

Provider limit snapshots may still be `missing`. That is honest: Pylon records
provider rate-limit snapshots when the underlying SDK or event stream emits a
structured rate-limit payload; token usage is available on completed turns.

## 3. Register Multiple Local Accounts

Use named refs when you want separate Codex or Claude homes per subagent
session. Keep the actual homes local and out of tracked files.

Find the config path:

```sh
cd apps/pylon
bun src/index.ts bootstrap --json | jq -r '.paths.config'
```

Edit that config file locally and add `dev.accounts`:

```json
{
  "dev": {
    "accounts": [
      {
        "ref": "codex-a",
        "provider": "codex",
        "home": "/absolute/local/path/to/codex-a-home"
      },
      {
        "ref": "codex-b",
        "provider": "codex",
        "home": "/absolute/local/path/to/codex-b-home"
      },
      {
        "ref": "claude-a",
        "provider": "claude_agent",
        "home": "/absolute/local/path/to/claude-a-config"
      }
    ]
  }
}
```

Then verify without printing paths:

```sh
bun src/index.ts accounts list --json
bun src/index.ts accounts usage --account codex-a --json
```

If a named account is missing or invalid, fix the local config or credential
home before starting sessions. Do not paper over it by copying credentials into
the repo.

## 4. Batch Mode: Spawn N Local Subagent Sessions

Use `apps/pylon/scripts/multi-session-run.ts` when a controller wants a
repeatable batch run. It reads a plan, resolves one account per session,
materializes or accepts one workspace per session, runs `dev-proof-run.ts`,
and writes:

- per-session proof or failure artifacts
- `heartbeats.jsonl`
- `multi-session-summary.json`

Each session must use exactly one workspace selector:

- `worktreePath`: an existing local worktree/directory.
- `repoRef`: a public GitHub repository and pinned 40-character commit SHA;
  Pylon materializes an isolated detached worktree from the shared cache.

The workspace may be on a named branch or in detached HEAD. The bounded proof
verification runs over isolated worktrees, so it tolerates a detached HEAD (the
`repoRef` path materializes one); the detached state is still reported honestly
in the retained proof's change summary. The run never touches the branch or
commit. Earlier builds blocked verification on a detached HEAD with
`blocker.dev_loop.branch_unknown_or_detached`, which silently failed every
`repoRef` session; that is fixed (openagents #4873).

Each session may use exactly one account selector:

- `accountRef`: a named ref from `dev.accounts`.
- `accountHome`: a direct provider home path, for the provider implied by the
  adapter.
- `codexHome` or `claudeConfigDir`: adapter-specific direct home shortcuts.
- no account selector: use the default provider home for that adapter.

The `--account codex` and `--provider codex` shortcuts are for the
`pylon accounts usage` CLI. Multi-session plans and control-session commands
use `accountRef`, direct homes, or the default-by-omission rule.

### Automatic account failover

A session whose primary account is quota-blocked or ledger-unavailable is
replaced instantly, within the same run, by another available account — no
second pass required. The fallback pool is:

- the optional run-level `accountPool` (a top-level array on an object-shaped
  plan, `{ "sessions": [...], "accountPool": [ { "codexHome": "..." }, ... ] }`);
  plus
- every other account selector used anywhere in the plan's own sessions.

Each session tries its primary account first, then routes through that pool,
skipping accounts the quota ledger marks unavailable and recording a quota block
on any account that returns a provider usage-limit signal. Fallback members are
filtered to the session's adapter (a codex session never fails over to a Claude
account), deduped, and an unresolvable member is skipped rather than failing the
session. A session only ends in `all_accounts_exhausted` when the entire pool is
unavailable. To pin a session to exactly one account with no failover, give it
its own single selector and run a plan with no other accounts and no run-level
`accountPool`.

### Workspace provisioning and verify commands

The `verify` argv runs inside the session worktree. A fresh Git worktree or a
`repoRef`-materialized checkout has no `node_modules`, so a verify command
such as `bun test ...` fails unless the worktree is already provisioned.
Provision it by installing dependencies in that worktree, or by symlinking
`node_modules` from a sibling checkout of the same repository. If provisioning
is not part of the run, use a dependency-free verify command instead, such as a
doc existence check or another plain shell check that does not rely on project
dependencies.

Detached-HEAD worktrees are supported for proof verification. This includes the
isolated detached worktrees materialized from `repoRef`. Earlier builds
silently failed those runs with `blocker.dev_loop.branch_unknown_or_detached`;
that failure mode was fixed in openagents issue #4873.

Worked examples:

```json
{
  "sessions": [
    {
      "id": "docs-check",
      "adapter": "codex",
      "accountRef": "codex-a",
      "worktreePath": "../task-worktrees/docs-check",
      "objective": "Update the runbook and keep edits scoped.",
      "verify": ["test", "-f", "docs/autopilot-coder/pylon-multi-session-agent-runbook.md"]
    },
    {
      "id": "preprovisioned-bun-test",
      "adapter": "codex",
      "accountRef": "codex-b",
      "worktreePath": "../task-worktrees/preprovisioned-bun-test",
      "objective": "Repair the focused failing test without unrelated refactors.",
      "verify": ["bun", "test", "apps/pylon/tests/account-usage.test.ts"]
    }
  ]
}
```

For the `bun test` example, provision
`../task-worktrees/preprovisioned-bun-test` first; one local shortcut is
symlinking its `node_modules` to a sibling checkout of the same repo.

Example plan for two Codex accounts and one Claude account:

```json
{
  "sessions": [
    {
      "id": "codex-a-docs",
      "adapter": "codex",
      "accountRef": "codex-a",
      "worktreePath": "../task-worktrees/codex-a",
      "objective": "Update the focused docs section and keep edits scoped.",
      "verify": ["bun", "test", "apps/pylon/tests/account-usage.test.ts"],
      "timeoutSeconds": 600
    },
    {
      "id": "codex-b-tests",
      "adapter": "codex",
      "accountRef": "codex-b",
      "repoRef": {
        "provider": "github",
        "visibility": "public",
        "fullName": "OpenAgentsInc/openagents",
        "branch": "main",
        "commitSha": "0123456789abcdef0123456789abcdef01234567"
      },
      "objective": "Repair the focused failing test without unrelated refactors.",
      "verify": ["bun", "test", "apps/pylon/tests/account-usage.test.ts"]
    },
    {
      "id": "claude-review",
      "adapter": "claude_agent",
      "accountRef": "claude-a",
      "worktreePath": "../task-worktrees/claude-a",
      "objective": "Review the same area and leave a scoped implementation note.",
      "verify": ["bun", "test", "apps/pylon/tests/account-usage.test.ts"]
    }
  ]
}
```

Use a real commit SHA in `repoRef`; the placeholder above is invalid by
design.

Run the batch from the repository root:

```sh
PYLON_HOME="$PWD/.pylon-local" \
bun apps/pylon/scripts/multi-session-run.ts \
  --plan /tmp/pylon-multi-session-plan.json \
  --proofs-dir /tmp/pylon-proofs/multi-session \
  --pylon-home "$PWD/.pylon-local" \
  --concurrency 2 \
  --run-id run.local.codex-fanout
```

Inspect completion:

```sh
jq '{runRef,totalSessions,completedCount,failedCount,deviations,outcomes}' \
  /tmp/pylon-proofs/multi-session/multi-session-summary.json
tail -n 20 /tmp/pylon-proofs/multi-session/heartbeats.jsonl
```

Interpretation:

- `completedCount == totalSessions` means all sessions completed and their
  verification commands passed.
- `failedCount > 0` means inspect each listed failure artifact; the runner is
  failure-tolerant and continues other sessions.
- `deviation.pylon.multi_session.some_sessions_failed` is an honest retained
  deviation, not a reason to hide or rewrite artifacts.

## 5. Daemon Mode: Control A Running Pylon Node

Use daemon mode when an external orchestrator needs live session control over a
running Pylon.

Start the node from `apps/pylon`:

```sh
PYLON_HOME="$PWD/.pylon-local" bun src/index.ts node
```

The node binds loopback by default, normally `http://127.0.0.1:4716`, and
writes a bearer token under the Pylon home. In another terminal:

```sh
cd apps/pylon
BASE="http://127.0.0.1:4716"
TOKEN="$(cat .pylon-local/control-token)"
curl -s "$BASE/health" | jq .
```

Spawn a bounded Codex session:

```sh
curl -sS "$BASE/command" \
  -H "authorization: Bearer $TOKEN" \
  -H "content-type: application/json" \
  --data @- <<'JSON' | jq .
{
  "type": "session.spawn",
  "adapter": "codex",
  "accountRef": "codex-a",
  "worktreePath": "../task-worktrees/codex-a",
  "objective": "Make the focused fix, keep edits scoped, then stop.",
  "verify": ["bun", "test", "apps/pylon/tests/account-usage.test.ts"],
  "timeoutSeconds": 600
}
JSON
```

List sessions:

```sh
curl -sS "$BASE/command" \
  -H "authorization: Bearer $TOKEN" \
  -H "content-type: application/json" \
  --data '{"type":"session.list"}' | jq .
```

Stream one session's events:

```sh
SESSION_REF="session.pylon.control_session..."
curl -N "$BASE/sessions/$SESSION_REF/events" \
  -H "authorization: Bearer $TOKEN"
```

Cancel a session:

```sh
curl -sS "$BASE/command" \
  -H "authorization: Bearer $TOKEN" \
  -H "content-type: application/json" \
  --data "{\"type\":\"session.cancel\",\"sessionRef\":\"$SESSION_REF\"}" | jq .
```

Control sessions reject local danger modes. Do not send
`danger-full-access`, `bypassPermissions`, `codexDanger`, or `claudeDanger`.
Codex sessions run in `local_bounded` mode with `approvalPolicy: "never"` and
network disabled for the composer call; Claude sessions run with
`permissionMode: "acceptEdits"`. Both run the supplied verification command
and retain a proof/failure artifact under the Pylon home.

## 6. Close Out A Run

Before reporting success:

```sh
git status --short --branch
bun --cwd apps/pylon test tests/account-usage.test.ts
```

For Pylon source changes, run the full Pylon suite when time allows:

```sh
bun --cwd apps/pylon run test
```

Report:

- which accounts were selected, by hashed refs only
- which workspaces were used, by workspace refs or repo/commit refs only
- session refs and proof artifact paths
- verification command and result
- any deviations or blockers
- whether `providerTruth`, `localSession`, and `platform` usage tiers were
  available, stale, missing, or unavailable

Do not summarize raw child stdout if it contains private content. Use
artifact refs, digest refs, blocker refs, and verification results.
