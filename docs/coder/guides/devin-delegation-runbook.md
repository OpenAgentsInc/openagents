# Delegate work to Devin with Coder

Use this runbook to give an overseeing agent a repeatable way to assign work
to several Devins, inspect their results, and integrate verified changes.
The supervisor owns task selection, acceptance, and publication. Devin does
the delegated research, implementation, and task-specific testing.

The current local path is:

```text
Supervisor → Coder → program selection and independence decisions
                  → up to six Devin CLI sessions, each in its own worktree
                  → retained changes and execution records → supervisor review
```

Coder and Devin can run on your computer. This path needs no relay, database,
worker service, or Devin Cloud session. A remote executor adds a relay and
`coder-worker`; the worker then runs the same Devin CLI adapter.

The checked-in capability does not create Devin Cloud sessions through the
Cloud API. The Cloud API credential used to inspect earlier sessions is
separate from the Devin CLI login. A Cloud-hosted supervising agent can use
this runbook too, provided its machine has the required tools and boundary
backend.

## Instructions to give an overseeing agent

```text
Use docs/coder/guides/devin-delegation-runbook.md. Have Coder delegate the bounded
implementation work to Devin; supervise selection, evidence, and integration.

Inspect the checkout, credentials by presence only, executor approval, and
current work before changing anything. Preserve unrelated work. Use an isolated
integration checkout and a separate Cargo target directory. Run the six-item
smoke check before assigning implementation work.

Prepare at most six ready tasks with disjoint output paths, explicit acceptance
criteria, and a fixed base commit. Record dependencies outside the active batch.
Run Coder's burn-down program with devin-local, or devin-relay when a configured
remote worker is intended. Capture the JSON result, ATIF trace, work list, exit
code, and base commit. Check every delegation; a successful Coder exit is not
enough. Inspect actual diffs and run independent checks before accepting work.

Keep one integrator. Fetch each retained scratch repository, review only the
changes after its seeded root commit, and integrate accepted commits serially.
Resolve conflicts and test the combined result. Refill with another ready batch
until the authorized work is complete. Keep failures and incomplete work visible.
Publish only within the user's existing authorization; never force-push or let
delegates push to the shared branch. Report the commit, tests, and remaining limits.
```

## Prepare the local workstation

Use macOS with `/usr/bin/sandbox-exec`, or Linux with `bwrap` and working
user namespaces. Unsupported or unavailable enforcement refuses delegation.
Install the Devin CLI available to your account and confirm `devin --version`
and `devin auth status`. This procedure was tested with CLI `3000.10.31`.
If you need to sign in, run `devin auth login` before continuing.

Use a clean integration checkout. Delegation worktrees start at its **HEAD**;
uncommitted edits and untracked files do not become their starting tree.
Commit the authorized baseline first, or use a separate checkout when another
agent is working. Keep one integration branch and one integrator.

Set these paths in a Bash or Zsh shell. Change `REPO` to the integration
checkout you actually intend to use. Give every integration worktree its own
`CARGO_TARGET_DIR`.

```sh
umask 077
REPO="$HOME/work/openagents"
OPS="$HOME/.openagents/coder-delegation"
STATE="$OPS/state"
APPROVALS="$OPS/approvals"
EVIDENCE="$OPS/evidence"
SECRETS="$HOME/work/.secrets"
export CARGO_TARGET_DIR="$OPS/target-openagents"
mkdir -p "$STATE/xdg/devin/cli" "$APPROVALS" "$EVIDENCE" "$SECRETS"
chmod 700 "$OPS" "$STATE" "$APPROVALS" "$EVIDENCE" "$SECRETS"

cd "$REPO"
cargo build --locked -p coder -p capability -p coderbench
BIN="$CARGO_TARGET_DIR/debug"
```

The checkout pins Rust. See [verification prerequisites](../../verification.md)
for the full manual gate, additional compilers, and PostgreSQL tools.

### Configure credentials and writable state

Program selection and semantic decisions need `TYPESAFE_API_KEY`. Load it
through your existing protected environment file; Coder does not load dotenv
files itself. For example, a private `~/work/.secrets/typesafe.env` can export
that variable:

```sh
set -a
. "$SECRETS/typesafe.env"
set +a
test -n "${TYPESAFE_API_KEY:-}" || { echo "TypeSafe key is missing" >&2; false; }
```

Do not echo credentials or put their values in shell history. The local
delegation path needs the Devin CLI login and the TypeSafe key. It needs no
generation API key or locally served model. Keep the optional Devin Cloud API
token in its separate `devin-api-key` file.

The default Devin data directory can also contain its installed binary.
Granting writes to that directory would let a delegate change the adapter
whose bytes you approved, so the boundary refuses it. Copy only startup state
to a separate directory:

```sh
test -f "$HOME/.local/share/devin/credentials.toml"
cp "$HOME/.local/share/devin/credentials.toml" "$STATE/xdg/devin/"
for file in installation_id trusted_workspaces.json; do
  if [ -f "$HOME/.local/share/devin/cli/$file" ]; then
    cp "$HOME/.local/share/devin/cli/$file" "$STATE/xdg/devin/cli/$file"
  fi
done
chmod 600 "$STATE/xdg/devin/credentials.toml"
export XDG_DATA_HOME="$STATE/xdg"
export CODER_CAPABILITY_TRUST="$APPROVALS/trust.json"

# Trust this checkout under the same data directory that Coder uses.
# Answer the workspace trust prompt, then quit the interactive CLI.
(cd "$REPO" && devin)

"$BIN/capability-trust" approve devin-local --in "$REPO" --writable "$STATE"
"$BIN/capability-trust" list
```

Skip copying over an already configured isolated login when resuming. If no
default login exists, run `devin auth login` with this `XDG_DATA_HOME` instead.
Coder temporarily trusts each generated task worktree at its exact path and
withdraws that entry after the run.

The approval pins the manifest and adapter. Reapprove after reviewing an
adapter or manifest update. The **directory containing the approval file is
protected**, not only the file: `$OPS/approvals/trust.json` works with the
sibling `$OPS/state`; `$OPS/trust.json` does not. Never grant writes to the
checkout, common Git directory, approval directory, or installed binary tree.
The boundary confines filesystem writes. It does not isolate reads, inherited
credentials, network access, or external services. Use a dedicated account or
worker host if tasks require stronger separation.

## Run the six-Devin smoke check

The [smoke work list](../examples/devin-smoke.work-list.json) contains six
read-only questions with explicit expected answers. Verify those expectations
against the selected checkout when source files change. Keep an existing
`.coder/work-list.json` before replacing it. This file is local operating state
and is ignored by Git.

```sh
cd "$REPO"
mkdir -p .coder
RUN="$(mktemp -d "$EVIDENCE/smoke.XXXXXX")"
if [ -f .coder/work-list.json ]; then
  cp .coder/work-list.json "$RUN/previous-work-list.json"
fi
cp docs/coder/examples/devin-smoke.work-list.json .coder/work-list.json
cp .coder/work-list.json "$RUN/work-list.json"
git rev-parse HEAD > "$RUN/base.txt"
git status --porcelain=v1 > "$RUN/before.status"

unset CODER_DOOR_KEY CODER_AI_GATEWAY_KEY CODER_DOOR_URL CODER_MODEL
unset CODER_WORKER CODER_RELAY CODER_EXECUTOR CODER_SOURCE_DIR
export CODER_DELEGATE=devin-local CODER_SHELL=off
export CODER_PROGRAMS=burn-down,project-task
export CODER_PROGRAM_EFFECTS=reads,writes,delegation,network,subprocesses,spend

if "$BIN/coder" -p --json --trace "$RUN/trace.atif.jsonl" \
  "Run the burn-down program for every item in this checkout's work list." \
  > "$RUN/result.json" 2> "$RUN/progress.log"; then
  code=0
else
  code=$?
fi
printf '%s\n' "$code" > "$RUN/exit-code.txt"
test "$code" -eq 0
python3 scripts/check-coder-delegation-run.py \
  --result "$RUN/result.json" --trace "$RUN/trace.atif.jsonl" \
  --program burn-down --expected 6
git status --porcelain=v1 > "$RUN/after.status"
diff -u "$RUN/before.status" "$RUN/after.status"
```

Stop on any failed command. Each invocation uses a fresh trace path. Inspect
`progress.log`, `result.json`, and the trace on failure; do not overwrite them
on retry. Restore the saved work list after the smoke test if there was one.

Require six completed, answered delegations with six matching expectations.
Independently check the six answers against the source files too. The checker
checks recorded execution and completion evidence; it does not independently
verify code changes. The status comparison detects ordinary checkout changes,
not every possible external side effect. For the stronger snapshot-based
historical benchmark, use [CoderBench](../measurements/2026-09-20-observed-fanout.md)
at its pinned base; do not advance its expectations merely to make a run pass.

`program` must be `burn-down`. A plain answer, a stub response, or `program:
null` means this procedure did not run. Exit `0` means the top-level turn
answered; it can still contain six refused tasks. `CODER_SHELL=off` disables
the ordinary Coder command loop, not program delegation or the Devin tools.

## Prepare implementation batches

Choose **at most six ready items**. Give each one an acceptance criterion,
bounded scope, relevant input paths, owned output paths, and targeted checks.
Use similar-sized tasks to reduce time spent waiting for the last task.
The current `burn-down` program caps the list and active delegates at six,
runs each in a worktree, and applies a 30-minute executor deadline per task.
Decision calls, checkout preparation, and review add time; this is not a
whole-run deadline or a spending limit.

Example `.coder/work-list.json`:

```json
{
  "v": 1,
  "work": [
    {
      "id": "parser-empty-input",
      "prompt": "Fix the specified empty-input parser defect in crates/example/src/parser.rs. Add the focused regression in that file and run the relevant package test. Change only that file. Commit to the supplied scratch Git repository. End with Final answer: done only after the checks pass; otherwise report the blocker.",
      "reads": "crates/example/src/parser.rs",
      "writes": true,
      "touches": ["crates/example/src/parser.rs"],
      "after": [],
      "expects": "done"
    }
  ]
}
```

Replace this illustrative path and defect with real work. `expects: "done"`
checks the final report only; it is never proof that the patch is correct.
The program briefs Devin about `.coder-git` and extracts the last
`Final answer:` line for comparison, retaining earlier text as narration.
The source orders items by `id`. `reads` is a single string and is also folded
into the collision record; list additional known paths in `touches`. Name each
task's allowed output paths explicitly in its prompt.

Before dispatch:

1. Verify prerequisites in the actual checkout and issue tracker. An `after`
   reference to another item still in the list excludes that dependent from
   this batch, but absence from the list does not prove its prerequisite is
   complete. Keep blocked work outside the ready batch.
2. Check writes against other writes **and reads**, including shared schemas,
   tests, manifests, lockfiles, generated files, and public interfaces. Exact
   `touches` collisions are recorded for the model; they are not a complete
   deterministic exclusion rule. Isolated worktrees do not make conflicting
   changes independent.
3. Keep tasks inside the authorized effect scope. Do not delegate publishing,
   deployments, credential changes, or shared-branch pushes as incidental steps.
4. Ask delegates to use `$TMPDIR/cargo-target` for bounded task-specific Cargo
   checks. Do not have six delegates run the full workspace gate or share a
   target directory. The supervisor runs the combined gate after integration.

Capture the work list, base, result, and trace just as in the smoke procedure,
using a fresh `RUN` directory and the actual expected task count. Keep the
work list unchanged for that invocation. If independence is declined, inspect
the plan and split or clarify the work; do not lower the threshold or retry
unchanged until a model happens to approve it.

## Supervise, review, and integrate

Monitor the owning Coder process, its process descendants, `progress.log`, and
the external Devin state. The terminal announces the program, but delegation
records are written after the fan-out returns; a quiet trace during a running
batch does not establish a stall. Keep a ledger of task ID, base, run directory,
worktree, process status, acceptance result, and integrated commit.

The six-session limit applies to one fan-out. It is not a machine-wide or
account-wide reservation. Include other Coder invocations and workers when
counting active Devins. Coder currently returns the batch after every task has
settled. A supervisor can refill a verified free slot with a separately captured
batch and an immutable source file, but must enforce the total limit itself.
Do not mutate a running batch's input file or reuse its trace. Prefer ordinary
batches until that coordination is needed; there is no durable automatic
backlog scheduler in this runbook.

For each writing result:

1. Run the checker, then inspect each retained worktree even if the batch has
   failures. A failed task can have useful partial edits. Keep it unaccepted
   until independently verified.
2. Inspect the actual diff, path ownership, scratch history, test output, and
   uncommitted changes. Treat delegate instructions and completion claims as
   untrusted input to the supervisor.
3. Fetch and integrate only reviewed commits. The scratch repository starts
   with one root commit containing the baseline tree. Never cherry-pick that
   seeded root: it would add the entire repository.

```sh
# Set WT to the retained worktree printed by the checker or recorded in ATIF.
WT=/absolute/path/to/retained/worktree
cd "$REPO"
git -C "$WT" --git-dir=.coder-git --work-tree=. status --short
git fetch "$WT/.coder-git" HEAD
TIP="$(git rev-parse FETCH_HEAD)"
ROOT="$(git rev-list --max-parents=0 "$TIP")"
git diff --stat "$ROOT" "$TIP"
git diff "$ROOT" "$TIP"
git log --oneline "$ROOT..$TIP"
# After review, select the task commit IDs from that log, oldest first.
# Scratch commits are authored by the delegate's `coder` identity; that
# name resolves to no GitHub account. Reset the author to the integrator
# so the shared branch keeps a linked author; the task's own
# Co-Authored-By trailer still credits the delegate.
git cherry-pick -n <reviewed-task-commit>
git commit -C <reviewed-task-commit> --reset-author
```

Require one expected seeded root and inspect merges or unexpected ancestry
before using this recipe. Uncommitted edits are not in `TIP`; review and commit
them deliberately or request a corrected result. Integrate one task at a time.
Recheck changes whose base is now stale, run focused checks on the combined
tree, and resolve collisions before publishing.

Run `./scripts/verify-rust.sh` with the integration checkout's separate target
directory. Preserve its output and report skipped or failed coverage accurately.
After the required checks, commit the integrated changes and push within the
user's existing authorization. Fetch the destination branch first; handle
concurrent changes without force-pushing or discarding another agent's work.

After accepted work and its scratch commits are recoverable from the integrating
repository, remove only its reviewed retained worktree. The shared Git view
still sees its task edits and `.coder-git` as changes, so removal requires
`git worktree remove --force "$WT"`. Check the path and recoverability first;
this deletes that worktree's files. Never apply it to unreviewed output or
blanket-delete `.coder/worktrees`.

On a timeout or interruption, inspect task worktrees and process descendants
before retrying. Local supervised subprocesses have deadline and cancellation
cleanup, but abrupt termination can leave worktrees or temporary trust entries.
Remote work can continue after the coordinator disconnects, up to the worker's
bound. Do not start duplicate writing tasks until their earlier execution is
accounted for. Keep completed items out of a retry batch.

## Use a remote executor

Use this option when the Devin CLI runs on another machine. Both machines need
the same baseline checkout; Coder does not upload the repository to the worker.
The worker needs its own CLI login, writable state, capability approval, and
boundary backend. The coordinator needs the decision key and its own Nostr
identity; it does not need the worker's Devin credentials.

Use a reachable relay such as `wss://relay.openagents.com`, or follow the
[disposable local relay procedure](../../deployment/runbook-local-dev.md) for a
test relay backed by PostgreSQL. Set `NOSTR_RELAY_BIND_ADDR=127.0.0.1` for a
loopback test and start the binary from your `CARGO_TARGET_DIR`. Jobs are
encrypted NIP-CJ events; they are ephemeral, so the worker must be connected
before dispatch. The relay is transport, not a durable task queue.

Generate separate worker and coordinator identities, store private keys in
mode-`0600` files outside the checkout, and exchange only public keys. Coder
can create its coordinator key at `~/.openagents/nostr-secret` on first relay
use, or read `CODER_SECRET_KEY`/`CODER_NSEC`. To derive a public key without
dispatching work, run `coder-worker --check` in a clean environment with that
key as `CODER_WORKER_SECRET`, a loopback `CODER_RELAY`, and no door configured;
read the public `worker` line. A `stub` door in this identity-only check is
expected. A production executor check must instead say `executor (devin-local)`.

For example, create an identity on each respective host without displaying its
secret. Run this once with `IDENTITY=worker` on the worker and once with
`IDENTITY=coordinator` on the coordinator:

```sh
IDENTITY=worker
KEY_FILE="$SECRETS/coder-$IDENTITY-secret"
if [ ! -e "$KEY_FILE" ]; then
  (umask 077; openssl rand -hex 32 > "$KEY_FILE")
fi
chmod 600 "$KEY_FILE"
(
  unset CODER_DOOR_KEY CODER_AI_GATEWAY_KEY CODER_EXECUTOR CODER_WORKER
  unset CODER_MODEL CODER_WORKER_MODEL CODER_WORKER_ALLOW CODER_WORKER_JOBS
  export CODER_RELAY=ws://127.0.0.1:7447
  export CODER_WORKER_SECRET="$(cat "$KEY_FILE")"
  "$BIN/coder-worker" --check
)
```

Only the public `worker` line is exchanged, even when deriving the coordinator's
key. If you created `coder-coordinator-secret`, load it as
`CODER_SECRET_KEY` in the coordinator shell before the relay invocation;
otherwise Coder would use its default identity instead of the allowlisted one.

Worker environment, after completing the local approval setup on that host:

```sh
export XDG_DATA_HOME="$STATE/xdg"
export CODER_CAPABILITY_TRUST="$APPROVALS/trust.json"
export CODER_CAPABILITY_DIR="$REPO/capabilities"
export CODER_EXECUTOR=devin-local CODER_EXECUTOR_WORKDIR="$REPO"
export CODER_EXECUTOR_MINUTES=10 CODER_WORKER_JOBS=6
export CODER_RELAY='wss://relay.openagents.com'
export CODER_WORKER_ALLOW='<coordinator-public-key>'
export CODER_WORKER_SECRET="$(cat "$SECRETS/coder-worker-secret")"
unset CODER_DOOR_KEY CODER_AI_GATEWAY_KEY CODER_WORKER
"$BIN/coder-worker" --check
"$BIN/coder-worker" 2> "$EVIDENCE/worker.log"
```

Read the check's door, job limit, and allowlist. `--check` does not prove relay
connectivity or task success. Wait for `subscribed; jobs arrive live from here`.
Use an allowlist even in a local test; a public worker without one fails the
configuration check. The job limit refuses overflow with `busy`. A delegation's
stated minutes control that job; `CODER_EXECUTOR_MINUTES` covers ordinary jobs
without a delegation bound. The worker also imposes a completion guard.

Coordinator environment, using the worker's public key from its startup log:

```sh
export CODER_RELAY='wss://relay.openagents.com'
export CODER_WORKER='<worker-public-key>'
export CODER_DELEGATE=devin-relay
export CODER_PROGRAMS=burn-down
export CODER_PROGRAM_EFFECTS=reads,writes,delegation,network,subprocesses,spend
unset CODER_DOOR_KEY CODER_AI_GATEWAY_KEY CODER_MODEL CODER_EXECUTOR
# Repeat the captured smoke invocation and checker above with these settings.
# Do not repeat the local block that unsets CODER_WORKER and CODER_RELAY.
```

`CODER_DELEGATE=devin-relay` is required. Setting only `CODER_WORKER` and
`CODER_RELAY` can route generation while delegation still runs locally. Verify
each trace call's capability and `relayed.request`; join those IDs to the
worker's logs. The coordinator does not observe the worker's filesystem boundary
or snapshots, so do not claim locally verified remote artifacts.

Remote writing worktrees and scratch commits remain on the worker. Have the
supervisor inspect them there, create a Git bundle of reviewed scratch history
or transfer it over an authorized SSH connection, and apply the same seeded-root
review before integration. NIP-CJ does not automatically return a patch bundle
or a resumable remote task handle. See [worker operation](worker-executor.md)
and [service deployment](../../../deploy/README.md) for long-lived workers.

## Readiness and troubleshooting

This is a supervised dogfooding workflow. The supervisor compensates for the
following open implementation work; successful small batches do not close it:

| Work | Current responsibility | Tracking issue |
| --- | --- | --- |
| Program-wide authorization | Review intended effects independently of model selection. | [#9504](https://github.com/OpenAgentsInc/openagents/issues/9504) |
| Scoped tracker intake | Inspect issues and write the ready work list. | [#9507](https://github.com/OpenAgentsInc/openagents/issues/9507) |
| Conflict enforcement and scheduling | Check shared paths, dependencies, and total active sessions. | [#9508](https://github.com/OpenAgentsInc/openagents/issues/9508) |
| Independent completion | Inspect actual artifacts and run checks; text matching is insufficient. | [#9509](https://github.com/OpenAgentsInc/openagents/issues/9509) |
| Durable execution and whole-run budgets | Retain the ledger, reconcile interruptions, and bound retries and spend. | [#9510](https://github.com/OpenAgentsInc/openagents/issues/9510) |
| Full Decision Router integration | Current decisions use the Jev client; gateway profiles and receipt joins remain planned. | [#9502](https://github.com/OpenAgentsInc/openagents/issues/9502) |

The capability charges the operator's account and does not enforce a monetary
budget. Six is the current configured concurrency ceiling, not a measured
account entitlement or an optimal throughput claim. Lower task count when
account limits, memory, build contention, or available independent work demand
it. A semantic probability above the program's `0.7` floor is not proof of
independence. `requires_scorable_answer` checks probability-bearing model
output, not an admitted calibration map. The old `requires_calibration` bound
is refused; update the program deliberately and retain its previous digest in
historical runs.

| Symptom | Next action |
| --- | --- |
| `program: null` or ordinary prose | Check the TypeSafe environment, program selection record, and capability probe. No delegation has been established. |
| `untrusted_workspace` | Trust the exact checkout under the run's `XDG_DATA_HOME`. |
| `executor_state_not_writable` | Put `XDG_DATA_HOME` inside the approved state grant. |
| `boundary_unavailable` | Read the named paths; separate writable state from checkout, binary, and approval-store parent. Check the platform backend. |
| `unapproved` | Inspect changed manifest/adapter bytes and reapprove the intended version. |
| `too_many_results` or `no_tasks` | Correct the work list and dependency filtering; dispatch only the ready bounded batch. |
| Independence below the floor | Inspect overlaps and hidden prerequisites. Rescope the tasks without weakening the bound. |
| Exit `0` with refused, failed, or unverifiable delegates | Reject the batch through the checker, inspect each outcome, and retry only unresolved work after fixing the cause. |
| Empty or unchanged writing result | Inspect actual edits and the writing invocation; do not accept a `done` claim alone. |
| `busy` | Wait for a verified free worker slot; there is no automatic queue. |
| `worker_absent` or `worker_stalled` | Check worker subscription, identity, allowlist, relay connectivity, and job logs before retrying. |
| Scratch commit includes the whole repository | Find the seeded root and inspect only later commits; do not integrate the root. |
| Timed-out or interrupted writing task | Preserve its worktree and reconcile the old attempt before dispatching a replacement. |

The [consumer vision](../design/coder-as-decision-router-consumer.md) describes the
remaining product work. The [workstation verification record](../verification/2026-09-20-devin-runbook.md)
records the actual batches used to validate this procedure and their limits.

## Read an earlier Devin Cloud session

This is an optional research step, separate from launching local Devins. Store
the Cloud API token in `~/work/.secrets/devin-api-key`, with directory mode
`0700` and file mode `0600`. Read it from the file into the HTTP client's
authorization header. Do not put its value in a prompt, command argument,
repository file, issue, or trace.

Use `GET https://api.devin.ai/v3/self` to identify the credential's organization.
For an organization-scoped credential, the response includes `org_id`. Read
`GET /v3/organizations/{org_id}/sessions/{devin_id}` for session metadata, then
`GET /v3/organizations/{org_id}/sessions/{devin_id}/messages?first=200` for
messages. Follow `end_cursor` with the `after` parameter while `has_next_page`
is true. Use the API's full session identifier, including the `devin-` prefix.
Credentials with different scopes might require an explicitly selected
organization or additional permissions. See the official
[session endpoint](https://docs.devin.ai/api-reference/v3/sessions/get-organizations-session)
and [message endpoint](https://docs.devin.ai/api-reference/v3/sessions/get-organizations-session-messages).

Keep downloaded transcripts outside the checkout in a private directory.
They can contain login codes and authentication links. Extract operational
lessons without publishing the raw transcript. A parent's `child_session_ids`
records hosted child sessions; it does not prove that Coder's local delegation
path was used. Verify the actual capability and transport in Coder's trace.

## Continuous project supervision

For project polling, durable claims, resource-aware admission, and completion-driven
refill, use the [project supervisor procedure](project-supervision.md). The local
executor and protected approval/state setup in this runbook still apply. Program
authority is explicit; see [program authority](program-authority.md).
