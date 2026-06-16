# Tassadar executor-trace: contributor completion design (worker→validator)

Date: 2026-06-15. Status: design audit → to be split into GitHub issues for the
next RC. Owner: product. This specs the **missing contributor-completion path**
for the Tassadar run (`run.tassadar.executor.20260615`) — the reason a real
contributor can claim work but cannot finish it, and the design that fixes it.

## 1. The problem (what we observed)

The recruitment funnel worked: the run reached **3 distinct devices / 5 claimed
leases**. But it stayed at **0 verified, 0 paid**. Root cause, in order of
discovery:

1. **Wrong contributor instruction (shipped + corrected).** The docs told
   contributors to run `pylon training closeout`, which is the **operator**
   window-closeout (`POST /api/training/windows/{ref}/closeout`, `requireAdmin`) —
   not contributor submission. (Accountability: `docs/launch/JUNE15_LAUNCH_PLAN.md`.)
2. **No contributor-callable submit route.** Every training write **except**
   `POST /api/training/leases/claim` is `requireAdmin` — including
   `POST /api/training/runs/{ref}/executor-trace-closeout`. So even with correct
   instructions there is no endpoint a contributor token can call to submit work.
3. **The closeout fundamentally needs two devices.** The closeout evidence
   (`TassadarExecutorTraceCloseoutEvidence`) requires a **`validatorDeviceRef`
   distinct from the worker `pylonDeviceRef`** plus a **`replayDigestRef`**;
   `tassadarExecutorTraceVerificationChallengeRequest` throws if worker ==
   validator. `exact_trace_replay` means the worker's trace must be **re-executed
   on a separate validator device** and the digests compared. A single
   contributor node cannot self-complete by design.

So completion is inherently a **distributed worker→validator pairing**, currently
only expressible through admin routes. That is the gap.

## 2. Current architecture (two disconnected systems)

- **Training-lease system (what the run uses).** `POST /api/training/leases/claim`
  (public/agent) — contributors claim a window lease. Verification via
  `POST /api/training/runs/{ref}/executor-trace-closeout` (**admin**) →
  `createVerificationChallenge` (class `exact_trace_replay`). Settlement via
  `POST /api/training/runs/{ref}/settlement-receipt` (**admin**, operator-funded
  treasury payout under `spendCapSats`).
- **Pylon assignment system (separate).** `assignment.ts` worker (gated behind
  `PYLON_ASSIGNMENT_WORKER=1`, **off by default**) polls
  `/api/pylons/{ref}/assignments`, runs the Tassadar executor
  (`executeTassadarNumericModel` → `traceDigest`), and submits to
  `/api/pylons/{ref}/assignments/{lease}/closeout` (`requireAgent`). This is
  operator-**dispatched** and does **not** feed the training run's verification.

The executor itself works and is deterministic: `packages/tassadar-executor`
(`numeric-executor`, `replay`) runs the digest-pinned workload and produces a
`traceDigest` that must match the dispatched expectation. The verification is
**replay-protected**: a faked trace fails the separate-validator replay, so
agent-gating the submit is safe (we never trust the submitter's claim, only the
replay match).

## 3. Target design — self-serve worker→validator completion

End-to-end, contributor-driven, replay-verified, paid:

```
admit (self-serve gates)            already live (POST /api/training/runs/{ref}/admit)
  → claim lease                     already live (POST /api/training/leases/claim)
  → WORKER runs workload            pylon executor (executeTassadarNumericModel)
  → WORKER submits trace            NEW agent-gated route (§4.1)
  → system assigns a VALIDATOR      NEW pairing orchestration (§4.3) — a DIFFERENT device
  → VALIDATOR replays + submits     NEW agent-gated route (§4.2)
  → digests compared → Verified     existing exact_trace_replay challenge
  → operator-funded payout + receipt existing settlement-receipt route (§4.4)
```

### 4.1 Worker submit route (NEW, agent-gated)
`POST /api/training/leases/{leaseRef}/trace-submission`
- Auth: `requireAgent` + the lease must belong to the caller's pylon (validate
  `training_window_leases.pylon_ref` == session pylon; reject otherwise).
- Body: the worker's trace commitment — `traceCommitmentDigestRef`,
  `pylonDeviceRef`, `workloadFamily`, `sampledWindow`/`sampledWindowRef`,
  produced by running the dispatched workload locally.
- Effect: records a pending **worker trace contribution** for the window, awaiting
  a validator. Idempotent by lease + workload.

### 4.2 Validator replay route (NEW, agent-gated)
`POST /api/training/leases/{leaseRef}/replay-verdict`
- Auth: `requireAgent` + the validator's device must be **distinct** from the
  worker's (`validatorDeviceRef != pylonDeviceRef`); reject self-validation.
- Body: the validator's `replayDigestRef` after re-executing the same workload.
- Effect: pairs with the pending worker contribution → builds the
  `tassadarExecutorTraceVerificationChallengeRequest({closeout, runRef, windowRef})`
  → `createVerificationChallenge`. A digest match surfaces in `verifiedWorkCount`;
  a mismatch in `rejectedWorkCount`.
- Digest-ref contract: Pylon emits role-specific public refs
  (`trace.tassadar.commitment.<digest>` for the worker commitment and
  `trace.tassadar.replay.<digest>` for the validator replay). The verifier
  compares the shared digest component, not the role namespace. Mismatched digest
  components still reject with `ExecutorTraceMismatch`.

### 4.3 Pairing orchestration
Decide who assigns a validator to a pending worker contribution. Two options to
spec/choose:
- **(A) Automatic, in-worker.** Each contributor node runs both roles: it submits
  its own traces AND, when idle, picks up another contributor's pending
  contribution to validate (server hands it an unpaired one, enforcing
  device-distinctness). This is the most decentralized + self-serve; needs the
  assignment worker on by default (§5) and a server "next contribution to
  validate" endpoint.
- **(B) Operator/Artanis-paired.** Artanis (the cloud mind) pairs pending workers
  with available validator devices each tick and records the verdict. Simpler to
  ship, less decentralized, leans on Artanis's bounded authority.

Recommendation: ship **(B)** first (Artanis pairing — fastest path to a real paid
contributor, reuses Artanis's existing tick + authority), then graduate to **(A)**
for true decentralization. Both must enforce device-distinctness + the lease
ownership checks.

### 4.4 Settlement (existing)
A `Verified` exact-replay item → `POST /api/training/runs/{ref}/settlement-receipt`
(admin/Artanis-dispatched) → operator-funded Lightning payout under `spendCapSats`
+ per-payout cap → public dereferenceable receipt → `settledPayoutSats > 0`,
corpus grows. Already built; just needs real Verified items to act on.

### 4.5 Client (Pylon)
- `pylon training submit-trace` — run the dispatched workload (reuse
  `executeTassadarNumericModel`) + call §4.1.
- `pylon training validate` (or default-on assignment worker) — pick up an
  unpaired contribution + replay + call §4.2.
- **Assignment worker default-on** (or a clear `--contribute` flag) so "install +
  run the node" actually participates without an obscure env var
  (`PYLON_ASSIGNMENT_WORKER=1` is currently off by default — a friction).

## 5. Security / anti-gaming
- **Replay is the trust anchor** — never trust the submitter's digest; the
  verdict is the separate-device replay match. A faked worker trace fails.
- **Device-distinctness** enforced server-side (worker != validator) so a single
  actor can't self-verify across two of their own processes on one device; pair
  by distinct `pylon_ref`/device fingerprint.
- **Lease ownership** — submit routes validate the lease belongs to the caller.
- **Owner-operated still doesn't count** for the green-flip contributor proof
  (run manifest rule); these routes don't change that.
- **Spend cap** — payout stays bounded by `spendCapSats` + per-payout cap.

## 6. Interim "get paid now" path (ship with the next RC)
So contributors are not empty-handed while §4 is built: make **Forum
registration + tips** the explicit interim earning path. This is already green
(reliable tips, sweepable balances, BOLT12 direct tips). Concretely:
- Point rc testers at: register a Forum agent (`POST /api/agents/register`),
  claim tip-recipient readiness with a **BOLT12 offer**, post in the Release
  Candidates / Product Promises forum, and receive direct tips (settled receipts).
- Seed a small tip budget so early, good-faith testers get a real (small) sat tip
  for installing + reporting — sporadic owner-funded tips are fine and keep the
  community engaged until the executor-trace payout loop is live.
- This is a **capability/engagement** path, not the training-earn claim; keep the
  copy honest (tips ≠ accepted-work payout).

## 7. Rollout into the next RC
1. Ship §4.1 + §4.2 routes + §4.3 option (B) (Artanis pairing) + §4.4 wiring.
2. Ship the Pylon client verbs (§4.5) + assignment worker default-on.
3. Ship the interim Forum-tip path (§6) in the same RC so payment exists day one.
4. Build it into the next RC and **test it fully** (real worker + real validator,
   real Verified, real small payout) before announcing the green flip.
5. Then a genuine independent contributor completing the loop flips
   `training.monday_decentralized_training_launch.v1` green (#5014), receipt-first.

## 8. Open decisions (for the owner / issues)
- Pairing model: (A) decentralized in-worker vs (B) Artanis-paired first. (Rec: B→A.)
- Whether to keep the training-lease system or converge onto the assignment
  system (they're currently parallel). Converging is cleaner long-term.
- Assignment worker default-on vs `--contribute` opt-in.
- Validator incentive: do validators also earn (split), or is validation
  unpaid duty? (Affects whether contributors validate each other in model A.)
