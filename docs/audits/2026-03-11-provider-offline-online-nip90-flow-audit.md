# Provider Offline/Online NIP-90 Flow Audit

Date: 2026-03-11
Branch audited: `main`
Audit type: static repo audit against observed runtime logs

## Question Audited

What exactly is happening when the desktop logs show a mix of:

- provider ingress while "offline",
- Apple FM execution succeeding locally,
- later `Cannot publish NIP-90 feedback/result while provider lane is offline`,
- and finally `job execution timed out after 75s`?

Related concern:

- there is visible confusion between "relay preview", "provider offline", "provider online", and "job already accepted and running".

## Scope

Primary docs reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`

Primary code reviewed:

- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/state/job_inbox.rs`
- `crates/openagents-provider-substrate/src/lib.rs`

## Executive Summary

The observed logs are not one simple failure. They are the overlap of three different state machines:

1. the relay lane transport state,
2. the provider runtime mode shown in UI,
3. the active job lifecycle.

Today those three states are not cleanly separated in the product surface.

The most important findings are:

- The app can remain relay-connected in preview mode while the provider is still "offline".
- Local Apple FM execution completion does not mean the job is "delivered" yet.
- A provider job is only treated as delivered after the result publish succeeds.
- The same 75-second TTL currently covers not only model execution, but also the post-execution publish phase.
- If result publish fails because the provider lane is offline, the job stays in `Running`, keeps retrying result publish, and can later fail with `job execution timed out after 75s` even though model execution already finished successfully.
- The same underlying relay publish failure is mirrored into both `relay.connections` and `provider.runtime`, so the console makes one failure look like many failures.

So the confusing behavior in the log is real. It is not just noisy logging. The product currently conflates:

- "observing relays while offline",
- "eligible to accept work",
- "finished local execution",
- and "successfully published the result back to the network".

## The Three Different States

### 1. Relay lane state

Owned by `apps/autopilot-desktop/src/provider_nip90_lane.rs`.

Important modes:

- `Offline`
- `Preview`
- `Connecting`
- `Online`
- `Degraded`

Important detail:

- `ProviderNip90LaneState::desired_state()` returns `Preview` whenever relays are configured and `wants_online == false`.
- That means the app can still maintain relay transport while the provider is not online for serving jobs.

Important detail:

- `provider_request_ingress_enabled()` depends only on `compute_capability.is_ready()`.
- It does not depend on `wants_online`.

Practical consequence:

- if the machine has a ready local backend, the relay lane can still subscribe to provider request ingress while offline,
- which means the desktop can still observe provider-side NIP-90 traffic in preview mode.

### 2. Provider runtime mode

Owned by `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs` and derived through `crates/openagents-provider-substrate/src/lib.rs`.

Important modes shown in UI:

- `Offline`
- `Connecting`
- `Online`
- `Degraded`

Important detail:

- when the relay lane is only in `Preview`, `derive_provider_lifecycle(...)` does not make the provider runtime `Online`.
- In the common case it leaves the provider runtime effectively offline.

Practical consequence:

- the UI can honestly say the provider is offline,
- while the relay lane is still connected and previewing traffic.

### 3. Active job lifecycle

Owned by `apps/autopilot-desktop/src/input/reducers/jobs.rs` and `apps/autopilot-desktop/src/app_state.rs`.

Important stages:

- `Accepted`
- `Running`
- `Delivered`
- `Paid`
- `Failed`

Important detail:

- local execution finishing is not the same as `Delivered`.
- `Delivered` happens only after result publish succeeds and the state machine can advance.

Practical consequence:

- you can see Apple FM complete locally,
- but the job still remains logically `Running` if the result event never publishes.

## What Each Observed Log Line Actually Means

### `Provider ingress ... preview_only=true`

This means:

- the relay lane observed a job event from relays,
- but `state.provider_runtime.mode == Offline`,
- so the request is only preview material.

This comes from `apply_ingressed_request(...)` in `provider_ingress.rs`.

Important detail:

- previewed requests should not be auto-accepted.
- `next_auto_accept_request_id_for(...)` in `jobs.rs` only accepts requests when `provider_mode == Online`.

So:

- seeing ingress while offline is expected in the current code,
- accepting while offline is not supposed to happen.

### `Provider accepting request_id=... source=job.inbox.auto_accept`

This means the provider runtime was considered `Online` at acceptance time.

Auto-accept only happens when all of the following are true:

- provider mode is `Online`,
- there are no provider blockers,
- inflight capacity is below the configured limit,
- the request is still pending and valid.

This comes from `next_auto_accept_request_id_for(...)` and the acceptance path in `jobs.rs`.

### `Provider queued Apple FM execution ...`

This means:

- the request was accepted,
- the active job was created,
- the Apple FM backend job was enqueued,
- and the execution deadline was set to `now + ttl_seconds`.

This deadline is set in `queue_provider_apple_fm_execution_start(...)`.

### `Apple FM bridge completed ...`

This means the local model produced output.

It does not mean:

- the result was published,
- the buyer saw it,
- the job reached `Delivered`,
- or the payment leg started.

The completion path only stores output and marks execution complete. It does not advance the stage to `Delivered` by itself.

### `Provider queued result publish ...`

This means the app signed a NIP-90 result event and queued it to the provider relay lane worker.

At this point the job is still not yet `Delivered`.

It becomes `Delivered` only after:

- the relay lane reports a successful publish outcome,
- `sa_tick_result_event_id` is attached,
- and `transition_active_job_to_delivered(...)` succeeds.

### `Cannot publish NIP-90 result while provider lane is offline`

This comes from `handle_publish_event(...)` in `provider_nip90_lane.rs`.

The exact guard is:

- if `!state.wants_online`,
- and the publish role is not a buyer request,
- reject the publish immediately.

There is one exception:

- buyer feedback for tracked buyer request ids may publish while offline.

That exception does not apply to provider result publishes.

So this error means:

- at publish time, the relay lane believed the provider did not want to be online,
- regardless of whether relay connections still existed,
- regardless of whether the job had already started,
- regardless of whether Apple FM had already completed locally.

This is the core mismatch in the observed logs.

### `Provider result publish failed ...`

This is the follow-on effect of the offline publish guard.

In `apply_active_job_publish_outcome(...)`:

- `result_publish_in_flight` is cleared,
- the failure is appended to the active job,
- but the job is not advanced to `Delivered`,
- and it is not immediately terminal-failed either.

So the job remains in `Running`.

### repeated `queued result publish ...` lines

This is a retry loop.

`run_active_job_execution_tick(...)` requeues result publish whenever:

- stage is still `Running`,
- local execution is complete,
- there is still no result event id,
- and no publish is currently in flight.

Because the failed publish clears `result_publish_in_flight`, the next tick just queues another result publish.

There is currently no meaningful backoff here.

### `job execution timed out after 75s`

This is the most misleading line in the entire sequence.

What it really means today is:

- the active job remained non-terminal until the TTL expired.

It does not strictly mean:

- the local model took 75 seconds,
- or Apple FM hung.

In your sample log, Apple FM completed in about 1.5 to 3 seconds. The timeout happened later because the job never advanced out of `Running`.

The timeout check runs before other transition work in `run_active_job_execution_tick(...)`.

That means:

- if publish keeps failing,
- and the stage never reaches `Delivered`,
- the same job can eventually be marked failed for "execution timeout" even though actual execution already succeeded.

This is semantically wrong for operators. It is really a post-execution delivery/publish timeout, not a model execution timeout.

### `ui error [relay.connections] ...` and `ui error [provider.runtime] ...`

These are mostly duplicated surfaces for the same underlying relay publish failure.

Why it duplicates:

- `apply_lane_snapshot(...)` copies lane snapshot errors into `relay_connections.last_error`.
- `apply_publish_outcome(...)` copies publish errors into `provider_runtime.last_error_detail`.
- `mirror_ui_errors_to_console(...)` logs both channels independently.

So one publish failure becomes:

- relay connections error,
- provider runtime error,
- plus the provider trace line,
- plus active job event entries.

That makes the console look much worse than the actual number of unique failures.

## Why This Is So Confusing In Practice

### 1. "Offline" does not mean "disconnected"

Current behavior:

- provider runtime offline can still coexist with relay preview transport.

Operator expectation:

- "offline" usually means "not receiving provider work at all".

Mismatch:

- the system currently uses "offline" to mean "not eligible to serve",
- not necessarily "not observing relays".

### 2. "Execution completed" does not mean "job delivered"

Current behavior:

- execution completion only means local output exists.

Operator expectation:

- once the model returns output, the job feels done.

Mismatch:

- publish and settlement are treated as part of the same pre-terminal path,
- but the logs do not make that separation obvious enough.

### 3. TTL currently spans both execution and publish

Current behavior:

- the same deadline governs the entire accepted/running phase until successful result publish.

Operator expectation:

- execution timeout should measure inference time,
- not relay publish failures after inference already ended.

Mismatch:

- timeout wording points blame at the model runtime even when relay state is the real blocker.

### 4. Same process can be both buyer and provider

Your sample includes:

- `autopilot_desktop::buyer`
- and `autopilot_desktop::provider`

in the same console stream.

That means one app instance may be:

- sending Buy Mode requests,
- receiving provider ingress,
- and trying to serve jobs

at the same time.

That interleaving is valid, but it makes console reading much harder because buyer success and provider failure lines sit next to each other.

## Concrete Failure Pattern In The Sample Log

The likely sequence is:

1. Provider was online and auto-accepted the request.
2. Apple FM ran and completed quickly.
3. Result publish was queued.
4. Before publish succeeded, the provider relay lane was no longer considered online for provider publishes.
5. Result publish started failing with `Cannot publish NIP-90 result while provider lane is offline`.
6. The retry loop kept requeuing result publish.
7. The active job never advanced to `Delivered`.
8. The original 75-second deadline expired.
9. The job was marked failed as an execution timeout.
10. Failure feedback publish also hit the same offline guard, causing more duplicate errors.

This is not a model-runtime failure first. It is a relay/publish-state failure that later gets mislabeled as execution timeout.

## Key Findings

### Finding 1: relay preview and provider offline are not clearly separated in operator-facing semantics

Impact:

- operators see provider ingress while "offline" and reasonably assume the app is doing the wrong thing.

Current code shape:

- preview relay transport is allowed while offline,
- and provider request ingress may still be subscribed if compute capability is ready.

### Finding 2: accepted jobs can outlive provider online intent

Impact:

- a job accepted while online can later fail to publish because the lane no longer wants to be online.

Current code shape:

- publish permission is checked at publish time,
- not locked at acceptance time.

### Finding 3: execution timeout currently includes delivery/publish time

Impact:

- operators get a false diagnosis.

Current code shape:

- deadline is set when execution starts,
- checked continuously while the job is still non-terminal,
- and not retired when local execution finishes.

### Finding 4: publish failure leads to repeated retries without a clear intermediate state

Impact:

- logs spam,
- multiple result event ids get generated,
- and the operator loses the true causal story.

Current code shape:

- failed publish clears `result_publish_in_flight`,
- tick loop requeues publish again,
- no explicit `delivery_pending_publish_retry` state exists.

### Finding 5: one relay publish error is mirrored into multiple UI error channels

Impact:

- console output exaggerates the failure count.

Current code shape:

- `relay.connections` and `provider.runtime` both mirror the same lane failure,
- plus provider trace logging.

## Recommended Fixes

### 1. Split execution completion from delivery publication in the state machine

Introduce an explicit post-execution stage such as:

- `ExecutionCompleted`
- or `DeliveryPending`

This would make it obvious that:

- the model finished,
- but the network delivery leg is blocked.

### 2. Stop calling post-execution relay failures "execution timeout"

The TTL model should be split into at least two concepts:

- execution deadline,
- publish/settlement deadline.

At minimum, once local output exists, the timeout error string should stop blaming execution.

### 3. Decide the product truth for offline behavior during active jobs

There needs to be one explicit policy:

- either `Go Offline` is blocked while an accepted job is in flight,
- or going offline drains existing jobs and preserves publish rights until they finish,
- or going offline explicitly aborts active provider jobs immediately.

Current behavior is in-between and misleading.

### 4. Make preview logging visually distinct from claimable/served logging

Previewed offline ingress should be labeled with one canonical wording everywhere:

- "preview observed only; provider offline; not claimable"

and should never look like a served job path.

### 5. Collapse duplicate relay publish errors into one canonical operator message

The console/UI should emit one high-signal line such as:

- `delivery blocked: result publish rejected because provider lane is offline`

instead of separately surfacing the same condition through:

- relay connections,
- provider runtime,
- provider result publish failure,
- and later timeout.

### 6. Add a single audit log line when online intent changes during an active job

The missing forensic line right now is something like:

- `provider online intent changed to offline while active job request_id=... is still running`

That would make these incidents immediately legible.

## Bottom Line

The logs you pasted are consistent with the current code, but the current code is not operator-truthful enough.

The core confusion is:

- relay preview transport,
- provider online eligibility,
- local model completion,
- result publication,
- and payment/settlement

are currently represented as overlapping but insufficiently separated states.

That is why the console can simultaneously imply:

- "job completed locally",
- "provider is offline",
- "result publish failed",
- and "execution timed out".

Those are not four independent failures. They are one state-machine mismatch expressed through several different layers.
