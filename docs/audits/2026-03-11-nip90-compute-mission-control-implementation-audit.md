# NIP-90 Compute + Mission Control Implementation Audit

Date: 2026-03-11
Branch audited: `main`
Audit type: static codebase audit against the retained MVP implementation and the current headless/runtime verification surfaces

## Audit Question

What do we actually have today for the NIP-90 compute implementation in Autopilot Desktop, especially around:

- provider ingress and result delivery,
- buyer settlement and race resolution,
- Mission Control pane truth,
- Apple FM-backed local execution,
- payment visibility and logs,
- and the headless end-to-end verification path?

Secondarily:

- where is the current implementation strong,
- where is it still too complex or operator-hostile,
- and how should it be improved without violating `docs/MVP.md` or `docs/OWNERSHIP.md`?

## Scope

Primary docs reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PANES.md`
- `docs/headless-compute.md`
- prior audits under `docs/audits/`

Primary code reviewed:

- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/state/operations.rs`
- `apps/autopilot-desktop/src/state/provider_runtime.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/logging.rs`
- `apps/autopilot-desktop/src/runtime_log.rs`
- `apps/autopilot-desktop/src/headless_compute.rs`

## Executive Summary

The NIP-90 compute lane in this tree is substantially real. It is not a mock marketplace and it is not just a UI prototype. The desktop app now contains a real buyer path, a real provider path, real relay ingress, real NIP-90 result and feedback publishing, real Spark wallet settlement, real Mission Control operator projection, and a real headless harness that can validate the flow deterministically and with live sats.

The strongest parts of the current implementation are:

- the app-owned product orchestration stays mostly within `apps/autopilot-desktop`, which matches `docs/OWNERSHIP.md`,
- payment truth is wallet-authoritative rather than inferred from optimistic relay events,
- Mission Control is now a serious operator surface rather than a decorative pane,
- the headless compute harness is strong enough to validate the real paid loop end-to-end,
- and recent work has materially improved result publish continuity, settlement grace windows, invoice parsing, fee visibility, and runtime logs.

The main weakness is no longer "does the flow exist?" The main weakness is that the flow is spread across too many overlapping state machines and too many app-owned reducer paths. The implementation works, but it is still harder to reason about than it should be.

In practical terms:

- Mission Control is currently a projection over several different authorities rather than a single explicit compute-flow model.
- Buyer/provider race resolution is much better than it was, but still complex because many providers can emit mixed results, payment-required feedback, processing messages, and error chatter for the same request.
- Publish continuity and settlement continuity now work, but their logic is still intricate enough that it deserves a first-class app-owned domain model instead of being distributed across reducers, pane projections, and log synthesis.
- Operator trust is still too dependent on interpreting short labels like `work=result-received`, `payment=idle`, `preview`, `delivered`, and `processing`.

Bottom line:

- the MVP NIP-90 compute implementation is real enough to ship and iterate on,
- the current risks are mostly legibility, simplification, and state-authority clarity,
- and the next improvements should focus on consolidating state, not expanding feature scope.

## MVP and Ownership Alignment

Against `docs/MVP.md`, the current tree is directionally aligned:

- the app is clearly desktop-first,
- the wallet loop is treated as a product truth rather than a side detail,
- the provider earn loop is central,
- Apple FM is integrated as the local Mac inference path,
- and Mission Control is the operator-facing control surface for the loop.

Against `docs/OWNERSHIP.md`, the current retained shape is also mostly correct:

- `apps/autopilot-desktop` owns pane orchestration, provider orchestration, payout UX, execution snapshots, and Mission Control behavior,
- reusable provider/backend identity primitives stay narrow in `openagents-provider-substrate`,
- wallet primitives remain in `spark`,
- and product-specific NIP-90 orchestration has not been pushed down into generic crates.

This matters because the next round of cleanup should not be "move more logic into reusable crates." The next cleanup should be "make the app-owned compute flow easier to understand inside `apps/autopilot-desktop`."

## What Is Implemented Today

### 1. Provider lane: real relay-backed NIP-90 transport

`provider_nip90_lane.rs` is a real worker-backed lane, not just a helper module.

What it owns today:

- relay pool lifecycle,
- preview vs online transport mode,
- publish requests and publish outcomes,
- tracked buyer-response request ids,
- tracked provider-publish continuity ids,
- ingress subscriptions,
- NIP-89 handler publication,
- and lane snapshots that feed the app.

Important current truth:

- the lane has explicit `Preview`, `Online`, and `Degraded` modes,
- `desired_state()` uses `wants_online` to separate preview transport from provider eligibility,
- `provider_request_ingress_enabled()` is backend-readiness-sensitive,
- and publish behavior already distinguishes between ordinary provider publishing and continuity cases that still need to publish while the provider is otherwise offline.

This is a strong transport seam. The lane already behaves like a real protocol actor.

### 2. Provider ingress and jobs: real provider orchestration

`provider_ingress.rs` and `jobs.rs` together implement the app-owned provider lifecycle.

What they do today:

- receive and filter ingressed requests,
- apply target-policy checks,
- support preview-only rows while offline,
- auto-accept matching requests when online,
- create active job state,
- queue Apple FM execution,
- publish processing feedback,
- publish canonical result events,
- retry result publish when relay confirmation stalls,
- advance to `Delivered` only after result publish is actually confirmed,
- generate and publish `payment-required` feedback,
- and wait for wallet-authoritative settlement before terminal success.

Recent improvements matter here:

- result publish continuity now has its own retry and timeout window,
- settlement has its own grace window instead of sharing raw execution TTL,
- malformed `payment-required` feedback without an invoice is treated as nonterminal noise instead of an immediate false terminal failure,
- and Active Job now exposes more truthful lifecycle details and copyable logs.

This is no longer a toy provider loop. It is a real provider state machine.

### 3. Buyer path: real race resolution and wallet-authoritative settlement

`state/operations.rs` is where the buyer path becomes real product logic rather than raw event collection.

What it does today:

- tracks per-request provider observations,
- correlates results, feedback, invoices, wallet pointers, and resolution reasons,
- selects a payable winner,
- queues Spark payment only when the winning provider is actually payable,
- ignores losing-provider noise once a valid payable winner exists,
- and treats wallet confirmation as the authority for paid state.

The most important recent buyer-side correction is conceptual:

- the app no longer treats "first provider with any result" as good enough,
- it now requires a provider to have both a usable invoice and a non-error result before selecting that provider as the payable winner.

That is the correct product behavior for the MVP. It is much closer to truthful operator semantics.

### 4. Mission Control: real operator projection, not just a theme

Mission Control is assembled primarily out of `app_state.rs`, `pane_renderer.rs`, and `pane_system.rs`.

What the product now exposes:

- provider mode and blockers,
- Apple FM readiness,
- wallet status,
- relay preview / inbox visibility,
- active job status,
- buy mode state,
- buy-mode payment history,
- log-stream copy actions,
- active job copy actions,
- and explicit operator-facing summaries.

The core synthesis point is `build_mission_control_log_lines(...)` in `app_state.rs`. That function rolls together:

- provider runtime truth,
- local runtime truth,
- provider blockers,
- wallet actions and errors,
- recent network request states,
- job inbox state,
- and active job state.

That projection is one of the best parts of the retained MVP surface. It makes the system operable.

### 5. Logging and runtime logs: materially better than most desktop apps

`logging.rs` and `runtime_log.rs` provide a serious observability path:

- tracing events are persisted to JSONL session logs,
- `latest.jsonl` is maintained for easy access,
- selected tracing targets are mirrored into Mission Control,
- and UI errors are mirrored back out to console channels.

This means the desktop now has:

- operator-facing logs,
- persistent logs for postmortems,
- and machine-readable logs for agents or scripts.

That is a significant strength for a desktop-first marketplace app.

### 6. Headless compute: the strongest verification surface in the tree

`headless_compute.rs` and `docs/headless-compute.md` are especially strong.

Today the repo already has:

- a local relay harness,
- a headless buyer,
- a headless provider,
- deterministic canned backend paths,
- multi-payment roundtrip scripts,
- and live-sats verification capability.

Recent work also pushed fee visibility through the headless flow, which matters because the MVP promise is not just "jobs finish," it is "the money movement is real and legible."

This headless surface is good enough to be treated as a first-class regression harness for the product.

## Strengths

### 1. The implementation is app-owned in the right places

The current design mostly respects the MVP ownership boundary:

- protocol transport stays narrow,
- reusable crates stay reusable,
- and product-specific orchestration remains in `apps/autopilot-desktop`.

That is the right architecture for this stage.

### 2. Wallet truth is explicit

The implementation has moved toward the correct truth model:

- request published is not payment,
- result received is not payment,
- payment-required received is not payment,
- and provider "success" is not payment unless wallet-authoritative settlement also exists.

That is the right discipline for an earn product.

### 3. The provider lifecycle is meaningfully staged

The provider path now distinguishes:

- accepted,
- running,
- awaiting relay confirmation,
- delivered,
- awaiting settlement,
- and settled.

This is materially better than collapsing everything into a single "done" boolean.

### 4. Mission Control now has operational value

Mission Control is no longer empty chrome. It exposes:

- market preview while offline,
- blockers,
- buy mode status,
- payment history,
- log copy,
- active job copy,
- and operator-facing summaries.

That is the right product direction.

### 5. The headless harness reduces shipping risk

The presence of a deterministic and live-sats verification path is unusually valuable.

It gives the team a way to verify:

- relay ingress,
- provider execution,
- result publish,
- invoice publication,
- buyer payment,
- fee accounting,
- and provider settlement,

without depending only on the desktop UI.

## Findings and Gaps

## Finding 1: Mission Control is a projection over multiple authorities, not a single explicit compute-flow model

This is the biggest remaining product/architecture issue.

Today Mission Control rolls together several different authorities:

- provider lane transport state,
- provider runtime mode,
- Apple FM backend readiness,
- active job lifecycle,
- job inbox visibility,
- buyer request state,
- and wallet settlement state.

That projection is useful, but it is still hard to reason about because the underlying states live in different places and use different vocabularies.

Examples:

- `Preview` and `Offline` are different truths but can still feel similar in the pane.
- `result-received` and `payment-required` can both coexist for the same request while loser-provider noise keeps arriving.
- `Delivered` means result publish continuity succeeded, not that money moved.
- `processing` feedback from losers can still visually compete with the winning provider.

This is no longer a correctness crisis, but it is still an operator-legibility problem.

## Finding 2: The end-to-end NIP-90 product flow is still spread across too many app files

The product behavior is currently split across:

- `provider_nip90_lane.rs`
- `provider_ingress.rs`
- `jobs.rs`
- `operations.rs`
- `provider_runtime.rs`
- `app_state.rs`
- `pane_renderer.rs`
- `logging.rs`

That is survivable, but it makes it harder to verify invariants like:

- when exactly a job becomes delivered,
- when exactly a buyer request becomes payable,
- which provider is the current winner,
- when loser-provider errors should be ignored,
- and which state should be rendered as operator truth.

The implementation works because the team has been patching the seams. It would be healthier if more of this was represented through a single app-owned flow snapshot.

## Finding 3: Buyer race-mode remains semantically complex even after recent fixes

The buyer path is much better, but the logs show why this is still a hard area:

- multiple providers can emit `processing`,
- some providers emit `payment-required`,
- some providers emit `result`,
- some emit `error`,
- some emit all of the above in different orders,
- and some provide zero-amount or malformed result noise.

The new payable-winner logic is the right fix. But from a product-model standpoint, the buyer still needs a clearer first-class concept of:

- current selected provider,
- why that provider is selected,
- which providers lost and why,
- and which events are now informational only.

Right now that reasoning is in the code, but not exposed cleanly enough in the product.

## Finding 4: Publish continuity and settlement continuity are correct enough, but still too implicit

Recent fixes gave result publish and settlement their own grace windows. That was necessary and correct.

But the underlying model is still more implicit than it should be. The app currently reasons about:

- execution completion,
- signed result publication,
- relay publish outcome,
- `Delivered`,
- invoice generation,
- `payment-required` feedback publication,
- wallet pointer assignment,
- wallet settlement,
- and paid terminal state.

Those are all real phases, but they are distributed across several reducer branches and pane summaries.

This is now correct enough to work, but still too brittle to be the final shape.

## Finding 5: Mission Control still compresses too much into short status strings

The pane has improved a lot, but the operator still has to infer meaning from concise labels like:

- `work=queued`
- `work=result-received`
- `payment=idle`
- `payment=queued`
- `preview`
- `processing`
- `streaming`

That compactness helps layout, but it still leaves the user to answer critical questions mentally:

- Which provider are we actually going to pay?
- Are we waiting on relay delivery or on settlement?
- Is this request blocked, racing, payable, or lost?
- Is the active job healthy or only locally complete?

The pane should not make the operator reverse-engineer this.

## Finding 6: Logs are strong, but still too generic for fast postmortems

The runtime-log path is good. The Mission Control mirror is good. The copy actions are good.

But the semantics of the logs still skew too close to raw implementation events rather than stable domain events. That means logs are informative, but not always immediately decisive.

Example classes that deserve more normalized phrasing:

- buyer selected payable provider
- buyer ignored loser-provider error
- provider awaiting relay confirmation
- provider awaiting wallet settlement
- settlement invoice published
- settlement invoice superseded
- settlement confirmed by wallet

This is an observability polish issue, not a correctness blocker.

## Finding 7: There is still too much manual projection logic in `app_state.rs`

`app_state.rs` is doing a lot of legitimate product work, but it is also becoming a place where multiple compute truths are reassembled for panes.

That is not an ownership violation, but it is a concentration risk:

- more pane logic tends to accumulate there,
- more string synthesis tends to accumulate there,
- and more "UI truth" starts to drift away from primary state transitions.

This is the right place for app-owned projection, but it needs a cleaner domain snapshot to project from.

## Improvement Plan

### Priority 0: Introduce one app-owned compute flow snapshot

Add a single app-owned domain model inside `apps/autopilot-desktop`, something like:

- `Nip90ComputeFlowSnapshot`
- or `ComputeMarketFlowSnapshot`

It should normalize, for each active request/job:

- authority: `relay`, `provider`, `wallet`, or `ui`
- phase: `preview`, `accepted`, `executing`, `publishing_result`, `delivered`, `requesting_payment`, `awaiting_payment`, `paid`, `failed`
- selected provider
- payable provider, if different from current selected provider
- current blocker, if any
- next expected event
- timer/deadline context

This should stay app-owned in `apps/autopilot-desktop`. Do not push this into reusable crates.

Why this matters:

- Mission Control can render from one explicit truth source.
- Active Job can render from the same truth source.
- Buy Mode Payments can render from the same truth source.
- Logs can emit normalized state transitions from the same truth source.

This is the highest-value cleanup.

### Priority 1: Make winner selection visible in the UI

Mission Control and Buy Mode Payments should show:

- selected provider,
- payable provider,
- losing providers count,
- and a short loser summary such as:
  - `2 losers ignored: no invoice, error-only`

That is more truthful than letting the user infer the winner from whatever feedback line arrived most recently.

### Priority 2: Promote "next expected event" to a first-class UI field

For both buy mode and the sell-side active job, show:

- `next=relay confirmation`
- `next=provider invoice`
- `next=wallet settlement`
- `next=buyer result`
- `next=none`

This will do more for operator trust than adding more raw log lines.

### Priority 3: Promote publish continuity state into Mission Control

Right now publish continuity is exposed mostly through Active Job and logs.

Mission Control should also clearly expose:

- result signed,
- relay publish attempts,
- age since first publish attempt,
- current continuity window,
- and whether the flow is waiting on relay confirmation or settlement.

That closes the remaining gap between "local execution succeeded" and "the market saw it."

### Priority 4: Normalize domain log events

Keep the raw tracing, but add a cleaner domain-event layer for the operator-facing copy and JSONL entries:

- `buyer.selected_payable_provider`
- `buyer.queued_payment`
- `buyer.payment_settled`
- `provider.result_signed`
- `provider.result_published`
- `provider.payment_requested`
- `provider.settlement_confirmed`
- `provider.loser_feedback_ignored`

This will make runtime logs much easier to audit automatically.

### Priority 5: Reduce flow duplication between headless and desktop orchestration

The headless harness is strong, but the desktop and headless paths should continue converging on the same domain semantics.

The goal is not necessarily one shared mega-module. The goal is:

- same phase names,
- same winner-selection semantics,
- same fee semantics,
- same settlement semantics,
- same publish continuity semantics,
- same domain log phrases.

That will reduce the chance that desktop and headless each become correct in slightly different ways.

### Priority 6: Make fee truth more operator-visible

Recent work added fee tracking. The next step is to make those fees more obvious in buyer and provider operator surfaces:

- invoice amount,
- Spark routing fees,
- total buyer debit,
- provider received amount,
- and net delta to each wallet.

The money loop should be explicit everywhere it matters.

### Priority 7: Add a small Mission Control “truth legend”

Mission Control is dense. A small persistent legend would help:

- `PROV = selected provider`
- `WORK = market/job phase`
- `PAY = wallet phase`
- `NEXT = next expected event`

This is simple, but it would reduce cognitive friction for testing and operations.

## What Not To Do

To improve this implementation without violating the MVP guardrails:

- do not move Mission Control orchestration into `wgpui`,
- do not move app-specific payout UX or NIP-90 flow correlation into `spark`,
- do not move product-specific compute-flow semantics into `openagents-provider-substrate`,
- do not add more product surfaces before the existing Mission Control truth model is simplified,
- and do not replace wallet-authoritative truth with more optimistic protocol inference.

The right move is simplification and consolidation inside the app layer.

## Recommended Next Sequence

1. Create an app-owned `Nip90ComputeFlowSnapshot` and render Mission Control, Active Job, and Buy Mode from it.
2. Expose winner selection and loser reasons explicitly in Mission Control and Buy Mode Payments.
3. Add `next expected event` and `authority` fields to the operator surfaces.
4. Normalize domain log events in both Mission Control and JSONL runtime logs.
5. Keep using headless compute as the regression truth surface for every major NIP-90 flow change.

## Follow-up Completion Addendum

The cleanup arc above has now been completed on `main` through issues `#3387` to `#3393`.

Delivered from that sequence:

- one app-owned NIP-90 compute flow snapshot now drives the main operator projections
- Mission Control and Buy Mode expose selected provider, payable provider, loser summaries, and next expected event
- publish continuity and settlement continuity are explicit in both Mission Control and Active Job surfaces
- Mission Control and session logs emit normalized compute-domain events
- headless and desktop validation now use aligned phase and settlement semantics
- buyer/provider fee truth is visible in the operator panes and headless logs
- Mission Control includes a persistent truth legend so the dense status cells are easier to interpret

Residual guidance:

- this audit tracker is complete
- future NIP-90 compute cleanup should start with a fresh audit against the current `main` tree rather than reopening this sequence
- new follow-up issues should be framed around post-cleanup simplification or new MVP requirements, not the already-landed truth-model work

## Final Assessment

The MVP NIP-90 compute implementation is now credible.

It has:

- real transport,
- real execution,
- real publish continuity,
- real wallet settlement,
- real fee accounting,
- real operator panes,
- and real end-to-end verification surfaces.

The main problem is no longer "missing implementation." The main problem is that the implementation still asks too much of the operator and too much of the engineer reading the code.

The next best move is not more features. The next best move is to consolidate the app-owned compute flow into one clearer domain model and make Mission Control render that truth directly.
