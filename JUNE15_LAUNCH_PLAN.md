# JUNE 15 LAUNCH PLAN — The Tassadar Run

Date authored: 2026-06-14 (Sunday). Target: **Monday 2026-06-15**.

We are launching the **Tassadar run** tomorrow. This is the plan to make the core
experience real: a public Tassadar training run goes live, real (non-owner)
people install **Autopilot** (our Electrobun desktop app), get dispatched
**executor-trace work**, it's verified by **exact replay**, and they **earn real
Bitcoin** for it — with public receipts — while their accepted work accumulates
the verified-trace corpus that trains Tassadar.

This is not CS336. CS336 is shared plumbing (training-run routes, the settlement
ladder, the verification-class registry). The **run we are launching is
Tassadar** — the Percepta Executor Class model direction from Episode 236.

## 0. The node software is Autopilot (Electrobun desktop), not a standalone Pylon CLI

**Clarification (2026-06-14):** the contributor-facing install is now **Autopilot
Desktop** — `@openagentsinc/autopilot-desktop`, an Electrobun + Foldkit app — not
a standalone "Pylon v0.3" CLI/TUI. Audited state of `apps/autopilot-desktop`:

- It is the **cockpit over the local node**: the Bun main process owns local node
  control over loopback (via the shared `autopilot-control-protocol` bridge /
  control token), and the webview renders the operator UI. The **Pylon runtime
  still exists as the local node Autopilot drives** — "Pylon v0.3" is superseded
  as a separate user-facing install, not deleted as a runtime.
- It already drives the **full training-contribution loop** through local node
  intents: request bootstrap, claim training lease, plan/activate/reconcile a
  training window, build the evidence packet, admit evidence, and queue training
  launch + closeout (`ClaimTrainingLease`, `ActivateTrainingWindow`,
  `ReconcileTrainingWindow`, `BuildTrainingEvidencePacket`, `QueueTrainingLaunch`,
  `QueueTrainingCloseout`), plus the lane selector (auto|local|cloud-gcp|cloud-shc)
  and session spawn. It has a Training cockpit pane (`oa-training-run`, gated by
  `verify:autopilot-desktop:training`).
- It ships as a **signed + notarized macOS `.app`** with an OTA update feed
  (`updates.openagents.com/desktop/stable/feed.json`, BSDIFF deltas). Pricing TBD.

**The honest seam for the launch:** today Autopilot Desktop **discovers an
existing running local node** (it reads the control token from a `.pylon-*` home);
it does **not yet bundle/launch the node itself**. So "install Autopilot and
contribute" is only one-step once the app bundles + launches the node runtime.
Making that one-step is on the install-path critical path (§4.F). Where this plan
says "the node" it means the local Pylon runtime Autopilot controls; where it says
"install," it means installing **Autopilot Desktop**.

> Follow-up: the product-promise registry still frames the install as Pylon v0.3
> (`pylon.v03_release_candidate.v1`, `pylon.release_tomorrow.v1`,
> `pylon.install_without_wallet_knowledge.v1`) alongside
> `autopilot.desktop_gui_client.v1`. Reconcile that copy to the Autopilot-is-the-
> install positioning in a registry pass (separate from this doc; it needs a
> Worker deploy).

> From Episode 236 (`docs/transcripts/236.md`): "Monday we're launching the
> largest decentralized training run… you're just going to install our node
> software and you're going to get paid Bitcoin to contribute to a training run…
> a very fancy new architecture… Percepta Executor Class model… CPU computation
> transform… adding support for that to Pylon version 0.3 paired with the Bitcoin
> payments… one piece of software that's going to earn you Bitcoin in multiple
> different ways, including helping train this very experimental but we think very
> powerful new kind of model which we're calling **Tassadar**. We'll launch that
> Monday."

> Grounding: built from `docs/transcripts/236.md`, the Tassadar lane
> (`docs/tassadar/README.md`, `compute.tassadar_executor_poc.v1` green,
> `artanis.tassadar_evolution_loop.v1` yellow), the Episode 236 gap audit
> (`docs/2026-06-12-episode-236-training-launch-gap-audit.md`), the live surfaces
> below, and the promise registry (`/api/public/product-promises`, `2026-06-14.4`).

---

## 1. What the Tassadar run actually is

Tassadar is the **compiled exact-executor lane**: digest-pinned exact-program
workloads dispatched to contributor machines, run, and **verified by exact trace
replay** on a separate device (a verdict is just re-execution + a digest
comparison — the cheapest, strongest verification that exists). Accepted work is
paid in sats and its verified traces accumulate the corpus that trains the
Tassadar / Percepta Executor Class model.

Why Tassadar is the **right** run to launch into a public, paid, many-contributor
event — these are advantages, not hedges:

- **Verification is trivial and strong.** Exact replay means even the weakest
  device in the funnel can both contribute *and* validate. No gradient quorum, no
  statistical grading — replay + digest match.
- **It already settled real money.** `compute.tassadar_executor_poc.v1` is green:
  a real Pylon ran a digest-pinned workload, the worker re-executed it as a
  separate validator (Verified, plus a Rejected on a tampered digest), and one
  paid closeout settled over real Lightning.
- **The dispatch/verify loop is already live.** The Artanis evolution loop
  (`/api/public/artanis/admin-ticks`) is in production, dispatching executor-trace
  workloads and publishing per-tick receipts. Today it's mostly no-spend and
  dispatch-fails for lack of eligible online devices — which is exactly what a
  public launch fixes by bringing the fleet online.

The job for tomorrow is to turn that bounded, owner-driven loop into a **public
Tassadar run a stranger can join self-serve and earn from.**

---

## 2. Definition of Done (the scoreboard)

The Tassadar run launch is real — not just copy — when all of these are publicly
verifiable:

1. **The Tassadar run is RUNNING.** ✅ **met (#5006)** — the public run page
   reports `state: active` (not `planned`), with the stable `trainingRunRef`
   `run.tassadar.executor.20260615`, a published manifest, and a fresh
   `generatedAt` / `live_at_read` staleness contract.
2. **A non-owner joined self-serve.** Someone who is not the owner installed
   **Autopilot Desktop**, brought a node online, declared the executor capability,
   was admitted to the Tassadar run, and was dispatched real executor-trace work —
   without operator hand-staging.
3. **Their work was verified by exact replay.** A public
   `training.verification.challenge.<id>` `Verified` (exact_trace_replay) verdict
   references their submission.
4. **They earned real sats, and can see it.** Accepted executor-trace work
   settled a real Lightning payout to the contributor's wallet, with a
   dereferenceable public receipt, AND the public run / leaderboard shows
   `settledPayoutSats > 0`. **Seam met (#5009):** the settlement-receipt route +
   projection read are live — `settledPayoutSats` / `providerConfirmedSettledPayoutSats`
   now reflect run-linked settled receipts (no longer hardcoded `0`). Goes
   non-zero the moment the first real settlement lands — the launch event.
5. **The corpus grew.** The accepted verified traces are recorded toward the
   Tassadar training corpus (the evolution loop's accumulation), visible on the
   tick ledger.

Hitting #2 + #4 for one stranger is the moment "install Autopilot, help train
Tassadar, get paid Bitcoin" becomes a fact instead of a promise.

---

## 3. Current reality (honest snapshot, 2026-06-14)

Green / proven:

- **`compute.tassadar_executor_poc.v1` (green):** one bounded executor-trace
  workload, dispatched to a real Pylon, replay-verified on a separate device,
  one paid Lightning closeout with balance receipts on both sides.
- **`artanis.tassadar_evolution_loop.v1` (yellow):** the automated
  dispatch → replay-verify → accumulate loop is deployed; the public tick monitor
  is live; it dispatched and closed out no-spend executor work autonomously once.
- **Verification:** `exact_trace_replay` is a live, exercised verification class;
  the **reliable-tips ladder** settles real sats and never drops them (green).
- The **node runtime** (Pylon v0.3-rc2) runs the agent surface from the device
  (green `pylon.v03_agent_economy.v1`) and is what **Autopilot Desktop** drives
  over loopback; operator-staged install-to-bitcoin settled a real 21-sat payout
  (yellow).

Shipped since this plan was written:

- ✅ **Step A — run authority + manifest is live** (#5006, deployed). The public
  Tassadar run `run.tassadar.executor.20260615` reports `state: active` (not
  `planned`) with its launch manifest (`workloadFamily: executor-trace`,
  `verifierPolicy: exact_trace_replay`, `paymentMode: operator_approved_small_sats`,
  spend cap, status URL, abort rule), a `live_at_read` staleness contract, and
  typed blockers; a run-level state-transition route now moves runs off `planned`
  without D1 patches. No promise flipped green.
- ✅ **Step B — executor-trace admission + claimable work is live** (#5007,
  deployed). `POST /api/training/runs/{ref}/admit` makes a **reasoned admission
  decision** (receipted executor capability + owner-operated check + the #4852
  host-RAM device gate, every branch with a stated measured reason), and the run
  now carries a claimable, **digest-pinned executor-trace work window**
  (`activeWindowCount: 1`); verification is already run-aware (`exact_trace_replay`
  challenge carries `trainingRunRef` + `windowRef`). The **live non-owner
  admit→claim→verify run-through is the launch event** (§6), not faked on the
  production run; `assignedContributorCount` stays 0 until a real contributor
  claims. No-spend only; no promise flipped green.
- ✅ **Step C — closeout → run-tied verification is live** (#5008, deployed).
  `POST /api/training/runs/{ref}/executor-trace-closeout` takes a contributor's
  executor-trace closeout and creates a **run+window-tied `exact_trace_replay`
  verification challenge** (the builder was previously unwired), enforcing the
  distinct-validator-device rule. On validator replay a `Verified` verdict
  surfaces in the run's `verifiedWorkCount` (a tampered digest → `rejectedWorkCount`).
  The live closeout→replay run-through is the launch event (§6);
  `verifiedWorkCount` stays 0 until then. No-spend only; no promise flipped green.

Red / the gap (the rest of the critical path):

- **No self-serve contributor path** — the PoC and loop were owner-driven; the
  fleet is thin (~4 nodes online / 51 registered, mostly rc1) and the loop is
  currently dispatch-failing for lack of eligible online devices. Autopilot
  Desktop is the new install but does not yet bundle/launch the node (§0 seam).
- **Payout leg — programmatic via the treasury wallet.** Correction (2026-06-14,
  owner): payouts are **not** blocked. The OpenAgents **treasury wallet** (the
  `/treasury` MDK-backed wallet) can make payouts, and **Artanis is already wired
  to pay out from it** under bounded spend authority (the nexus-treasury payout
  ledger). So the earn leg is **programmatic treasury payout under the run's spend
  cap** — the "hosted-MDK checkout payout adapter is off" note is about one
  specific adapter and does **not** mean payouts are operator-manual-only. The
  spend cap + receipt discipline still bind.
- **Settlement projection:** `settledPayoutSats` reads `0` even where receipts
  exist — settlement isn't joined into the public projection.
- **`models.tassadar_percepta_executor.v1` stays red** — we are launching the
  **run that trains it / earns contributors sats for contributing**, not claiming
  a trained model. "Help train Tassadar" = contribute verified work. Do not claim
  Tassadar is trained or CPU-equivalent.

---

## 4. Critical path (dependency-ordered) — drive all the way to a paid stranger

Each step: **owner lane** · **done-when** · **promise it moves**. A→D is the
spine; E makes it honest; F→G make it usable and public.

### A. Tassadar run authority + manifest · worker-api + product — ✅ DONE (#5006)
- Shipped + deployed + verified live: the **Tassadar run**
  `run.tassadar.executor.20260615` exists with a **run-level state-transition
  route** (`POST /api/training/runs/{ref}/(activate|seal|reconcile)`) that moves
  runs `planned → active → sealed → reconciled` without D1 patches, a public-safe
  **launch manifest** (runRef, promiseRef, state, admission rule, workload family
  `executor-trace`, verifier policy `exact_trace_replay`, payment mode, settlement
  state, spend cap, status URL, abort rule, blockers), and a run projection
  carrying `generatedAt` + a `live_at_read` staleness contract + typed blockers
  (including the planned-with-reconciled-windows caveat).
- **Done (verified):** `GET /api/training/runs/run.tassadar.executor.20260615`
  returns `state: active` + manifest + staleness contract.
- **Moves:** `training.monday_decentralized_training_launch.v1` stays **red** until
  D lands (no promise flipped green).
- **Next:** Step B below — make a non-owner contributor able to join and be
  dispatched real executor-trace work from this run.

### B. Self-serve executor-capability admission + claimable work · ✅ machinery DONE (#5007)
- Shipped + deployed: `POST /api/training/runs/{ref}/admit` makes a reasoned
  admit/exclude decision (receipted **executor-trace capability** + owner-operated
  check + the reasoned device-admission gates #4852, every branch with a stated
  measured reason), and the run carries a claimable **digest-pinned executor-trace
  work window** (`activeWindowCount: 1`) that an admitted contributor claims via
  `POST /api/training/leases/claim`.
- **Done when (machinery, met):** the run gates contributors with measured
  reasons and exposes claimable executor-trace work. **The live non-owner
  admit→claim run-through is the launch event** (§6) — `assignedContributorCount`
  stays 0 until a real contributor claims; not faked on the production run.
- **Next:** Step C below — wire the contributor's executor-trace closeout
  submission to the run-tied `exact_trace_replay` verification so verified work
  surfaces in the run's `verifiedWorkCount`.

### C. Exact-replay verification · ✅ machinery DONE (#5008)
- Shipped + deployed: `POST /api/training/runs/{ref}/executor-trace-closeout`
  turns a contributor's executor-trace closeout into a run+window-tied
  `exact_trace_replay` challenge (the builder was previously unwired), enforcing
  the distinct-validator-device rule. The submission is re-executed on a separate
  validator device; a `Verified` verdict surfaces in the run's `verifiedWorkCount`
  and a tampered digest in `rejectedWorkCount`.
- **Done when (machinery, met):** a contributor closeout creates a run-tied
  `exact_trace_replay` challenge. **The live closeout→`Verified` run-through is
  the launch event** (§6) — `verifiedWorkCount` stays 0 until a real submission
  is replayed; not faked.
- **Next:** Step D below — pay the contributor real sats for accepted work.

### D. Payout + settlement to the contributor · ✅ ledger + projection seam DONE (#5009)
**The "earn Bitcoin" leg.**
- ✅ **Ledger + projection seam is live** (#5009, deployed). Accepted
  (`Verified`) `exact_trace_replay` work →
  `POST /api/training/runs/{runRef}/settlement-receipt` records the
  operator-approved treasury payout chain (intent → attempt → reconciliation →
  `settlement_recorded` receipt) under the run manifest `spendCapSats` + a hard
  per-payout cap, and links the **public settlement receipt** onto the run.
  Payout is **programmatic** via the `/treasury` MDK wallet (Artanis dispatches
  under bounded spend authority). `providerConfirmedSettledPayoutSats` and the A1
  `settledPayoutSats` now read the linked settled receipts (no longer hardcoded
  `0`); `paid ≠ accepted ≠ credited ≠ settled` stays strict.
- **Remaining (the launch event, §6):** a real **non-owner stranger** holding a
  real settled Lightning payout from the Tassadar run with a dereferenceable
  public receipt — the live send is performed by the treasury container/Artanis
  and recorded by the #5009 route. This is the Go/No-Go event, not a separate
  build step.

### E. Settlement + corpus projection consistency · worker-api
- ✅ **`settledPayoutSats` is joined into the projection** (#5009): the run page
  and A1 leaderboard read real provider-confirmed settled receipts linked to the
  run, and a reconciled/accepted trace cannot leave the run claiming `planned`
  (existing `TrainingRunPlannedWithReconciledWindowsBlocker`); settled receipts
  surface only when dereferenceable + redacted.
- **Remaining (#5010 — W2 surface):** project the **accumulating verified-trace
  corpus** on the run page — the count of accepted, replay-verified
  `exact_trace_replay` **closed ticks** (RESEARCH_PLAN §5 W2.5: tick closure as
  acceptance — intent + execution + state delta + evaluation; "closed ticks _are_
  training records"). It must **rebuild on validation transitions, not
  registration** (RESEARCH_PLAN §6.3 + Standing Order 5; case law #4744–#4747)
  and stay bounded evidence — no Tassadar exactness / model-capability claim. It
  is **evidence toward** `artanis.tassadar_evolution_loop.v1`'s
  `tassadar_distillation_dataset_receipt_missing` blocker, not the dataset
  receipt itself.
- **Done when:** the public run shows a non-zero settled total equal to reality
  **and** a growing accepted verified-`exact_trace_replay` corpus count that
  rebuilds on verdict transitions.

### F. Autopilot Desktop install path contributors can use · pylon + desktop
- **The install is Autopilot Desktop** (signed + notarized macOS `.app`, OTA feed
  at `updates.openagents.com/desktop`; Linux path documented). Resolve the §0
  seam: a fresh install must **bundle and launch the node runtime** (or ship a
  one-command node bring-up) so a contributor does not have to separately stand up
  a Pylon node — today the app only *discovers* an existing one.
- Carry the **executor-trace lane** through the desktop's training cockpit, with
  install → node online → register → heartbeat → wallet-ready → assignment-ready
  smokes on macOS + Linux, plus failure modes (gap audit §5). Make sure the build
  the announcement links is the one the run admits and pays.
- **Done when:** a clean machine goes from "install Autopilot Desktop" to
  "admitted + wallet-ready + dispatched executor work" following only public
  instructions, with no separate Pylon-node setup step.

### G. Announce · product/forum
- After the Go/No-Go gate (§6), with the manifest/status URL, the live registry
  version, and exact promise IDs. See **§7 Copy gate**.

---

## 5. Scale ambition vs the "largest" claim

Bring the fleet online and admit as many contributors as the run can verify and
pay — that ambition is the whole point, and exact-replay work scales to weak
devices better than gradient work does. **But** the "largest decentralized
training run / beat 200 contributors" *claim* needs the participant-count rule
first (gap audit §4): count only **admitted contributors with accepted useful
work and public-safe receipt refs**, never raw registrations or stale heartbeats.
Define that rule in the manifest; make the comparison claim only once the count
clears it. Launching big is the goal; claiming "largest" without the count is the
one thing to hold.

---

## 6. Go / No-Go gate

Run before announcing that contributors are earning:

- [x] Tassadar run page: live state + manifest + staleness (A). ✅ #5006
- [ ] ≥1 **non-owner** Pylon admitted + dispatched executor work self-serve (B).
- [ ] Public `exact_trace_replay` `Verified` verdict for that contributor (C).
- [ ] That contributor holds a real settled-sats payout + public receipt (D).
      _Seam wired (#5009): the settlement-receipt route + run link are live;
      box flips when the real stranger settlement lands._
- [ ] Leaderboard/run `settledPayoutSats` non-zero + accepted-trace count growing
      (E). _`settledPayoutSats` read is wired (#5009); goes non-zero on the first
      real settlement. The verified-trace corpus (closed-tick) count surface is
      the §E remainder (#5010, W2)._
- [ ] The linked install path is reproducible on a clean machine (F).
- [ ] Copy passes §7, cites live registry version + promise IDs.

When all of A–E are real for a stranger, flip
`training.monday_decentralized_training_launch.v1` green **receipt-first** per
`proof.claim_upgrade_receipts.v1`. Until then the run is still **launched and
live** — you just describe the earn loop by what the receipts actually show
(§7), without claiming a payout nobody can dereference.

---

## 7. Copy gate (Tassadar wording)

Before any copy: query `/api/public/product-promises`, use the live version, cite
exact promise IDs.

**Say:** "the Tassadar run is live"; "install Autopilot (the desktop app) and
contribute executor-trace work to help train Tassadar"; "work is verified by
exact replay"; and — only with a real receipt — "the first contributors earned
Bitcoin, here's the receipt."

**Do NOT say** (until the matching promise is green or the copy is explicitly
caveated): "Tassadar is trained / outperforms a CPU / is a working model";
"largest decentralized training run" or "200+ contributors" (no count rule yet);
"earn Bitcoin from training today" as a blanket claim before a stranger has;
"stable / GA Autopilot Desktop"; any payout number that isn't a settled,
dereferenceable receipt. We are launching the **run that trains Tassadar**, not a
trained Tassadar.

---

## 8. Owner lanes & smokes

- **worker-api:** Tassadar run state-transition route + projection (A),
  exact-replay verdict on the live submission (C), payout leg (D),
  settlement+corpus projection consistency (E).
- **pylon + desktop:** Autopilot Desktop install that bundles/launches the node
  (§0 seam), self-serve executor-capability admission + dispatch (B), install
  path + smokes (F), contributor-side payout receive (D).
- **product/forum:** manifest + count rule (A, §5), Go/No-Go (§6), announcement
  (G), receipt-first promise flip.

Suggested checks (gap audit + Tassadar lane + desktop cockpit):

```sh
bun run --cwd apps/openagents.com/workers/api smoke:tassadar:executor-trace
bun run --cwd apps/openagents.com/workers/api smoke:training-runs:public
bun run verify:autopilot-desktop:training   # Autopilot Desktop training cockpit gate
```

New end-to-end **stranger smoke** (passing it *is* the launch): fresh non-owner
**Autopilot Desktop install → node online → declare executor capability** → admit
to the Tassadar run → dispatch a digest-pinned workload → exact-replay verify →
programmatic treasury payout (capped) → public receipt → run/leaderboard reflects
it + corpus count grows.

---

## 9. Risks & abort rules

- **Payout leg (D) is the gating risk** — the read/write seam linking accepted
  executor work to a run-referenced settlement receipt is the part that isn't
  wired yet. Payout itself is programmatic: the OpenAgents treasury wallet (the
  `/treasury` MDK-backed wallet) makes the payout and Artanis is already wired to
  pay out from it under bounded spend authority (the nexus-treasury payout
  ledger). Keep a hard per-payout cap; do not block the launch on large
  unattended dispatch.
- **Thin/rc1 fleet + the §0 install seam** — the loop is dispatch-failing for lack
  of eligible online devices; the launch's job is to bring devices online, so make
  Autopilot-install → node-online → admit frictionless (resolve the bundle/launch
  seam) and make sure rc-version nodes are actually admittable.
- **Owner Pylons are not strangers** — the DoD requires a non-owner.
- **A payout the recipient can't see is the projection-staleness bug wearing
  money** — if settlement lands somewhere undereferenceable, stop and fix the
  projection before announcing.
- **No secrets** in the manifest, receipts, tick ledger, Forum posts, or run
  projection: no prompts, host paths, provider payloads, invoices, preimages,
  payment hashes, mnemonics, or bearer tokens.
- **Don't leak the PoC into a model claim** — `compute.tassadar_executor_poc.v1`
  green proves replay of bounded workloads, not a trained Tassadar.

---

## 10. One-line status to repeat tomorrow

> The Tassadar run is live at `<status URL>`. Install Autopilot, contribute
> executor-trace work, it's verified by exact replay, and you get paid sats —
> first receipts at `<leaderboard URL>`, corpus growing toward Tassadar. (Word it
> to what §6 actually shows; never claim a payout without a dereferenceable
> receipt.)
