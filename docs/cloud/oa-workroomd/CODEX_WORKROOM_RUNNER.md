# oa-workroomd Codex Workroom Runner

Status: Cloud MVP scaffold for `CND-045`

`oa-workroomd codex run` is the Cloud-side runner for single-turn agent VM
workrooms. The command supports two agent runtimes:

- `opencode_codex`: the default production path. `oa-workroomd` converts the
  materialized Codex connected-account cache into `OPENCODE_AUTH_CONTENT` and
  launches `opencode run --format json --model openai/gpt-5.5`. The model is
  configurable with `OA_OPENCODE_CODEX_MODEL`, but this runtime intentionally
  rejects non-`openai/` model selectors because it must use the OpenAI/Codex
  subscription auth path.
- `codex`: the raw Codex CLI fallback path. `oa-workroomd` launches
  `codex exec --skip-git-repo-check --json`.

Both modes use the same per-run Codex auth grant, workspace, artifact policy,
event log, usage receipt, and cleanup path. `oa-workroomd codex session ...` is
the long-running workroom path for SHC/Vortex runs that need to preserve
workspace state across turns while still scrubbing `CODEX_HOME` auth material
after each active turn.

This runner is intentionally narrow:

- one structured assignment;
- one private no-wallet workspace;
- one session-scoped `CODEX_HOME`;
- `opencode run --format json --model <openai GPT-5 model>` by default, or raw
  `codex exec` when requested;
- declared artifacts only;
- redacted event and receipt logs;
- workspace and auth cleanup after completion or failure.

## One-Shot Command

```bash
oa-workroomd codex run \
  --assignment-file ./codex-workroom-assignment.json \
  --agent-runtime opencode_codex \
  --codex-bin codex \
  --opencode-bin opencode \
  --state-dir ./workroom-state \
  --json
```

Use `--stream-jsonl` instead of `--json` when the caller wants the normalized
event stream in JSONL form.

## Session Commands

Use the session command family when Vortex or another control plane needs a
Codex workroom that can pause, continue, close out, archive, and destroy later:

```bash
oa-workroomd codex session create \
  --assignment-file ./codex-workroom-assignment.json \
  --ttl-ms 86400000 \
  --state-dir ./workroom-state \
  --json

oa-workroomd codex session start-turn \
  --grant-file ./codex-auth-grant-turn-1.json \
  --auth-json-file ./brokered-auth-cache.json \
  --codex-bin codex \
  --state-dir ./workroom-state \
  --json

oa-workroomd codex session continue-turn \
  --prompt "Continue from the prior workspace state." \
  --grant-file ./codex-auth-grant-turn-2.json \
  --auth-json-file ./brokered-auth-cache.json \
  --codex-bin codex \
  --state-dir ./workroom-state \
  --json

oa-workroomd codex session status --state-dir ./workroom-state --json
oa-workroomd codex session events --cursor 0 --state-dir ./workroom-state --json
oa-workroomd codex session closeout --state-dir ./workroom-state --json
oa-workroomd codex session archive --state-dir ./workroom-state --json
oa-workroomd codex session destroy --state-dir ./workroom-state --json
```

The session path stores:

| File | Contents |
| --- | --- |
| `codex-session-state.json` | Assignment, preserved workspace path, status, turn index, TTL/archive/destroy timestamps, artifact refs, receipt refs, event refs. |
| `codex-session-events.jsonl` | Normalized `openagents.codex_workroom_event.v1` events for the multi-turn session. |
| `openagents-runner-events.jsonl` | Typed OpenAgents runner events for Vortex/Probe ingestion, including messages, shell commands, tool calls, file edits, artifacts, receipts, usage availability, and terminal state. |
| `resource-usage-receipts.jsonl` | `openagents.resource_usage_receipt.v1` receipts with host/device facts, run resource facts, and explicit subscription-backed Codex token-usage unavailability. |
| `codex-workspaces/<assignment>/` | Preserved private workspace across turns until `session destroy`. |
| `codex-auth-state.json` / `codex-auth-receipts.jsonl` | Fresh turn-scoped auth materialization/status/scrub receipts. The auth directory is removed after each turn. |
| `artifact-state.json` / `artifact-receipts.jsonl` | Artifact policy, upload, and closeout state when `session closeout` is called. |

`session start-turn` and `session continue-turn` materialize a fresh auth grant
from the supplied brokered auth cache, verify `codex login status`, run
`codex exec`, and scrub the turn-scoped `CODEX_HOME` before returning. They do
not remove the workspace. `session closeout` captures the declared artifacts
from the preserved workspace and emits closeout receipts. `session destroy`
removes the preserved workspace and any local Codex auth roots while leaving a
destroyed session record for audit.

## Required Setup

Before `codex run` starts, Cloud must materialize and verify the matching
Codex auth grant:

```bash
oa-workroomd codex auth materialize \
  --grant-file ./codex-auth-grant.json \
  --auth-json-file ./brokered-auth-cache.json \
  --state-dir ./workroom-state \
  --json
```

Production SHC control resolves the grant through Omega with
`includeAuthMaterial: true`, then writes the returned OAuth JSON into the
current run directory before calling `oa-workroomd`. Account-scoped local auth
files are only a fallback for local development and older bootstrap paths. If a
browser reconnection succeeds but SHC keeps using the old account-scoped file,
OpenAI can reject the stale refresh path with `token_invalidated`.

`codex run` calls `codex login status` again under the session `CODEX_HOME`
before it starts `codex exec`. It refuses the assignment if the grant,
provider account, workroom, or login status do not match. The runner passes
`--skip-git-repo-check` because VM workrooms are intentionally fresh private
directories and may not contain a git checkout until the assignment asks Codex
to create or clone one.

When the `opencode_codex` path converts a materialized Codex `auth.json` into
`OPENCODE_AUTH_CONTENT`, it preserves a real token expiry when the brokered
cache provides one. If the cache omits expiry metadata, `oa-workroomd` sets the
OpenCode expiry to `0` so OpenCode refreshes the ChatGPT/Codex account token
before the first OpenAI request. Do not synthesize a future expiry for a cache
that did not include one; that can make OpenCode reuse an invalidated access
token and produce a `token_invalidated` run failure immediately after
reconnect.

The SHC real-account smoke uses `danger_full_access`, which maps to Codex
`--sandbox danger-full-access`, because this nested VPS currently fails Codex's
Linux `workspace-write` sandbox at the bubblewrap/loopback layer. Keep that
profile explicit in the assignment and rely on the no-wallet VM/workroom
boundary until SHC has a working Codex `workspace-write` sandbox profile.

## Files

| File | Contents |
| --- | --- |
| `codex-run-state.json` | Assignment refs, workspace path, status, artifact refs, receipt refs, event refs. |
| `codex-run-events.jsonl` | Normalized `openagents.codex_workroom_event.v1` events. |
| `openagents-runner-events.jsonl` | Typed OpenAgents runner events projected from Codex JSON/text output and Cloud artifact/receipt handling. |
| `resource-usage-receipts.jsonl` | Resource/model usage receipts for the one-shot run. |
| `codex-workspaces/<assignment>/` | Temporary private workspace, removed after run cleanup. |
| `artifact-state.json` / `artifact-receipts.jsonl` | Existing artifact policy, upload, and closeout state. |
| `closeout-manifest.json` | Existing closeout manifest for declared artifacts. |

The workspace contains an `AGENTS.md` file that states the workroom id,
assignment id, target node, no-wallet policy, sandbox mode, and declared
artifacts.

## Event Contract

The runner emits Probe-compatible events:

```text
queued -> started -> log/redacted -> artifact -> receipt -> completed -> cleanup
```

Failure paths emit `failed`, `timeout`, or `cleanup` failure events. Output
lines that contain forbidden markers are replaced with a `redacted` event.

The compatibility event stream remains receipt-oriented. The product event
stream lives in `openagents-runner-events.jsonl` and uses stable string event
types that Vortex can map to messages, tool calls, shell commands, artifacts,
receipts, and checkpoints:

```text
run.queued
run.started
run.heartbeat
turn.started
message.delta
message.completed
tool.call.started
tool.call.delta
tool.call.completed
shell.command.started
shell.output.delta
shell.command.completed
file.edit
artifact.created
receipt.created
turn.completed
ThreadTokenUsageUpdated
opencode.step-finish
opencode.session.next.step.ended
resource.usage.captured
usage.unavailable
run.waiting_for_input
run.failed
run.timed_out
run.cancelled
run.completed
redacted
```

The runner preserves only bounded excerpts plus digest refs. SDK
`turn.completed.usage`, app-server `thread/tokenUsage/updated` /
`ThreadTokenUsageUpdated`, and OpenCode `step-finish` /
`session.next.step.ended` payloads are forwarded as product runner events and
used to populate the resource usage receipt. OpenCode events keep the raw
provider/model/tokens payload so Vortex can ledger cache read/write detail
across OpenCode-supported models. If the producer does not expose token usage
for the turn, the runner emits `usage.unavailable` with an
`openagents.resource_usage_receipt.v1` receipt ref instead of inventing token
counts or per-token cost. That receipt also records host/device facts,
workspace/artifact/log byte counts, wall time, exit state, sandbox profile,
KVM/Firecracker candidate facts, and the redacted provider-account ref.

See `docs/contracts/openagents.resource_usage_receipt.v1.md`.

## Git Writeback (cloud#96)

A managed coding run on an ephemeral VM previously produced artifacts and a
resource receipt but never pushed its code changes, so the work was lost on
teardown. `oa-workroomd codex run` now runs a git writeback step after a
successful run and **before** workspace cleanup:

1. If the workspace is not a git checkout, writeback is skipped
   (`git.writeback.skipped`).
2. If no GitHub write token is in the run environment, writeback is skipped
   (`git.writeback.skipped`) — these runs behave exactly as before.
3. If the working tree has no changes (`git status --porcelain` empty), the step
   emits `git.writeback.no_changes`.
4. Otherwise it stages all changes (`git add --all -- .`), commits with a
   message referencing the run plus the standard
   `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
   trailer, and pushes `HEAD:refs/heads/<branch>` to `origin`, where `<branch>`
   is resolved from the assignment `repo_ref` (`owner/repo@ref`, defaulting to
   `main`).

On success it emits a refs-only `git.writeback.completed` runner event and
writes a `git-writeback.json` receipt carrying only the commit sha and branch
ref. Committer identity is set per-invocation with `git -c`, never in global
config. The token is supplied to `git push` only through the askpass helper and
the process environment; it never enters the commit, git config, remotes,
events, the receipt, or logs (INVARIANTS.md "Capability And Secret Handling").

### Write token sources

The writeback (and the existing repo checkout) accept a GitHub write token from,
in order:

1. `GITHUB_TOKEN` — the run-scoped credential materialized by `oa-codex-control`
   from a `github_write_grant_ref`;
2. `GH_TOKEN` — alias for the same;
3. `OA_CODEX_GITHUB_TOKEN` — a statically-configured fallback for
   operator-driven runs without a grant resolver.

All three are process-environment only. Assignments with no write token run
exactly as today (no writeback, no checkout).

## Security Rules

- No VM-global `~/.codex` login is used.
- No wallet authority is passed into the workspace.
- No broad GCP credentials are passed into the workspace.
- `CODEX_HOME` points to the session-scoped auth directory from CND-046.
- The one-shot workspace is removed after `codex run`.
- The session workspace is preserved across turns and removed only by
  `codex session destroy`.
- The turn-scoped Codex auth directory is scrubbed after every one-shot run or
  session turn.
- Event logs and state files contain refs, digests, sanitized messages, and
  decisions only.

## Tests

`crates/oa-workroomd/tests/codex_run.rs` covers:

- session auth materialization before a run;
- `codex login status` verification before `codex exec`;
- artifact capture and closeout;
- workspace cleanup;
- auth cleanup;
- redaction of fake secret-looking process output.
- multi-turn session create/start/continue/events/closeout/archive/destroy;
- workspace preservation across session turns;
- auth scrub after each session turn;
- cancel marking without leaving auth material.
- resource usage receipt emission for one-shot and session turns;
- observed Codex and OpenCode token usage events plus receipt-backed
  `usage.unavailable` fallback when no usage payload is emitted.

## GCP VM Smoke

On 2026-06-01, `oa-gcp-shc-katy-01` was reachable and running with
`/dev/kvm` present. The VM was prepared with current Rust via rustup and
`codex-cli 0.135.0`.

The Cloud source was copied to the VM without `.git` or `target`, and the
narrow runner test passed there:

```bash
cargo test -p oa-workroomd --test codex_run -- --nocapture
```

That smoke uses the test fake-Codex executable to prove assignment validation,
session auth state, event redaction, artifact capture, closeout, workspace
cleanup, and auth cleanup on the VM. A real account-backed Codex run still
requires a Vortex-issued provider-account grant and server-side brokered
auth material.

## SHC VM Smoke

On 2026-06-01, `oa-shc-katy-01` became reachable as `ubuntu@23.182.128.195`
and repeated the GCP setup lane with SHC-specific host evidence:

```text
OS: Ubuntu 24.04.4 LTS
CPU/RAM/disk: 16 vCPU, 62 GiB usable RAM, 247 GiB root disk
KVM: /dev/kvm present; kvm-ok reports KVM acceleration can be used
Rust: rustc/cargo 1.96.0 via rustup stable
Node/npm: Node 18.19.1, npm 9.2.0
Codex: codex-cli 0.135.0 from the openai/codex static GitHub release tarball
```

Do not install Codex CLI for this lane with Cargo. On the SHC host,
`cargo install codex-cli --version 0.135.0 --locked` failed because the
crate/version was not available from crates.io. Use the static release tarball
install path documented in `docs/bootstrap/CND-041-shc-katy-01-bootstrap.md`.

Install `bubblewrap` for host diagnostics, but do not assume it makes Codex
`workspace-write` usable on this nested VPS. On 2026-06-01, SHC returned
`loopback: Failed RTM_NEWADDR` inside the Codex sandbox; `danger_full_access`
was the working profile for the first real account-backed run.

The Cloud source was copied to `~/openagents-cloud` without `.git` or `target`.
Both smoke checks passed on SHC:

```bash
scripts/verify-bootstrap.sh
cargo test -p oa-workroomd --test codex_run -- --nocapture
```

The fake-Codex runner test proves the same VM-local behavior as the GCP smoke:
assignment validation, session auth state, event redaction, artifact capture,
closeout, workspace cleanup, and auth cleanup. It still does not prove a real
ChatGPT/Codex account-backed run; that requires the Vortex-issued provider
account grant and brokered auth material.

## Product Boundary

This command is a VM runner scaffold, not the final product API. Vortex should
control it through a Cloud API, SSE stream, WebSocket, or runner service. Probe
should take over durable coding-agent runtime behavior once its adapter is
ready. The event contract here is the compatibility layer between the temporary
Cloud runner and the future Probe-owned runtime.
