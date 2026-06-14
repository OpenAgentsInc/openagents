# JUNE 15 LAUNCH PLAN — "Training Run Starts / Users Earn Bitcoin for Contributing"

Date authored: 2026-06-14 (Sunday). Target: Monday 2026-06-15.

This is the critical-path ops plan for the **one thing that has to be true
tomorrow**: a public training run is live, real (non-owner) people can install
Pylon and contribute useful work, and they get **paid real Bitcoin** for it,
with public receipts. Everything else — coding-agent earning, the labor market,
the Tassadar model story, the "largest run" comparison — is **bonus** and must
not sit on this critical path.

> Grounding: this plan is built from
> `docs/2026-06-12-episode-236-training-launch-gap-audit.md` (the canonical gap
> list), the live surfaces below, and the product-promise registry
> (`/api/public/product-promises`, currently `2026-06-14.4`). Read the gap audit
> before executing — this plan turns its "What Still Needs To Be Built" section
> into a sequenced, gated launch.

---

## 1. Definition of Done (the only scoreboard that matters)

The launch is real — not just copy — when **all** of these are true and publicly
verifiable:

1. **A run is RUNNING.** A public run page reports a live state (not `planned`),
   with a stable `trainingRunRef`, a published manifest, and a fresh
   `generatedAt`/staleness contract.
2. **A non-owner contributor joined self-serve.** Someone who is not the owner
   installed Pylon v0.3, was admitted to the run, claimed a lease, and submitted
   work — without an operator hand-staging it.
3. **The work was verified.** The submitted training window passed a named
   verification class (deterministic_recompute / freivalds_merkle), with a
   public verdict ref.
4. **They earned real sats, and can see it.** Accepted work settled a real
   Lightning payout to the contributor's wallet, with a public settlement
   receipt, AND the public run page / leaderboard shows
   `settledPayoutSats > 0` (today it shows `0` everywhere).

If #2 or #4 can't be true for a stranger by tomorrow, see **§6 Launch tiers** —
we launch honestly at the tier we can actually back, and we do **not** say
"earn Bitcoin from training today" until a stranger has.

---

## 2. Current reality (honest snapshot, 2026-06-14)

What's **green / proven**:

- Bounded **two-device real-gradient run** `run.cs336.a1.real_gradient.demo`:
  2 assigned contributors, 2 reconciled windows, 3 verified work items, public
  loss curve, two 30-sat settled receipts (operator-staged).
  (`pylon.first_real_model_training_run.v1` = yellow.)
- **Verification classes** live and exercised on real dispatched work
  (exact_trace_replay, deterministic_recompute, freivalds_merkle); a weak-device
  validator was paid for a Freivalds recheck. (`training.verification_classes.v1`
  = yellow.)
- **Device-capability dataset** with paid benchmark closeouts (yellow).
- **Reliable tips ladder** settles real sats to registered offers, never fails
  (green). **Tassadar executor PoC** settled real Lightning (green).
- **Join-lifecycle + device-admission + staleness-priced acceptance contracts**
  landed on main (#4848–#4854) — the rails for admitting and pricing
  contributors.
- Pylon **v0.3.0-rc2** runs the agent surface from the device (green
  `pylon.v03_agent_economy.v1`); operator-staged **install-to-bitcoin** smoke
  passed end to end with a real 21-sat payout (yellow
  `pylon.install_without_wallet_knowledge.v1`).

What's **red / the gap** (this is the critical path):

- The live run row still reports **`state: planned`** even though windows are
  reconciled. No run **state-transition route / lifecycle**.
- Leaderboard `settledPayoutSats: 0` — **settlement isn't joined into the public
  projection**.
- **No self-serve contributor path** — every real run to date was
  operator-staged; the A1 evidence used owner-operated Pylons.
- **No Monday run + manifest** — no `trainingRunRef` for tomorrow, no public
  status URL, no admission/payout policy.
- **Hosted-MDK programmatic payouts are disabled on the production account** —
  so the "earn" payout leg currently needs an operator-approved small-sats path,
  not unattended dispatch.
- Fleet is thin: ~4 Pylons online / 51 registered, **mostly 0.3.0-rc1**; no
  stable 0.3.0 publish.

Net: **the pieces exist and have each been proven in isolation; nothing yet
stitches them into one public run a stranger can join and earn from.** That
stitch is the entire job for tomorrow.

---

## 3. Critical path (dependency-ordered)

Each step has an **owner lane**, a **done-when**, and the **promise it moves**.
Steps A→D are the spine; E→F make it safe and real; G is the announce.

### A. Run authority + Monday manifest  ·  owner: worker-api + product
Create the run and make it announce itself.
- Build/finish the **run state-transition route + projection** so a run moves
  `planned → active → sealed → reconciled → closed` without D1 patches
  (gap audit §1). Public-safe fields: runRef, promiseRef, state, admission rule,
  window/lease refs, workload ref, verifier policy, artifact/digest refs,
  payment mode, settlement state, `generatedAt` + staleness, typed blockers.
- Publish the **Monday Launch Manifest** (gap audit §3): `trainingRunRef`,
  objective, model/rung scope, dataset scope, max participants + admission
  policy, minimum useful work, validator policy, **payout policy + spend cap**,
  live public status URL, abort/stale rules, affected promise IDs. Public-safe
  (no secrets/paths/wallet material).
- **Done when:** `GET /api/training/runs/<monday-run>` returns a non-`planned`
  state with the manifest fields and a staleness contract.
- **Moves:** unblocks `training.monday_decentralized_training_launch.v1` and
  `training.public_distributed_training_run.v1` (still red until D lands).

### B. Self-serve admission + assignment  ·  owner: pylon + worker-api
Let a stranger's Pylon actually join.
- A fresh contributor Pylon **registers capability**, the run **admits** it via
  the reasoned device-admission gates (#4852, every admit/exclude carries a
  measured reason), it **claims a lease** and receives a training window/shard.
- Use the landed join-lifecycle ladder + staleness-priced acceptance
  (sync_reentry routing, not bare rejection) (#4848–#4853).
- **Done when:** a Pylon **not operated by the owner** appears as an admitted,
  assigned contributor in the run projection.
- **Moves:** the "contributes" half of the core promise. (Gap audit §6.)

### C. Verified work  ·  owner: worker-api
- The submitted window is verified by a named class
  (deterministic_recompute and/or freivalds_merkle), producing a public verdict
  ref. This rail already works on real dispatched work — confirm it fires on the
  stranger's submission.
- **Done when:** a public `training.verification.challenge.<id>` `Verified`
  verdict references the stranger's window.

### D. Payment + settlement to the contributor  ·  owner: worker-api + pylon
**This is the "earn Bitcoin" leg — the highest-risk item.**
- Accepted work → payout to the contributor's wallet over the **reliable-tips
  ladder / MDK bridge** (the path that already settles real sats), with a
  **public settlement receipt**.
- **Production constraint:** hosted-MDK programmatic payouts are disabled on the
  prod account. So tomorrow's payout leg is almost certainly **operator-approved
  small-sats** (strict spend cap), not unattended dispatch. That is fine for the
  DoD — a stranger earning 21 sats with a real receipt beats a perfect
  unattended system that pays no one.
- **Done when:** a stranger contributor holds a real settled Lightning payout
  from this run with a dereferenceable public receipt, and the payout is **not**
  collapsed with "accepted" or "credited" state (keep paid ≠ accepted ≠ settled
  distinct).
- **Moves:** the "earn bitcoin" half. With A–D done for one stranger, the core
  experience is real.

### E. Settlement projection consistency  ·  owner: worker-api
- Join settlement receipts into the **run page + leaderboard** so
  `settledPayoutSats` reflects reality (today it's `0` while receipts exist).
  Add the test from the gap audit: a reconciled window cannot leave the run
  claiming `planned`, and settled receipts surface only when refs are
  dereferenceable + redacted (gap audit §2).
- **Done when:** leaderboard/run `settledPayoutSats` is non-zero and equals the
  real settled total; no projection shows a payout a recipient can't see.

### F. Pylon v0.3 install path contributors can actually use  ·  owner: pylon
- Either a **stable 0.3.0** publish OR a **documented, smoke-passed source/
  install path** with register → heartbeat → wallet-ready → assignment-ready
  smokes on macOS + Linux, plus documented failure modes (gap audit §5).
- The fleet is mostly rc1 — make sure the install the announcement points to is
  the one the run actually admits and pays.
- **Done when:** a clean machine can go from "install" to "admitted +
  wallet-ready" following only public instructions.

### G. Announce (copy gate)  ·  owner: product/forum
- Only after the **Go/No-Go gate (§5)** passes. Use the live registry version,
  cite exact promise IDs, link the manifest/status URL. See **§7 Copy gate**.

---

## 4. What is explicitly OUT (bonus / do not block on)

Per the gap audit and the owner's direction, **do not** put these on tomorrow's
critical path:

- **Coding-agent / labor-market earning** (#4777/#4781/#4782/#4783 Lane C) —
  bonus. The labor market is green for its own first job, but it is **not**
  required for a training run to start and pay contributors.
- **Tassadar/Percepta trained-model claims** — the executor PoC is green;
  a *trained* model is not, and is not needed to run CS336-style training and
  pay people. Keep `models.tassadar_percepta_executor.v1` red.
- **"Largest run" / "200 contributors"** — no participant-count methodology, no
  comparable evidence, thin fleet. **Do not claim it.** (Gap audit §4 defines
  the count rule to build *before* any such claim — admitted contributors with
  accepted work and receipt refs only.)
- **Pylon multi-earning node** — each mode needs its own receipts; don't claim
  one install earns five ways tomorrow.
- **W3 student sweep (#4749)** — research, not launch runtime.

---

## 5. Go / No-Go gate (run this before announcing)

Announce the **full** "training run is live and paying contributors" message
only if every line is checked:

- [ ] Run page returns a non-`planned` live state + manifest + staleness
      contract (A).
- [ ] ≥1 **non-owner** Pylon admitted + assigned self-serve (B).
- [ ] That contributor's work has a public `Verified` verdict (C).
- [ ] That contributor holds a **real settled sats payout** with a public
      receipt (D).
- [ ] Leaderboard/run `settledPayoutSats` reflects it, non-zero (E).
- [ ] The install path the announcement links is reproducible on a clean
      machine (F).
- [ ] Copy passes §7 and cites the live registry version + promise IDs.

If any line fails, drop to the matching tier in §6. **Do not** upgrade
`training.monday_decentralized_training_launch.v1` to green until all of A–E are
real for a stranger; that flip is receipt-first per
`proof.claim_upgrade_receipts.v1`.

---

## 6. Launch tiers (launch honestly at the tier you can back)

- **Tier 1 — Full (all of §5 green):** "The run is live. Install Pylon, join,
  and earn Bitcoin for verified training work — here are the first contributors'
  receipts." Flip the Monday-launch promise green, receipt-first.
- **Tier 2 — Live run, earning switching on (A–C + at least one operator-
  approved small-sats payout to a non-owner, D partial):** "The run is live and
  the first contributors are being paid in small sats — here's a receipt;
  payouts are scaling this week." Promise stays **yellow/red** with the explicit
  caveat; do not say "earn Bitcoin today" as a general claim.
- **Tier 3 — Run live, no stranger payout yet:** "The run is live — install
  Pylon and contribute; payouts are coming online this week." Keep the promise
  **red**. Do **not** say anyone earned Bitcoin from training. This is still a
  real, honest launch of the run; it just doesn't claim the payout until it's
  true.

Tier 2 is the realistic target given the prod-MDK-payouts-disabled constraint.
Tier 1 is the stretch. Tier 3 is the floor and is still a legitimate launch.

---

## 7. Copy gate (what you may / may not say)

**Before any announcement copy:** query `/api/public/product-promises`, use the
live version, cite exact promise IDs.

**May say** (Tier-appropriate): "a public training run is live"; "install Pylon
v0.3 and contribute"; "verified training work"; and — only with a real receipt —
"the first contributors earned Bitcoin, here's the receipt."

**May NOT say** until the matching promise is green or the copy is explicitly
caveated: "largest decentralized training run"; "200+ contributors"; "earn
Bitcoin from training today" (as a general claim before a stranger has);
"stable Pylon v0.3"; "Tassadar model is trained / CPU-equivalent"; any payout
number that isn't a settled, dereferenceable receipt.

---

## 8. Owner lanes & suggested smokes

- **worker-api:** run state-transition route + projection (A), settlement
  projection consistency (E), payout leg wiring (D). Tests: a reconciled window
  cannot leave the run `planned`; settled receipts surface only when
  dereferenceable + redacted.
- **pylon:** self-serve admission/lease (B), install path + smokes (F), payout
  receive on the contributor side (D).
- **product/forum:** manifest (A), count methodology decision (out-of-scope
  claim guard, §4), Go/No-Go (§5), announcement (G), receipt-first promise flip.

Suggested focused checks (from the gap audit):

```sh
bun run --cwd apps/openagents.com/workers/api smoke:cs336-a1:no-spend
bun run --cwd apps/openagents.com/workers/api smoke:training-runs:public
bun run --cwd apps/openagents.com/workers/api smoke:tassadar:executor-trace
```

Plus a new end-to-end **stranger smoke**: fresh non-owner Pylon → admit → lease
→ submit → verify → operator-approved small-sats payout → public receipt →
leaderboard reflects it. Passing this smoke *is* Tier 1.

---

## 9. Risks & abort rules

- **Payout leg (D) is the gating risk.** Prod programmatic MDK payouts are off;
  plan the operator-approved small-sats path with a hard spend cap, and treat
  any unattended-payout idea as out of scope for tomorrow.
- **Owner-operated Pylons are not a stranger.** Do not present an owner Pylon as
  independent contributor proof — the DoD requires a non-owner.
- **Stale/empty projections read as "covered."** If the run can't show live
  state, ship Tier 3 honestly rather than faking `active`.
- **Abort:** if a settlement lands somewhere a recipient can't see, stop and fix
  the projection before announcing — a payout the recipient can't dereference is
  the projection-staleness bug wearing money.
- **No secrets** in the manifest, receipts, Forum posts, or this run's public
  projection: no prompts, host paths, provider payloads, invoices, preimages,
  payment hashes, mnemonics, or bearer tokens.

---

## 10. One-line status to repeat tomorrow

> The run is live at `<status URL>`. Install Pylon v0.3, join, do verified
> training work, get paid sats — first receipts at `<leaderboard URL>`. (Tier
> the wording to what §5 actually shows; never claim a payout without a
> dereferenceable receipt.)
