# Claude Agent Bridge × The Promise Registry — Leverage Audit

Date: 2026-06-10 (night)

Registry at audit time: `2026-06-10.24` (live at
`/api/public/product-promises`), 39 outstanding promises
(yellow/red/planned) reviewed in full.

## What this audit answers

The local Claude Agent bridge shipped tonight (epic #4717: probe +
capability declaration, bounded executor gate, dispatch work class,
CI-safe smoke — all on `main`, promise
`pylon.local_claude_agent_bridge.v1` yellow on one live-leg blocker).
The question: **which outstanding promises does this new capability
genuinely supercharge, and what should be built next to cash that in?**

"Supercharge" is held to a strict meaning here: the bridge must change
the *shape* of the path to green — turning a missing engine into an
existing one — not merely be thematically adjacent. Most of the 39
outstanding promises fail that test (payment-rail, Forum, identity,
training-math, and policy promises don't care which executor exists).
Three clusters pass it decisively.

## The new capability, stated precisely

What exists on `main` tonight that did not exist this morning:

1. Any Pylon on a machine with the user's own Anthropic credentials can
   truthfully declare `capability.pylon.local_claude_agent`
   (probe-gated, BYOK-only, stripped when stale).
2. The worker loop can execute a typed `claude_agent_task` work class:
   bounded workspace, sandboxed Claude Agent SDK session
   (escape-denial hook, `settingSources` isolation, turn/wall-clock
   budgets), **independent test-command verification**, ref-only
   public-safe closeout, five typed refusal arms.
3. An operator can dispatch that work class to a specific capable Pylon
   (`claude-agent-task-dispatch.ts`), with the capability ref enforced
   at Pylon-side admission, not just at dispatch.
4. The whole lifecycle is smoke-proven CI-safe and release-gate-wired;
   the live leg is a documented three-command operator runbook.

In one sentence: **the network now has a general-purpose coding executor
on contributor machines, behind the same admission/closeout/settlement
loop that already moved real sats for the Tassadar lane.** Before
tonight the fleet could execute exactly one work family (digest-pinned
numeric traces); now it can, in principle, execute anything a bounded
agent session plus a verification command can express.

## Cluster 1 — The labor stream (three promises, one shared blocker)

| Promise | State | Shared blocker |
| --- | --- | --- |
| `provider.compliant_usage_labor.v1` | red | `labor_stream_not_live` |
| `pylon.five_bitcoin_revenue_streams.v1` | red | `labor_stream_not_live` (1 of 4) |
| `autopilot.agentic_labor_products.v1` | yellow | `not_all_labor_flows_self_serve` |

This is the highest-leverage match in the registry, and it is almost
eerie how exactly the bridge fits the promise text.
`provider.compliant_usage_labor.v1` claims: *"Contributors can connect
their own provider accounts or prepaid API budgets and earn Bitcoin by
doing useful work with that compliant usage through the agent labor
market; OpenAgents never resells provider access."* Its verification:
*"A labor job must run on the contributor's own connected account or
API budget with output-only delivery, payment for accepted results, and
a public settlement receipt. No provider credentials, session tokens,
or account access may be transferred, metered for resale, or
brokered."*

Read the bridge's properties against that, line by line:

- "contributor's own connected account or API budget" → the BYOK-only
  probe; the capability declares itself only when the user's own
  credentials are present, and agents/platform never touch the values.
- "output-only delivery" → the ref-only closeout through the projection
  scanner; raw session material stays on the device by construction.
- "no credentials transferred, metered for resale, or brokered" → the
  lane's hard policy (no claude.ai login brokering, no platform keys on
  devices) — written into the promise's own `unsafeCopy`, the bridge
  docs, and the launch gates.
- "payment for accepted results, public settlement receipt" → the one
  piece the bridge does *not* provide — and that piece already exists
  and is proven: the Tassadar PoC settled 1,000 real sats through the
  identical assignment/closeout loop, and the reliable-tips ladder is
  green.

Before tonight, the labor stream's missing piece was the labor itself —
the network had no compliant way to *do useful work* with a
contributor's own AI budget. The bridge is that way. The remaining
distance to the first settled labor receipt is composition, not
construction.

Also directly fed: `payments.accepted_outcome_economics.v1` (red) needs
"one accepted outcome" to anchor its ledger states — a paid, accepted
claude-agent task is a far more representative anchor outcome than a
numeric-trace replay; and `autopilot.codex_cloudcode_wrapper.v1`-class
copy finally gets a true sentence: the runtime wraps a real coding
agent.

## Cluster 2 — The coding-runtime successor and the owner's sentence

| Promise | State | Blocker |
| --- | --- | --- |
| `autopilot.codex_probe_pylon_successor.v1` | yellow | `live_probe_pylon_runtime_gates_incomplete` |

This is the closest single flip. The blocker's whole content (per
#4661 and the #4633 honest caveat) is: a real coding task executed by
the installed Pylon binary's worker loop through the live assignment
lifecycle — not an agent driving the API by hand. The bridge's live
smoke leg *is* that, verbatim: same lifecycle, real edit, real test,
delivered, reviewed. One operator-credentialed run produces evidence
for two promises at once (this one and
`pylon.local_claude_agent_bridge.v1`), and #4661's Codex adapter can
then land as a peer behind the same gate at leisure instead of being
the single thread the promise hangs on.

Beyond the flip, this cluster is where the owner's original target
sentence lives — *"through my Pylon, ask my agent to do very cool shit,
coding shit gets done ASAP"* (the full-flow audit's leg 1 and leg 6).
The bridge closed leg 6 for the fixture class. The remaining distance
to the demo is (a) a work class beyond fixtures — a public-repo
checkout with a caller-supplied verification command — and (b) the
still-unowned `pylon work submit` entry command from the full-flow
audit. Both now have a working executor to land on, which they did not
this morning.

`autopilot.control_center_fanout_marketplace.v1` (red) sits behind this
cluster: "fan out work to many agents" was vacuous when zero agents
could do coding work; a fleet of capability-declaring Pylons makes
fan-out a dispatch-policy problem instead of a fantasy.

## Cluster 3 — Artanis gets a second real work class (and the trace corpus)

| Promise | State | Relevant blockers |
| --- | --- | --- |
| `artanis.tassadar_evolution_loop.v1` | yellow | `artanis_scheduled_runner_real_actions_missing`, `tassadar_distillation_dataset_receipt_missing` |
| `pylon.data_trace_revenue.v1` | red | `settled_trace_sale_missing` |

The evolution loop's design (the administrator dispatches work on its
tick, verifies results, accumulates a verified corpus) currently has
exactly one dispatchable work class. The bridge adds the second — and
the first one whose outputs are *coding* artifacts. Artanis's tick can
dispatch bounded claude-agent tasks to capable Pylons under the same
schema-validated, budget-gated proposal discipline the loop already
mandates (the mind proposes, gates hold). Verification has a natural
class: a validator device re-runs the verification command against the
submitted artifact refs — the coding analogue of `exact_trace_replay`,
and a concrete instance for `training.verification_classes.v1`
(planned).

The data angle is quieter but real: `pylon.data_trace_revenue.v1`'s
claim is literally *"mining valuable local traces from Claude Code,
Codex, and other agent work."* The bridge produces, on every run, a
complete local session JSONL that never leaves the device — which is
precisely the consent-gated raw material that promise needs. Nothing
should ship traces anywhere today (consent/redaction/valuation are the
promise's named gates), but the corpus now accumulates on contributor
devices as a by-product of paid work, instead of needing to be
conjured later.

## Honorable mentions (real but weaker leverage)

- `pylon.release_tomorrow.v1` / `pylon.v03_release_candidate.v1`: the
  bridge ships inside the v0.3 package by default (optional dep, lazy
  import, gate-green), so the release story gains a marquee capability
  for free — but the release blockers themselves (npm publish story,
  platform smokes) are untouched by it.
- `autopilot.mission_briefing.v1` / `autopilot.decision_queue.v1`:
  claude-agent work orders make briefings/decisions meaningful (real
  diffs to review), but the projections themselves don't need the
  bridge.
- `marketplace.signature_monetization.v1`, `energy.*`, `training.*`
  (math lanes), identity/Forum/payment promises: no structural change
  from this capability. Listed so the selection above is auditable as a
  choice, not an oversight.

## Top three next steps, in order

### 1. The first settled paid labor job on a local Claude (Cluster 1)

The single most valuable composition available, and nearly all of it
exists. Sequence:

1. Run the live leg of the bounded-task smoke (the epic #4717 operator
   runbook — three commands on a credentialed machine). This alone
   flips `pylon.local_claude_agent_bridge.v1` to green-proposable and
   clears `autopilot.codex_probe_pylon_successor.v1`'s last blocker —
   two promises from one run.
2. Extend `claude-agent-task-dispatch.ts` with the operator-funded paid
   mode the Tassadar PoC already proved (`paymentMode:
   payable_pending_settlement` → operator paid closeout → settlement to
   the Pylon's admitted payout target over the ladder). The settlement
   machinery needs zero new code; the dispatch script needs one flag.
3. Dispatch one paid claude-agent task to a real contributor Pylon
   (not owner-operated, per the independence discipline the tips lane
   learned), accept the delivered result, settle, and record the public
   receipt.

Receipts touched: first `labor_stream_not_live` evidence for
`provider.compliant_usage_labor.v1` and the five-streams labor lane, a
self-serve-shaped flow for `autopilot.agentic_labor_products.v1`, and a
candidate anchor outcome for `payments.accepted_outcome_economics.v1`'s
ledger work. This is "an agent earned Bitcoin doing coding work with
its owner's own AI budget" — the sentence the company exists to make
true, with receipts.

### 2. The real-repo work class and the `pylon work` entry command (Cluster 2)

Make the bridge do work people actually want, and let the owner ask for
it the way the target sentence says:

1. Add a `git_checkout` workspace kind to the claude-agent task payload
   (public repos only; pinned commit; caller-supplied verification
   command run the same independent way; same escape/budget/redaction
   law). The fixture registry stays as the smoke class; real tasks
   point at repos. This is the difference between "can repair a planted
   bug" and "can do your issue."
2. Build `pylon work submit|status|review` wrapping the live
   `POST /api/autopilot/work` API with the registered identity — the
   entry leg the full-flow audit flagged as unowned. The Autopilot
   placement/lease/delivery/review spine it lands on ran live in
   production on 2026-06-09 (#4633); the executor it needs now exists.
3. The owner demo drops out: `pylon work submit "fix the failing test
   in <public repo>"` → placement selects the owner's Pylon → the local
   Claude does it → delivered → review accept, all on production.

### 3. Artanis dispatches coding work on its tick (Cluster 3)

Give the autonomous loop its second work class and start the verified
coding corpus:

1. A `claude_agent_task` tick action for the Artanis scheduled runner
   (#4697/#4701 pattern): scan for capability-declaring online Pylons,
   propose a bounded dispatch under the per-tick budget, schemas
   validate, gates hold. Contributes directly to
   `artanis_scheduled_runner_real_actions_missing`.
2. Define the verification class: validator re-execution of the
   verification command over submitted artifacts (the coding analogue
   of `exact_trace_replay`) — a concrete first instance for
   `training.verification_classes.v1`.
3. Specify (do not yet ship) the consent-gated session-trace retention
   contract so `pylon.data_trace_revenue.v1`'s future
   redaction/consent/valuation work has a real corpus to stand on.

## Boundaries that hold across all three

No platform credentials on devices; the contributor pays for their own
inference and is paid for accepted output. Worker closeout is never
accepted work; settlement stays behind the existing operator/policy
gates. Session traces stay on-device absent explicit consent machinery
that does not exist yet. Copy law: nothing above may be claimed as live
before its receipts exist; the lane is "Claude Agent" / "your local
Claude", never "Claude Code". And per the campaign conventions: this
audit proposes; evidence flips promises; nobody flips their own.

## One-sentence truth

Tonight the network gained a compliant, sandboxed coding executor on
contributor machines behind a payment loop that already moves real
sats — the labor stream's missing engine, the coding-runtime promise's
missing evidence, and the autonomous loop's missing second work class —
and the three moves above convert it, in order, into the first paid
labor receipt, the owner's ask-your-Pylon demo, and an administrator
that hands out coding work by itself.
