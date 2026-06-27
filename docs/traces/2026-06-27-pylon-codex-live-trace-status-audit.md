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
- The latest successful continuation runs kept working only when the local agent
  used the runbook literally: local Khala token in the environment, fresh
  worktree, daemon routing disabled, fresh heartbeat, explicit Pylon ref,
  explicit `codex_agent_task`, exact commit pin, local `run-no-spend`, and
  assignment-scoped proof after closeout.
- After the production dispatch-gate fix was deployed, a fresh post-deploy
  smoke accepted, ran local Codex, passed verification, stored owner-only traces,
  and inserted an exact token row. That confirms the same runbook is currently
  working when the linked Pylon has an available Codex slot.
- A `2026-06-27T07:08Z` continuation smoke confirmed that the sampled
  owner-only Pylon/Codex trace is now readable through
  `/api/traces/{uuid}?token=...` and listable through
  `/api/traces?demand_kind=own_capacity&token=...`.
- The same smoke found that `GET /api/pylon/codex/trace-status` still returned
  production `404` for the sampled assignment, even though the endpoint exists
  in local code/tests. Treat that as a deploy/route parity gap until a deployed
  smoke returns the metadata payload.
- The trace read token and proof token are not interchangeable in every case:
  a Khala/local token linked to the same OpenAuth owner can read owner-only
  traces, while the Pylon/Codex proof route requires the exact assignment-owning
  Pylon agent credential.

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

## Latest Successful #6323 Delegation After Capacity Returned

After the trace read-scope fix landed, the same runbook sequence worked again
for the next #6323 GLM NVFP4 pilot slice from the clean worktree at
`54951e811bc5bbdd02e21c8e7f98b011656d03a1`.

Fresh preflight showed:

- Pylon ref `pylon.33afd48282a649047e3a`;
- heartbeat sequence `410`;
- linked, non-stale presence;
- `capacity.coding.codex.available=1`;
- daemon routing disabled so the current worktree source was used.

The typed request created:

`assignment.public.khala_coding.chatcmpl_44a4a0608a1049d59b41fae44b840433`

with durable request:

`chatcmpl_6e32740eb2594360bc15536853d9e221`

`assignment run-no-spend --json` completed accepted with closeout:

`assignment.closeout.02d6bb1b7c18e8ad84972294`

The accepted patch added a public boot/load evidence reference to the #6323
isolated-owner gate. That means the owner-armed NVFP4 pilot cannot pass on
endpoint URL, endpoint ref, owner approval, and decision ref alone; it now also
requires a public-safe `KHALA_GLM_NVFP4_BOOT_LOAD_EVIDENCE_REF` proving the
isolated 8x host actually booted and loaded the full-model endpoint.

The exact proof for this latest delegation reported:

```json
{
  "provider": "pylon-codex-own-capacity",
  "model": "openagents/pylon-codex",
  "usageTruth": "exact",
  "demandKind": "own_capacity",
  "demandSource": "khala_coding_delegation",
  "inputTokens": 2729097,
  "outputTokens": 9508,
  "reasoningTokens": 438,
  "cacheReadTokens": 2556288,
  "totalTokens": 2738605
}
```

It also reported `67` owner-only ATIF traces and one owner-only raw Codex event
archive containing `109` SDK events / `2,623,114` bytes.

This is the concrete path that got delegation working for the latest slice:
clean current worktree, local dependencies installed, daemon routing disabled,
fresh heartbeat immediately before dispatch, explicit `codex_agent_task`
workflow, explicit Pylon ref, exact commit pin, local `run-no-spend`, then
assignment-scoped proof.

## Latest Successful #6311 And #6320 Continuation Delegations

After the #6323 proof, two more continuation slices succeeded from the current
fresh worktree:

`/Users/christopherdavid/work/openagents-worktrees/khala-roadmap-goal-20260627-050244`

The working command environment stayed the same:

```sh
OPENAGENTS_AGENT_TOKEN="$(cat /Users/christopherdavid/.config/khala/agent-token)"
PYLON_OPENAGENTS_BASE_URL=https://openagents.com
PYLON_DISABLE_DAEMON_ROUTING=1
bun apps/pylon/src/index.ts ...
```

The crucial detail is that the token was provided to the local Pylon CLI and the
daemon was bypassed. Other agents that only invoke generic Khala chat, omit the
explicit Pylon ref, use stale daemon routing, dispatch from a dirty checkout, or
skip `assignment run-no-spend` will not create a real local Codex assignment and
therefore will not produce exact Pylon/Codex token rows.

The #6311 recovered-capacity/durability-blocker slice used:

- assignment:
  `assignment.public.khala_coding.chatcmpl_5abf9492392c45bfa04d0620b1dd0949`
- closeout: `assignment.closeout.449ee26126ab3e3137eff427`
- commit integrated to `main`: `17a8c70e68036c2950249faf3027e0cd0d2075d6`
- exact own-capacity proof: `3,568,906` total tokens
- owner-only trace proof: `69` ATIF traces
- private raw Codex archive: `112` SDK events / `1,750,286` bytes

That patch clarified the live #6311 shape: the public GLM fleet readout is still
`status:"degraded"` and durability acceptance remains blocked, but material
serving capacity has recovered (`readyReplicaCount:8`,
`reclaimedReplicaCount:0`, `warmOrReadyMaxInflight:9`). The issue stayed open
because the remaining durability acceptance blockers are owner/infra evidence,
not missing local code.

The #6320 throughput-readout dependency slice used:

- assignment:
  `assignment.public.khala_coding.chatcmpl_071d621d4cd94788875d83020a7bc5b9`
- durable request: `chatcmpl_110c3d7165af4793896ad00d0f372b33`
- closeout: `assignment.closeout.f44607a9a5409bc439ab2778`
- commit integrated to `main`: `40e08df70b94caab8e198971902c348fa7ab0b59`
- exact own-capacity proof: `2,813,295` total tokens
- owner-only trace proof: `56` ATIF traces
- private raw Codex archive: `92` SDK events / `2,387,478` bytes

That patch made #6320 consume #6311's degraded-but-recovered capacity shape.
Throughput rollout, #6317 stress, and #6312 benchmark now stay blocked in typed
readouts when serving is degraded or durability acceptance is incomplete, while
still showing the operator the recovered serving-capacity summary and remaining
blocker refs.

These two latest runs are the current best reproduction of what was required to
get delegation working reliably:

1. Read the local Khala/OpenAgents agent token into `OPENAGENTS_AGENT_TOKEN`.
2. Run Pylon from the clean current worktree, not from a stale daemon.
3. Keep `PYLON_DISABLE_DAEMON_ROUTING=1` and
   `PYLON_OPENAGENTS_BASE_URL=https://openagents.com` set for every command.
4. Run `provider go-online --json` and `presence heartbeat --json` immediately
   before dispatch and confirm linked, non-stale Codex capacity.
5. Dispatch with `khala request --workflow codex_agent_task --pylon-ref ...`
   plus explicit repo, branch, exact commit, bounded prompt, and verifier.
6. Execute the returned assignment with
   `assignment run-no-spend --assignment-ref ... --json`.
7. Inspect and integrate the materialized patch manually; the supervising agent
   still owns review, tests, commit, push, and issue comments.
8. Run `khala proof --assignment-ref ... --json`; use that proof as the
   assignment-scoped source of truth for exact tokens, trace counts, and raw
   archive counts.

Token counter behavior in these runs matched the earlier audit: the public
counter did not rise while Codex was still streaming raw SDK chunks. It moved
only after the local Codex turn closed out and the server accepted the exact
`token_usage_events` row.

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

The local follow-up fixed a narrower production-gate bug that made one version
of this path opaque. The coding-delegation branch now wraps owner-scope
resolution, linked-agent capacity reads, and assignment delegation in a plain
async `try/catch`. Generic storage/capacity-read failures return the typed
`coding_delegation_store_unavailable` 503 instead of escaping as an opaque 500.

## Post-Deploy Dispatch-Gate Re-Smoke

After that fix was committed as `ca2b2e30919744a053e0832047013d3a2a61d171`
and deployed as Worker `2accb9cf-01de-4701-ac0e-00af67380217`, the same
runbook succeeded from this worktree.

Preflight:

- `codex accounts list --json`: default local Codex ready; two extra registered
  Codex homes still `credentials_missing`;
- `provider go-online --json`: Pylon `pylon.33afd48282a649047e3a`, Codex
  `ready=1`, `busy=0`, `available=1`;
- `presence heartbeat --json`: registered, linked, non-stale heartbeat sequence
  `448`.

Dispatch:

- durable request: `chatcmpl_0377a2089b5d40e0aec5efc04ad296a0`
- assignment:
  `assignment.public.khala_coding.chatcmpl_4ee7c89308d345ff8a40ad96e174c9bd`
- closeout: `assignment.closeout.f50d9d54997fc2c0ebec9dd3`
- verifier:
  `bun run --cwd apps/openagents.com/workers/api test -- src/inference/benchmark/stress-saturation-plan.test.ts src/inference/chat-completions-routes.test.ts`

The runner reported:

- status `accepted`;
- `0` file edits;
- `19` commands;
- `1` Codex turn;
- verification passed;
- no blocker refs.

Proof:

```json
{
  "provider": "pylon-codex-own-capacity",
  "model": "openagents/pylon-codex",
  "usageTruth": "exact",
  "demandKind": "own_capacity",
  "demandSource": "khala_coding_delegation",
  "inputTokens": 711274,
  "outputTokens": 4310,
  "reasoningTokens": 336,
  "cacheReadTokens": 665728,
  "totalTokens": 715584
}
```

The same proof reported `32` owner-only ATIF traces and one private raw Codex
archive containing `51` SDK events / `163,196` bytes. A live public scalar read
after closeout returned `tokensServed: 428,419,989` with
`composition: "live_at_read"` over `token_usage_events`.

This is the clearest current recipe for other agents: delegation is not just
"call Khala." It requires a linked, heartbeat-fresh Pylon with available Codex
capacity, explicit coding workflow dispatch, local no-spend execution, and proof
after closeout. The token counter movement appears only after that closeout path
posts exact usage.

The very next real #6318 hardening dispatch exposed the remaining intermittent
gate behavior. Preflight again showed Pylon `pylon.33afd48282a649047e3a` with
Codex `ready=1`, `busy=0`, `available=1`, and fresh heartbeat sequences `455`
and `456`, but `khala request --workflow codex_agent_task` returned the same
typed `503` twice:

```text
The Khala coding dispatch gate could not read linked Pylon capacity right now.
This is a transient gate failure, not an account problem -- retry shortly.
```

No assignment ref was created for that real task, so there is no corresponding
Pylon/Codex closeout, exact token row, or owner-only trace. The operational
conclusion is precise: the runbook can work end-to-end, but the production
capacity-read gate still has an intermittent admission race. In an owner-directed
"do not stall" session, the supervising agent may continue locally after
recording that the attempted delegation did not create an assignment.

After the #6318 router hardening landed in `12ae92954633c80cac14c541cf63fbcb71a5764b`
and deployed as Worker `e7cb0683-58f4-48ac-836e-8bca3082d0ab`, a final
post-deploy smoke used the same runbook successfully again:

- durable request: `chatcmpl_558cd3edabb1425aa19b315d049a9008`
- assignment:
  `assignment.public.khala_coding.chatcmpl_b5ccba76058f48d58b903948cd396672`
- closeout: `assignment.closeout.af82664bae77efc3991e62e2`
- runner result: `0` edits, `17` commands, `1` Codex turn, verifier passed
- exact own-capacity proof: `514,462` total tokens
- owner-only trace proof: `28` ATIF traces
- private raw Codex archive: `45` SDK events / `755,150` bytes

So the latest observed state is not "delegation never works." It works when the
gate admits the request, stores exact usage at closeout, and records private
trace/raw-event evidence; it also still has an intermittent pre-assignment
capacity-read failure that agents must identify honestly.

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

Put differently: the scalar endpoint is "live at read" over the canonical
ledger, but the ledger only receives a Pylon/Codex row after a Codex turn has
final SDK usage. In the browser, the live room pushes a delta after that row is
inserted and then the DOM count-up animates to the new authoritative running
total. For a one-turn Pylon/Codex assignment that looks like "at the end"; for
multi-turn or many concurrent completions it moves once per committed usage row,
not once per second and not once per streamed chunk.

Current live scalar smoke:

```json
{
  "tokensServed": 435203075,
  "generatedAt": "2026-06-27T07:08:52.272Z",
  "staleness": {
    "composition": "live_at_read",
    "maxStalenessSeconds": 0,
    "rebuildsOn": ["token_usage_events"]
  }
}
```

## 2026-06-27T07:08Z Continuation Smoke

The latest continuation smoke used the #6323 delegated audit assignment:

`assignment.public.khala_coding.chatcmpl_1702ef0731874c72934c1f95068f8c47`

The exact assignment proof returned:

```json
{
  "provider": "pylon-codex-own-capacity",
  "model": "openagents/pylon-codex",
  "usageTruth": "exact",
  "demandKind": "own_capacity",
  "demandSource": "khala_coding_delegation",
  "inputTokens": 806377,
  "outputTokens": 4266,
  "reasoningTokens": 250,
  "cacheReadTokens": 707328,
  "totalTokens": 810643
}
```

The same proof reported:

- `32` owner-only ATIF-v1.7 trace rows;
- one owner-only raw Codex archive;
- `52` raw SDK events;
- `1,252,108` raw-event bytes.

The concrete credential behavior mattered:

- `bun apps/pylon/src/index.ts khala proof --assignment-ref ...` succeeded
  because the Pylon CLI used the assignment-owning Pylon agent credential.
- Direct `GET /api/pylon/codex/proof?...` with the assignment-owning Pylon agent
  bearer returned `200`.
- Direct `GET /api/pylon/codex/proof?...` with a different local Khala token
  linked to the same OpenAuth owner returned `403` with
  `pylon_codex_forbidden`, because the proof route gates on the exact
  assignment-owning agent.
- `GET /api/traces/68144063-d014-4bf6-879b-f582a67cc22a?token=...` returned
  `200` for the sampled owner-only final trace.
- `GET /api/traces?demand_kind=own_capacity&token=...` returned `200` and
  listed the sampled #6323 trace rows.
- `GET /api/pylon/codex/trace-status?assignmentRef=...` returned production
  `404` even with the assignment-owning Pylon bearer.

So the evidence path is real and currently queryable through proof plus
individual/listed trace reads. The assignment-level trace-status route still
needs a deployed-route smoke before it can be treated as live product surface.

## Latest Trace-Status Delegation Run

For this trace-status endpoint slice, the runbook worked again from:

`/Users/christopherdavid/work/openagents-worktrees/khala-roadmap-goal-20260627-050244`

The exact preflight/setup that made it work was:

- clean worktree from `origin/main`;
- local dependencies installed with `bun install --frozen-lockfile`;
- local Khala/OpenAgents token exported as `OPENAGENTS_AGENT_TOKEN`;
- `PYLON_OPENAGENTS_BASE_URL=https://openagents.com`;
- `PYLON_DISABLE_DAEMON_ROUTING=1`;
- current Pylon brought online and heartbeated immediately before dispatch;
- explicit target Pylon `pylon.33afd48282a649047e3a`;
- explicit `--workflow codex_agent_task`;
- exact repo, branch, and commit pin:
  `OpenAgentsInc/openagents`, `main`,
  `c540608c053b03dfc87c7765d0a8d3cf40df7fc0`;
- local runner invoked with `assignment run-no-spend --assignment-ref ...`;
- proof read after accepted closeout.

The delegated request was:

- durable request: `chatcmpl_5959923d80f8450f967b289b6fb0f775`;
- assignment:
  `assignment.public.khala_coding.chatcmpl_8010cf9064ce400fbd74f4e08969ffa8`;
- closeout: `assignment.closeout.6f5a49601f06edefe1ed725b`;
- runner result: `10` edits, `51` commands, `1` Codex turn, verifier passed.

The exact proof was:

```json
{
  "provider": "pylon-codex-own-capacity",
  "model": "openagents/pylon-codex",
  "usageTruth": "exact",
  "demandKind": "own_capacity",
  "demandSource": "khala_coding_delegation",
  "inputTokens": 5222264,
  "outputTokens": 20705,
  "reasoningTokens": 1034,
  "cacheReadTokens": 5014016,
  "totalTokens": 5242969
}
```

The same proof reported `96` owner-only ATIF traces and one private raw Codex
archive containing `157` SDK events / `3,558,909` bytes.

This run is the most recent concrete answer to why other agents have had
trouble delegating: the mechanism works, but only when the task is sent through
the typed coding-delegation path and then actually claimed by the local runner.
Generic Khala chat, a missing local token, stale daemon routing, a stale Pylon
heartbeat, a missing `--pylon-ref`, a missing `--workflow codex_agent_task`, or
skipping `assignment run-no-spend` will look like "Khala was used" while never
creating a Pylon/Codex assignment, token row, or trace bundle.

## What Actually Made Delegation Work

The successful runs were not magic "ask Khala to code" turns. They were a very
specific local-supervisor flow:

1. Start from a clean worktree pinned to current `origin/main`.
2. Install that worktree's dependencies with `bun install --frozen-lockfile`.
3. Read the local Khala/OpenAgents agent token into
   `OPENAGENTS_AGENT_TOKEN` without printing it.
4. Run the Pylon CLI from that worktree's source:
   `bun apps/pylon/src/index.ts ...`.
5. Set `PYLON_OPENAGENTS_BASE_URL=https://openagents.com`.
6. Set `PYLON_DISABLE_DAEMON_ROUTING=1` so a stale long-lived local daemon
   cannot intercept the command.
7. Immediately before dispatch, run:
   - `codex accounts list --json`
   - `provider go-online --json`
   - `presence heartbeat --json`
8. Confirm the returned Pylon is linked, heartbeat-fresh, and has at least one
   available Codex slot.
9. Dispatch with the typed coding workflow, not generic chat:
   `khala request --workflow codex_agent_task --pylon-ref <pylon-ref> ...`.
10. Pin repo, branch, and exact commit in the request.
11. Keep the prompt bounded and public-safe; prompts over the accepted request
    length were rejected before assignment creation.
12. Run the returned assignment locally with
    `assignment run-no-spend --assignment-ref <assignment-ref> --json`.
13. Wait for accepted closeout, inspect the materialized patch, and run local
    verification before committing.
14. Read assignment-scoped truth with
    `khala proof --assignment-ref <assignment-ref> --json`.

That exact flow is what produced accepted Pylon/Codex assignments, owner-only
ATIF traces, private raw Codex archives, and exact `token_usage_events` rows.

The most common false-positive states were:

- **Generic Khala chat**: can answer normally but does not create a local Codex
  assignment.
- **No local token in `OPENAGENTS_AGENT_TOKEN`**: the CLI may run, but it cannot
  prove the owner/agent scope needed by production assignment surfaces.
- **Stale daemon routing**: commands may hit old local code and produce behavior
  that does not match the current worktree.
- **Fresh heartbeat but no available Codex slot**: dispatch correctly returns
  admission failure before assignment creation.
- **Assignment created but `run-no-spend` skipped**: nothing local claims and
  executes the work, so there is no Codex closeout.
- **Waiting for the public counter instead of proof**: the global counter is
  aggregate and can move because of unrelated activity; proof is the
  assignment-scoped source of truth.

The most recent post-audit #6311 zero-edit delegation used the same flow:

- assignment:
  `assignment.public.khala_coding.chatcmpl_50871eacbf1647439bc273f7b4f118f3`
- closeout: `assignment.closeout.025cc2fde96f601dc661f1a0`
- result: accepted, `0` edits, `26` commands, `1` Codex turn, verifier passed
- exact usage: `917,278` total tokens (`913,476` input, `3,802` output,
  `100` reasoning, `799,488` cache read)
- trace proof: `38` owner-only ATIF traces
- raw-event proof: `64` private SDK events / `795,253` bytes

This confirmed the important operational point: a "no code change needed" run
still creates trace/proof/accounting evidence when it actually goes through
Pylon/Codex. Conversely, a failed pre-assignment gate, a plain chat response, or
an unclaimed assignment will not and should not move the assignment-scoped proof
surface.

## Token Counter Timing Clarification

The token counter finally updates when canonical usage is inserted, not while a
Codex process is merely active.

For Pylon/Codex runs, streamed SDK chunks are saved during execution as
owner-only observability evidence. Those chunks are not exact usage records and
do not update the public served-token counter. The public ledger updates only
after Codex emits final SDK usage for a turn, the Pylon/Codex closeout path posts
that completed turn, and the server inserts the exact `token_usage_events` row.

So the behavior observed in the browser is expected for exact accounting:

- while Codex is thinking, running commands, and streaming raw events, trace
  chunks may be accumulating privately, but the exact public token count can
  stay flat;
- when the turn closes and exact usage lands, the public scalar is immediately
  live-at-read over the ledger and the website's live counter can animate to the
  new authoritative total;
- if several assignments or chats finish close together, the global counter can
  jump by aggregate ledger movement, which is why assignment-scoped proof is the
  only reliable way to audit one run.

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

## First Assignment Status Endpoint

The first backend slice of that manifest gap now exists as:

`GET /api/pylon/codex/trace-status?assignmentRef=<assignmentRef>`

Important production-smoke correction: as of `2026-06-27T07:10Z`, the route
returned `404 {"error":"not_found"}` on `openagents.com` for the latest sampled
#6323 assignment, even when called with the assignment-owning Pylon bearer. The
code and focused tests exist locally, but the deployed route was not reachable
in that smoke. This should be treated as a route/deploy parity blocker for the
live assignment-status page until a production call returns the metadata-only
payload described below.

It is owner-scoped with the same local OpenAgents agent bearer token used by the
Pylon/Codex ingest and proof routes. It resolves the assignment through the
caller-owned Pylon assignment store before reading any rows, so another agent's
assignment returns `403` and never reaches trace/token/raw-event aggregation.

The status payload is intentionally metadata-only and public-safe:

- assignment ref and Pylon ref;
- owner refs (`agent:<agentUserId>` and linked OpenAuth owner ref);
- assignment lifecycle state plus public-safe closeout/proof/artifact refs;
- public-safe assignment event count, progress event count, latest event kind,
  latest status, and latest event timestamp;
- exact token row status and totals when a final row exists;
- owner-only ATIF trace count, latest trace UUID, and final-turn trace UUID when
  available;
- streamed raw chunk counts/event counts/bytes and latest chunk ref;
- final raw archive counts/event counts/bytes and latest raw archive ref;
- derived progress state:
  `assignment_created`, `streaming_chunks`, `final_trace_recorded`,
  `tokens_recorded`, `closed_out`, or `rejected`.

The endpoint does **not** return raw SDK payloads, R2 keys, command text, shell
output, prompts, local paths, provider credentials, or trace JSON. Raw SDK
payloads remain private owner-only audit evidence in R2/D1 metadata and are not
rendered by this status surface.

Focused coverage in `pylon-codex-turn-ingest-routes.test.ts` proves:

- a live chunk-only assignment reports `progress.state: "streaming_chunks"` with
  token usage still `pending`;
- after the final token row, final trace, final raw archive, and closeout refs
  exist, the same store reports `progress.state: "closed_out"` and exact token
  totals;
- another agent owner cannot read the status;
- the JSON payload does not expose raw payload/R2 key/prompt/shell/secret
  material.

## Can We View This Assignment Through `/trace/{uuid}` Right Now?

Yes for individual trace rows, after the owner read-scope fix. No for a whole
assignment session yet.

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

The `2026-06-27T07:09Z` production re-smoke confirmed the fix on the latest
#6323 assignment. A sampled trace UUID:

`68144063-d014-4bf6-879b-f582a67cc22a`

returned:

- `200` from `GET /api/traces/{uuid}?token=...`;
- `200` from `GET /api/traces?demand_kind=own_capacity&token=...`, with the
  sampled rows listed;
- owner-only metadata, `agentRef`, `trajectoryId`, step count, demand kind, and
  demand source, without raw SDK payload exposure.

That proves individual Pylon/Codex trace rows are viewable by the owner token
right now. It does not yet provide one stable "full session" URL or live
assignment timeline.

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
- frontend integration that resolves assignment ref or durable request id to the
  assignment trace-status API and renders it in a coherent timeline;
- deployed-route parity for the assignment trace-status endpoint; the local
  code/tests exist, but the latest production smoke returned `404`;
- front-end polling or streaming for that assignment/session view;
- proof/status endpoint expansion to include verifier-specific progress and
  bounded recent activity labels, beyond the current event/trace/chunk/token/
  archive counts;
- a public-safe rollup renderer that composes many chunk/final traces into one
  coherent timeline without exposing raw SDK payloads.

## Recommended Next Build

1. Keep trace read-scope ownership fixed:
   `resolveReadScopeOwner` now prefers `session.credential.openauthUserId` when
   present, falling back to `session.user.id`.

2. Build the assignment trace-status page:
   first get a production `200` smoke from
   `GET /api/pylon/codex/trace-status?assignmentRef=...`, then connect it to an
   owner-token web view. The local endpoint code/tests exist, but the latest
   deployed smoke returned `404`.

3. Add durable request/session lookup:
   the current status endpoint keys by assignment ref. The UI still needs a
   stable creation-time link from durable request id or `/trace/{uuid}` manifest
   id to assignment ref.

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
- the local owner-token mismatch for Pylon/Codex owner-only traces is fixed and
  live-smoked with the sampled #6323 trace;
- the assignment-level trace-status endpoint is present locally but still
  returned production `404` in the latest smoke;
- there is no single stable trace URL that represents the whole assignment.
