# 2026-06-27 Pylon/Codex Live Trace Status Audit

## Status

This audit answers the owner question: after a Khala -> Pylon -> Codex
assignment, should there already be trace material visible through the existing
`/trace/{uuid}` route, are tokens counted, and is the live step data saved?

Short answer:

- Yes, a successful Pylon/Codex assignment should produce trace material now.
- The audited assignment did produce trace material and exact token accounting.
- The existing `/trace/{uuid}` page exists, but it is not yet a live assignment
  status view.
- The original audit found that the owner-token read path did not expose these
  Pylon/Codex `owner_only` traces, because trace read-scope resolved the agent
  user id while Pylon/Codex trace ingest stored the linked OpenAuth owner id.
  This change fixes that read-scope mismatch by resolving
  `session.credential.openauthUserId` first and falling back to `session.user.id`.
- Public token count updates at final Codex turn closeout, not continuously for
  streamed raw event chunks.

## Audited Assignment

Assignment:

`assignment.public.khala_coding.chatcmpl_74f54e628d0a4dd7a131a8625afa0d9b`

Durable Khala request:

`chatcmpl_c0d4e9076178417c8441c30343c57358`

Closeout:

`assignment.closeout.0bd4d63a3990087e8cb87c23`

Pylon:

`pylon.33afd48282a649047e3a`

Repository checkout:

`OpenAgentsInc/openagents@4f1d27814580a71d29f33bb79889510bd16bbecf`

The assignment was accepted by local Pylon, ran local Codex, executed the
requested verifier, and closed out accepted. The Pylon closeout reported:

- `6` edited files
- `49` commands
- `1` Codex turn
- verifier passed
- no blocker refs

## What Was Needed To Get Delegation Working

The main checkout at `/Users/christopherdavid/work/openagents` was dirty and
behind `origin/main`, so the run was done from a fresh detached worktree:

`/Users/christopherdavid/work/openagents-worktrees/khala-runbook-seq-20260627-continue`

The worktree was created from current `origin/main` at:

`4f1d27814580a71d29f33bb79889510bd16bbecf`

Fresh worktree dependency installation was required:

```sh
bun install --frozen-lockfile
```

Without that, the local Pylon CLI failed to import workspace dependencies
(`effect`) from the fresh checkout.

Every Pylon command was run with:

```sh
PYLON_DISABLE_DAEMON_ROUTING=1
PYLON_OPENAGENTS_BASE_URL=https://openagents.com
bun apps/pylon/src/index.ts ...
```

`PYLON_DISABLE_DAEMON_ROUTING=1` mattered because there was already a long-lived
`pylon node` process on the machine. Disabling daemon routing ensured the
request used the current fresh-worktree source instead of stale loopback code.

Runbook preflight showed:

- default Codex home: `ready`
- default Claude home: `ready`
- two registry Codex refs: `credentials_missing`
- Pylon lifecycle: `online`
- Pylon ref: `pylon.33afd48282a649047e3a`
- Codex capacity: `available=1`, `busy=0`, `queued=0`, `ready=1`
- heartbeat: registered, linked, not stale, sequence `347`

The successful request used the typed workflow and explicit target Pylon:

```sh
pylon khala request \
  --workflow codex_agent_task \
  --pylon-ref pylon.33afd48282a649047e3a \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit 4f1d27814580a71d29f33bb79889510bd16bbecf \
  --verify "bun run --cwd apps/openagents.com/workers/api test -- src/inference/glm-fleet-readiness.test.ts src/inference/glm-fleet-readiness-routes.test.ts src/inference/glm-fleet-durability-operator.test.ts" \
  --prompt "<public-safe bounded #6311 prompt>"
```

The explicit `--workflow codex_agent_task` and `--pylon-ref` are important. A
plain Khala chat request is allowed to route to normal inference; this path must
create a coding-delegation frame and assignment ref before the local runner is
started.

The assignment was then run by explicit ref:

```sh
pylon assignment run-no-spend \
  --assignment-ref assignment.public.khala_coding.chatcmpl_74f54e628d0a4dd7a131a8625afa0d9b \
  --json
```

The runner appeared quiet for several minutes, but process inspection showed it
was not idle: a local `codex exec` process was running in the materialized Pylon
workspace and executing Vitest. This is a useful operational lesson: current
runner progress still reports `runtime_active` while Codex/verifier subprocesses
are doing real work, so lack of fine-grained progress text is not automatically
a stuck run.

## Continued Delegation Notes From The #6311 Follow-Up

A later #6311 durability/readiness slice used the same runbook path from the
same clean worktree, now at `7712f27ba198dc661e3232c5f562eab7402afb11`, and
successfully delegated to local Codex again:

`assignment.public.khala_coding.chatcmpl_8c5d5c98f2544913aea50facfb81ee30`

That second successful run clarified the operational requirements that matter
when other agents have trouble delegating:

- Use the fresh worktree's `apps/pylon/src/index.ts`, not a stale daemon. Keep
  `PYLON_DISABLE_DAEMON_ROUTING=1` on every command unless intentionally testing
  the daemon.
- Keep `PYLON_OPENAGENTS_BASE_URL=https://openagents.com` set so the local Pylon
  registers against production assignment/control surfaces.
- Run `codex accounts list --json`, `provider go-online --json`, and
  `presence heartbeat --json` immediately before dispatch. This proves the local
  Codex credential is ready, the Pylon ref is online, and the scheduler sees
  available capacity instead of stale presence.
- Pin the assignment to the current clean `origin/main` commit with
  `--commit <sha>`. This prevents a delegated Codex patch from being produced
  against an ambiguous or dirty local checkout.
- Use the typed workflow explicitly:
  `--workflow codex_agent_task --pylon-ref <current-pylon-ref>`. Plain chat does
  not create a local coding assignment.
- Keep the `--prompt` under the Khala request bound. One failed attempt used an
  objective summary longer than the accepted `3-1000` character range; shrinking
  the prompt to a public-safe, bounded task fixed dispatch.
- Use `assignment run-no-spend --assignment-ref <ref> --json` after the Khala
  request returns. The request creates the assignment; the local runner still has
  to claim and execute it.
- Do not treat a quiet terminal as failure. While the runner reports
  `runtime_active`, inspect the local Codex subprocess or wait for closeout; the
  verifier can run for minutes without emitting useful high-level status.
- Always run `khala proof --assignment-ref <ref> --json` after accepted
  closeout. The proof, not the global public counter by itself, is the source of
  truth for assignment-scoped exact tokens, owner-only trace counts, and raw
  event archive metadata.
- Review and integrate the delegated patch manually from the materialized Pylon
  workspace. The local supervisor still owns final code review, verification,
  commit, push, and issue comment.

The #6311 follow-up proof reported:

```json
{
  "provider": "pylon-codex-own-capacity",
  "model": "openagents/pylon-codex",
  "usageTruth": "exact",
  "demandKind": "own_capacity",
  "demandSource": "khala_coding_delegation",
  "inputTokens": 2763599,
  "outputTokens": 16845,
  "reasoningTokens": 3354,
  "cacheReadTokens": 2562304,
  "totalTokens": 2780444
}
```

It also produced `59` owner-only ATIF trace rows and one owner-only raw Codex
archive containing `99` SDK events / `3,399,098` bytes. The reviewed integration
landed as commit `7a3c97a737` and was commented back to issue #6311 without
closing it, because live fleet durability remained degraded.

## Continued Delegation Attempt For This Audit

On the next continuation, delegation setup itself worked, but no extra Codex
slot was actually available.

The work was started from a fresh clean worktree:

`/Users/christopherdavid/work/openagents-worktrees/khala-roadmap-goal-20260627-050244`

The worktree was pinned to `origin/main` at:

`43a069f4ba95751f63c5cfa5fb01f1b4973e6e9e`

Preflight succeeded:

- dependencies were installed with `bun install --frozen-lockfile`;
- `OPENAGENTS_AGENT_TOKEN` was read from the local Khala token file without
  printing it;
- `PYLON_OPENAGENTS_BASE_URL=https://openagents.com` was used;
- `PYLON_DISABLE_DAEMON_ROUTING=1` was used so commands ran against the current
  worktree source instead of a stale local daemon;
- `pylon codex accounts list --json` showed the default Codex account ready;
- `pylon provider go-online --json` returned
  `pylon.33afd48282a649047e3a`;
- `pylon presence heartbeat --json` succeeded and linked the Pylon.

The blocking condition was local capacity, not credentials. Process inspection
showed an unrelated local Pylon/Codex run already active:

`assignment.public.khala_coding.chatcmpl_da19d381e1284676a55e2826aa6f4102`

Because that run was not owned by this audit thread, it was left alone. After
advertising two Codex slots, `provider go-online --json` still reported the
slots as busy (`ready=2`, `busy=2`, `available=0`). A typed dispatch attempt for
the next roadmap slice failed with:

```text
409: The requested linked Pylon is not active, heartbeat-fresh, Codex-capable,
and available.
```

This is the key lesson for other agents: the runbook can be correctly set up and
still produce no assignment if all local Codex slots are busy. In that state
there is no assignment ref, no `assignment run-no-spend`, no closeout, no exact
Pylon/Codex token row, and no owner-only trace for that attempted dispatch.
Agents should not treat that as model inference fallback; it is a capacity
admission failure before delegation begins.

Later, at `2026-06-27T05:15:42Z`, a capacity recheck showed
`capacity.coding.codex.available=1`, `busy=0`, and `ready=1`, but simultaneous
`assignment run-no-spend` processes owned by another parent process appeared at
the same time. This audit did not claim or interrupt those assignments. The
operational rule is still to check capacity immediately before dispatch and to
avoid taking over another agent's active local runs.

The reliable sequence remains:

1. Use a clean worktree from current `origin/main`.
2. Install dependencies in that worktree.
3. Run the Pylon CLI from that worktree with daemon routing disabled.
4. Preflight Codex account readiness, Pylon online state, and fresh heartbeat.
5. Confirm `capacity.coding.codex.available > 0`.
6. Create a typed `codex_agent_task` with explicit `--pylon-ref`, repo, branch,
   commit, prompt, and verifier.
7. Run the returned assignment ref with `assignment run-no-spend --json`.
8. Verify the closeout with assignment-scoped proof, not only the global counter.

## Earlier Follow-Up Delegation Failure Mode

The same runbook was attempted again for the #6318 scheduler follow-up after the
local Pylon was brought online from the clean worktree:

- `codex accounts list --json` showed the default local Codex account ready.
- `provider go-online --json` returned Pylon ref `pylon.33afd48282a649047e3a`
  with `capacity.coding.codex.available=1`.
- `presence heartbeat --json` returned a fresh registered heartbeat.
- Two client-side request-shape mistakes were caught before dispatch:
  an abbreviated commit SHA and an unsupported verifier shape.

After correcting those, the typed `khala request --workflow codex_agent_task`
failed repeatedly with HTTP `503`:

```text
pylon khala request failed (503): The Khala coding dispatch gate could not read
linked Pylon capacity right now. This is a transient gate failure, not an
account problem -- retry shortly.
```

This is distinct from the earlier runbook success. The local executor and Codex
credential were ready; the blocking condition was the production dispatch gate's
capacity-read path, before an assignment ref was created. Because no assignment
was created, there was no `assignment run-no-spend`, no Codex turn closeout, no
Pylon/Codex exact token row, and no owner-only Pylon/Codex trace for this failed
attempt.

Operationally, this is the condition other agents should recognize:

- keep the clean-worktree, daemon-disabled Pylon setup;
- rerun heartbeat immediately before dispatch;
- if the gate still returns this `503`, do not invent a model fallback or count
  it as a Pylon/Codex run;
- continue local supervised implementation only when the user has asked not to
  stall, and record the gate failure in the audit/issue comment.

## Token Accounting

The assignment has one exact downstream Codex token row:

```json
{
  "provider": "pylon-codex-own-capacity",
  "model": "openagents/pylon-codex",
  "usageTruth": "exact",
  "demandKind": "own_capacity",
  "demandSource": "khala_coding_delegation",
  "inputTokens": 3645285,
  "outputTokens": 18679,
  "reasoningTokens": 3653,
  "cacheReadTokens": 3475968,
  "totalTokens": 3663964
}
```

The production D1 row in `token_usage_events` matched those values.

The public counter moved from the pre-run baseline observed during preflight:

`336,942,491`

to the post-closeout value:

`340,610,457`

The exact row for this assignment accounts for `3,663,964` of that movement.
The small difference is expected aggregate noise from concurrent activity
because the public counter is global, not assignment-scoped.

The 30-day model mix endpoint also included the assignment in the public
projection:

```json
{
  "totalTokens": 340610457,
  "pylonCodex": {
    "family": "pylon_codex",
    "label": "Pylon-Codex",
    "tokens": 242391883,
    "reqs": 132,
    "pct": 71.163958
  }
}
```

## Does The Counter Update Continuously?

No. For Pylon/Codex assignments, the exact public token counter updates when a
completed Codex turn is posted to:

`POST /api/pylon/codex/turns`

That route computes exact counts from the Codex SDK `turn.completed` usage and
inserts one `token_usage_events` row. It then publishes the public counter delta
only when the token row was inserted.

So the visible counter bump happens at the end of the Codex turn/closeout path,
when exact SDK usage has been received and accepted. The counter does not
increment once per streamed event chunk and does not move just because the local
runner is active.

The later #6318 live stress/external probe showed the same shape for normal
gateway chat traffic: the public counter moved after the metered requests
closed and exact usage rows existed. It did not provide a visible per-second or
per-chunk in-flight increment while the stress stream was still active.

Streaming raw SDK event chunks are posted during the run to:

`POST /api/pylon/codex/event-chunks`

Those chunks save observability evidence, but they do not create token usage
rows. This is correct for exact accounting: chunks do not carry final exact SDK
usage, so they should not move the exact public token counter.

If the product wants a visibly moving counter during long Pylon/Codex runs, that
needs to be a separate labeled estimate based on raw chunks or progress events.
It must not be mixed into the exact served-token counter until final usage
arrives.

For the homepage/stats live counter, the publish path is:

1. a served completion or Pylon/Codex turn inserts a canonical
   `token_usage_events` row;
2. the recorder publishes a public-safe sync delta only if the insert actually
   happened;
3. that delta carries the per-event token increment and the authoritative ledger
   running total after the insert;
4. the client advances monotonically from the latest total.

So when the token count "finally" updates after a quiet wait, that means the
final exact usage row landed and the public sync/scalar projection saw it. It
does not mean the counter was incrementing throughout the in-flight Codex run.

## Trace And Raw Event Persistence

The first-class proof command reports:

```json
{
  "traces": {
    "count": 80,
    "visibility": "owner_only",
    "schemaVersion": "ATIF-v1.7"
  },
  "rawEvents": {
    "count": 1,
    "eventCount": 139,
    "byteLength": 1305687,
    "visibility": "owner_only"
  }
}
```

Remote D1 read-only verification confirmed:

| Store | Rows | Events | Bytes |
| --- | ---: | ---: | ---: |
| `agent_traces` | 80 | n/a | n/a |
| `pylon_codex_raw_event_chunks` | 79 | 139 | 1,350,671 |
| `pylon_codex_raw_events` | 1 | 139 | 1,305,687 |
| `token_usage_events` | 1 | n/a | n/a |

The `agent_traces` rows split as:

- `79` chunk traces
- `1` final turn trace

This confirms that data from the live Codex SDK event stream was saved during
the run, and the final whole-turn raw event archive was saved at closeout.

The storage model is intentionally two-tier:

- `agent_traces`: owner-only, redacted, public-safe ATIF-v1.7 projections.
- `pylon_codex_raw_event_chunks` / `pylon_codex_raw_events`: owner-only private
  raw SDK evidence in R2 with D1 metadata. These may contain prompts, command
  args, local paths, file details, and shell output, and must never be surfaced
  through public trace pages, public counters, issue comments, or product
  promise output.

## Existing `/trace/{uuid}` Route

The app already has a `/trace/{uuid}` route. It is a stateless shell route that
loads the trace via:

`GET /api/traces/{uuid}`

The front-end `LoadTrace` command:

- fetches once on initial route load;
- forwards `?token=<oa_agent_...>` to the read API when present;
- uses `cache: "no-store"`;
- decodes the returned public-safe ATIF trajectory;
- renders loaded, not-found, or failed state.

There is no current polling loop, EventSource, WebSocket, or durable stream
subscription in the `/trace/{uuid}` page. A viewer does not live-update while
new Pylon/Codex chunks arrive. At best, a manual refresh can fetch a trace row
that already exists.

The route also expects one trace UUID. A Pylon/Codex assignment currently
produces many trace UUIDs: one per chunk and one final turn trace. There is no
single assignment-session trace UUID or manifest page that composes all 80 rows
into one coherent live assignment view.

## Can We View This Assignment Through `/trace/{uuid}` Right Now?

The trace rows existed, but the owner-token read path did not expose them during
the original audit.

One trace UUID sampled from proof:

`395c0e52-e0de-410d-9844-5f4a5404ad02`

Observed API behavior:

- anonymous `GET /api/traces/{uuid}`: `404` as expected for `owner_only`;
- `GET /api/traces/{uuid}` with the shell `OPENAGENTS_AGENT_TOKEN`: `404`;
- `GET /api/traces/{uuid}` with the stored Pylon token:
  `/Users/christopherdavid/.openagents/pylon/auth/openagents-agent-token`: `404`;
- `GET /api/traces?demand_kind=own_capacity` with the stored Pylon token:
  `200`, but zero traces.

The likely root cause is an owner-id mismatch between ingest and trace read
scope:

- Pylon/Codex turn ingest uses `ownerUserIdForAgent(session)`, which stores the
  linked OpenAuth owner id when present.
- Assignment proof reports the owner as:
  `openauthUserRef: github:14167547`.
- Trace read-scope token resolution currently returns `session.user.id`.
- For linked Pylon credentials, that can be the agent user id rather than the
  linked OpenAuth owner id.

So the owner-only trace rows are stored under the linked OpenAuth owner, while
the token read/list path can look under the agent user. That produces a privacy-
preserving 404 even for the local owner token.

The read-scope resolver now returns:

```ts
session.credential.openauthUserId ?? session.user.id
```

matching Pylon/Codex ingest. This preserves owner-only isolation while letting
the local Khala/Pylon token open the owner's own Pylon/Codex traces.

## Live Status Gap

The current system records the necessary pieces, but it does not yet present the
"full status of the session" live on `/trace/{uuid}`.

What exists:

- durable Khala stream for the initial delegation frame;
- Pylon assignment lifecycle events in the local runner;
- streamed raw SDK event chunk ingest;
- owner-only chunk ATIF traces;
- final exact token row;
- final owner-only ATIF trace;
- final private raw event archive;
- proof endpoint summarizing exact token/traces/raw final archive.

What is missing for the desired live page:

- one stable assignment/session view URL created at assignment creation time;
- a route/API that resolves assignment ref or durable request id to all related
  trace UUIDs, raw chunk metadata, final raw archive metadata, token row status,
  verifier status, closeout status, and public-safe progress;
- front-end polling or streaming for that assignment/session view;
- owner-token read-scope fix for linked OpenAuth-owned traces;
- proof endpoint expansion to include raw chunk counts and chunk refs, not just
  final raw event archive refs;
- a public-safe rollup renderer that composes many chunk/final traces into one
  coherent timeline without exposing raw SDK payloads.

## Recommended Next Build

1. Keep trace read-scope ownership fixed:
   `resolveReadScopeOwner` now prefers `session.credential.openauthUserId` when
   present, falling back to `session.user.id`.

2. Add assignment-level trace proof/read API:
   `GET /api/pylon/codex/proof` should include raw chunk aggregate counts and
   optionally bounded chunk refs.

3. Add a stable assignment/session status endpoint:
   `GET /api/pylon/codex/assignments/{assignmentRef}/trace-status` or equivalent
   owner-scoped endpoint returning:
   - assignment ref;
   - durable request id when known;
   - current lifecycle / closeout state;
   - token row status and totals;
   - chunk count/event count/byte count;
   - final raw archive status;
   - trace row count and final trace UUID;
   - public-safe recent progress.

4. Update `/trace/{uuid}` or introduce a trace manifest variant so the existing
   route can render a whole assignment, not only one immutable ATIF row.

5. Add live refresh:
   polling is enough for the first version. The page can poll until closeout is
   accepted or rejected. Streaming can come later.

6. Keep raw SDK events private:
   `/trace/{uuid}` should show redacted ATIF summaries and private-safe metadata
   only. Raw R2 event payloads remain owner-only audit material and should not be
   downloaded or rendered by the public trace page.

## Bottom Line

This assignment proves the backend evidence path mostly works:

- exact tokens are counted in the public ledger;
- streamed raw chunks are saved;
- final raw events are saved;
- redacted ATIF traces are saved;
- final proof can reconcile the assignment to exact usage.

The product gap is presentation and owner read scope:

- the existing `/trace/{uuid}` page is a single-fetch trace renderer, not a live
  assignment session monitor;
- the local owner-token mismatch for Pylon/Codex owner-only traces is fixed in
  this change, but live production should be re-smoked with a fresh linked trace
  URL after deploy;
- there is no single stable trace URL that represents the whole assignment.
