# Solver computer migration

2026-10-16. Implementation and qualification for
[#11807](https://github.com/OpenAgentsInc/openagents/issues/11807).
The [verification record](solver-computers-evidence.json) lists the tested source,
checks, deployed revisions, retained logs, and remaining GitHub account requirement.
The web, Computers service, and dispatcher run the tested source in production;
the web and Computers service also run it in staging. Explicit rollout admission
remains empty.

Solver's job queue, repository permission, model payer, and result handling stay
with the work service. The new computer provider uses
[`openagents-sdk`](../../crates/openagents-sdk/), including its saved mutation
requests, command references, output readers, and observed lifecycle states.
The [public Computers contract](computers.md) supplies the execution boundary.

## Job selection and ownership

The rollout settings apply when a job is submitted:

| Setting | Meaning |
| --- | --- |
| `OPENAGENTS_SOLVER_PUBLIC_ACCOUNTS` | Comma-separated account IDs whose new Solver jobs use the shared client. The account also needs existing cloud access. |
| `OPENAGENTS_SOLVER_PUBLIC_IMAGE` | Solver container reference ending in `@sha256:` and its complete digest. It is saved on the job. |
| `OA_SOLVER_STATE` | Dispatcher's persistent job directory. The service unit uses `/var/lib/oa-solver-dispatch/jobs`. |

For an enrolled account, submission requires its own repository broker ticket,
a saved and selected environment for that repository, and pull-request mode.
The newest updated matching environment supplies the selected version. The
saved binding includes its project, branch, source commit and digest, image,
runtime, build and verification evidence, and version number. Missing access or
setup refuses before allocation. The job keeps the requester account,
workspace, membership epoch, and repository-permission digest.

Public jobs have `wrc` IDs, records under `work-runs/public-computers/runs/`,
and a separate `work-runs/public-computers/queue.json`. Older web processes
reject those IDs and cannot take their queue entries. Existing `wrk` records
keep their original provider. New workers advertise `computers: true` when
claiming work; they can also finish older jobs through the original path.

The account's computer list shows the shared computer while it exists. The
work-run response names its computer, environment version, source, project,
branch, account attribution, model payer, limits, and observed cleanup status.
Vertex calls still bill the operator's `openagentsgemini` project. A missing
machine bill remains unknown; it is not replaced with zero or a model-only
total. Repeated job reports update one result rather than adding charges.

## Requests, source, and results

The logical create key is `solver-JOB_ID`; the command key is
`solver-command-JOB_ID`. Save both requests before sending them. The dispatcher
uses an exclusive per-job file lock and atomically writes and syncs its private
journal, including the computer, command, parser state, and output cursor.
A lost reply uses the same request and identity. It never changes a request's
size or command to retry it.

The command uses the digest-pinned Solver container and the existing Solver
engine. Its entrypoint checks out the saved source commit, prepares a local
immutable `origin/main` for Solver's fetch, and keeps the GitHub push URL.
An advancing GitHub branch cannot change that starting commit. This path
requires an open issue; closed-issue replay remains on the original path.
The frozen Coder binaries are unchanged.

The public command limit is 3,600 seconds. Creation and observed cleanup each
have a 600-second wait. Output retains a 32 KiB progress prefix and a final JSON
line of at most 192 KiB, within the service's 256 KiB output bound. A successful
result requires a successful command, a complete Solver summary, and no output
gap. A missing summary, command failure, or uncertain response cannot become
success. The existing job owner decides whether the result completes the job.
An `already_done` summary keeps the existing completion behavior without
claiming that a new pull request opened.

## Credentials

The work service issues an `scj_` credential for the saved job. The SDK imports
it with `Client::authenticate_computer_job`. Its signature binds the requester,
host, repository, issue, source, environment, payer, and create request. It
allows that computer and previously registered commands. Account reads, other
computers, file transfers, and other API groups refuse it. Current repository
access is checked before create, wake, credential preparation, or command
execution. Cancellation and cleanup remain available after access ends.

The dispatcher prepares command credentials through its authenticated work-host
route. Public command requests contain a reference; secret values enter only
the private command channel and `/run` on the computer. The account's session
and the work-host credential stay out of the computer and journal.

Vertex refresh uses a separate `scv_` credential that reads only the current
job's token. The shared job store keeps that token encrypted with `oa-seal`
under the deployment's existing custody keyring, with account/job/host metadata
as associated data. Its delivery record expires after 150 seconds and is
removed when the job ends. Worker and guest refresh every 60 seconds. This
supports web restarts and multiple instances while keeping plaintext out of
saved requests, images, exports, and evidence.

Run
[`runtime-identity.sh`](../../deploy/solver-box/dispatcher/runtime-identity.sh)
before deploying the dispatcher. It provisions `oa-solver-runtime`, with
`aiplatform.endpoints.predict` and `serviceusage.services.use` on the project,
and Artifact Registry Reader on the `openagents` repository. The dispatcher
can mint its one-hour tokens. The runtime identity has no dispatcher-secret
access. Both provider paths use this identity for credentials sent to a
computer. Google's
[short-lived credential API](https://docs.cloud.google.com/iam/docs/reference/credentials/rest/v1/projects.serviceAccounts/generateAccessToken)
defines the token request and lifetime.
For local `--local-issue` use, run the setup script with
`SOLVER_RUNTIME_CALLER=user:YOUR_GOOGLE_LOGIN` to grant that operator the same
token-minting permission. The dispatcher receives its grant without this option.

## Recovery and cleanup

Command cancellation, computer stop, deletion intent, and observed deletion
are separate actions. The dispatcher reports completion after deletion is
observed. If cleanup cannot complete, it keeps the journal and reports progress
so another claim can resume cleanup.

Claims with `recover: true` return the same host's unfinished public jobs,
including ended jobs whose computer still needs deletion. The caller excludes
its active job IDs. A read-only internal lookup by the saved create key recovers
an accepted allocation whose response was lost before its ID reached the job.
That lookup creates nothing and remains inside the trusted service boundary.

The web also checks the public cleanup index every 30 seconds. Cancelled jobs
and jobs whose worker has been quiet for 600 seconds are cleaned up through
the SDK, independently of the dispatcher's disk. Unclaimed cancelled jobs need
no computer operation. Cleanup entries remain until deletion is observed.
Account history pruning preserves records with unfinished cleanup.

## Retained provider calls

Review this inventory on 2026-10-30. Remove the original provider path after
its waiting/running jobs are gone and the operator deliberately migrates or
retires `--local-issue`.

| Original call | Owner and removal condition |
| --- | --- |
| `start_box`: create, poll, and size fallback | Original `wrk` jobs and explicit local runs; remove after those entry points are retired. |
| `sh`, `reachable`: setup and readiness commands | Same captured original resource; remove with `start_box`. |
| `on_box`: credential files, detached execution, command following | Original jobs only; remove with their execution path. |
| `on_box` refresh: direct Vertex-token file write | Original jobs only; remove with `on_box`. |
| `usage`: provider usage read | Original resource only; preserve missing values as unknown until that path is removed. |
| `delete_box` and `run_until`: provider deletion | Original resource returned by that allocation; keep until every original job can finish or be cleaned up. |
| Dispatcher Boat and pool GitHub credentials | Needed by original jobs; remove after the original provider is retired. Public jobs cannot fall back to the pool credential. |

The new provider is
[`solver_box::computers`](../../crates/solver-box/src/computers.rs).
The trusted job admission and cleanup code is
[`work_runs::computers`](../../crates/openagents-web/src/work_runs/computers.rs).
The original provider never discovers or adopts shared computers from labels
or matching names.

## Rollout and rollback

Provision the runtime identity, deploy the lifecycle service's read-only
request lookup, then deploy the web and dispatcher from current `main`.
Use a dedicated account with its own GitHub connection and synthetic
repository. Record the computer in that account's normal list, command and
source checks, restart and cancellation results, and confirmed deletion.

Removing an account from the rollout setting changes its future jobs. Existing
public jobs keep their saved requests and recovery path. Keep a migration-aware
web service and dispatcher available until the public cleanup index is empty
before downgrading either binary. Original jobs retain their original owner
throughout rollout and rollback.

## Verification limits

The tested source passes 119 affected-package tests, 801 web tests, and a
separate web source-copy check. Fifteen web tests remain ignored. Three
previously identified web checks were excluded; their names are in the evidence.
Formatting covers the 21 changed Rust files. Both changed shell scripts pass
syntax checks. The runtime identity was minted through the dispatcher identity
and a local operator; real IAM checks confirm prediction, service usage, and
image download permissions, with image upload and Boat-secret access denied.
No model call was made for those checks.

The approved production test account has no GitHub connection or project. The
Mac's dedicated test credential files did not provide another approved GitHub
session. The full synthetic repository job, list visibility, live worker
restart, cancellation, and deletion check remains unqualified. The fixtures and
IAM checks establish their stated boundaries; they do not replace that run.
The required test account is recorded in [`NEEDS_OWNER.md`](../../NEEDS_OWNER.md).
Keep rollout admission empty until that check passes.
