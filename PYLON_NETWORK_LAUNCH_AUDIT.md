# Pylon Network Launch Audit

Date: 2026-04-16
Audited branch: `pylon-stable-0.1.1`

## Scope

This audit answers the launch questions directly:

1. Does standalone `Pylon` actually work end to end right now?
2. Does the relay-based Pylon network path actually exist?
3. Can one run be distributed coherently across a fleet of Pylons?
4. Will operators automatically get the updated `Pylon` binary from the network?
5. How do the public promises in Transcript 222 map to what was actually built?

This audit is based on current repo code, current docs, and the local release
verification lane that was rerun on the `pylon-stable-0.1.1` branch tip.

It is not a claim that a live public multi-node internet fleet was soaked today.
The strongest verified claim in this audit is: the local end-to-end standalone
release lane is green, and the checked-in relay/job code paths are real.

## Direct Answers

### Does standalone `Pylon` actually work right now?

Yes, in a narrow and honest sense.

The current standalone release works as a local provider connector for one
retained lane:

- `psionic.local.inference.gemma.single_node`

The install/bootstrap path works, the standalone CLI/TUI work, the local
wallet/NIP-90 flow works in the checked-in verification lane, and the strict
lint/test gates now pass.

### Does the Pylon network path actually exist?

Yes, but it is a relay-based NIP-90 path, not a hosted fleet scheduler.

Current standalone `Pylon` can:

- publish one retained `kind:5050` text-generation handler announcement
- submit buyer requests to configured relays
- target one provider explicitly with `--provider <pubkey>`
- broadcast a request to relays without `--provider`
- scan relays for matching provider work
- publish `payment-required` and `processing` feedback
- publish retained results after local completion
- persist buyer/provider/payment/settlement state in the local ledger

That is real networked work over relays.

### Can one run be distributed coherently across a fleet of Pylons?

No, not in the sense most people mean by fleet distribution.

What exists today:

- targeted dispatch to one provider pubkey
- relay broadcast that may be picked up by more than one eligible provider

What does not exist today:

- hosted-Nexus single-assignee leasing
- fair scheduling across all online Pylons
- distributed shard assignment
- "send one job and have the network coordinate all Pylons for me"

If you want work to hit multiple specific Pylons, the current honest path is to
target each provider explicitly. Broadcast exists, but broadcast is not a fleet
controller.

### Will operators automatically get the updated `Pylon` binary from the network?

No.

There is no in-network binary rollout, no Nexus-driven upgrade push, and no
automatic fleet-wide self-update protocol in the current standalone release.

The honest update story is launcher-based:

- the npm/bun bootstrap package checks GitHub for the latest tagged
  `pylon-v...` release on each default run
- if the operator relaunches through that bootstrap path, it can pick up the
  newer tagged release
- if the operator is running an old extracted binary directly, nothing in the
  current network path upgrades it automatically

Current code does report `client_version` to Nexus during provider presence
heartbeat, but this audit did not find a standalone rollout controller, minimum
version enforcement path, or binary distribution path built on top of that.

## Episode 222 Roadmap And Issue Program Status

The broader Episode 222 gap was not left as hand-wavy launch copy. It was
turned into a concrete launch-hardening program in `workspace/docs/plan222.md`.

The key roadmap phases were:

- Phase 7: Transcript 222 truth contract
- Phase 8: zero-touch node automation
- Phase 10: real heterogeneous work delivery
- Phase 11: public-state and payout truth
- Phase 12: scale rehearsal and claim readiness

That roadmap was then tracked under `workspace#12`, which is closed in the
retained launch-truth receipt at
`workspace/docs/reports/transcript-222/20260412-172432-launch-truth-receipt.json`.

The closed blocker set includes:

- claim-contract and artifact-contract work: `openagents#4295` through
  `openagents#4299`
- automatic Pylon assignment, materialization, launch, and receipt upload:
  `openagents#4300` through `openagents#4304`
- weak-device and capability-tier launch contract: `openagents#4305` through
  `openagents#4307`, plus `psionic#932` and `psionic#933`
- payout and public-truth work: `openagents#4308` through `openagents#4313`,
  `openagents.com#10`, and `openagents.com#11`
- rollout and operational hardening: `openagents#4314` through
  `openagents#4317`, plus `workspace#13`
- canary and crowd-threshold rehearsals: `openagents#4318` and
  `openagents#4319`
- status, matrix, claim-sheet, audit-template, and FAQ follow-ons:
  `workspace#14` through `workspace#18`, plus `openagents#4320`

The sibling workspace repo also now says overall Transcript 222 launch status
is `GREEN`.

## Why That Does Not Make This Stable Branch The Full Episode 222 Stack

That `GREEN` status is about the broader retained training-launch package
across `openagents`, `openagents.com`, and `psionic`.

This audit is narrower. It is about the current standalone stable Pylon branch
that you asked about for launch.

On this `pylon-stable-0.1.1` branch, the branch-local evidence is much
narrower:

- the only Transcript 222 plan doc present locally is
  `docs/plans/transcript-222-launch-truth-contract.md`
- this branch does not carry the broader workspace launch-status bundle,
  crowd-rehearsal reports, weak-device FAQ, or launch-SLO docs that the
  workspace `GREEN` status points at
- current standalone docs and code in this branch are centered on a relay-based
  NIP-90 buyer/provider flow for the retained local Gemma lane

So the correct combined reading is:

- the Episode 222 launch-hardening program was closed as a broader training
  launch package
- this stable standalone Pylon release is still a narrower product slice
- you should not collapse those two truths into one claim

## Transcript 222 Promise Status

The important historical point is that the follow-on work split into two
different programs:

- one program made standalone `Pylon` into a real NIP-90 and wallet-aware node
- one program narrowed and formalized public launch claims around Transcript 222

Those are not the same thing.

### What The Post-Transcript Roadmap Actually Closed

The April 5 standalone Pylon issue-program audit said the new `apps/pylon`
still lacked:

- relay runtime
- NIP-89 announcement surface
- provider-side NIP-90 intake
- buyer-side NIP-90 workflow
- wallet management surface
- persisted local NIP-90 ledger

That gap is now materially closed.

Current standalone `Pylon` does have:

- relay configuration and NIP-42 handling
- provider announce / scan / run paths
- buyer submit / watch / approve / deny paths
- wallet and payout command surfaces
- persisted relay, job, wallet, and settlement ledger state

So if the question is "did we close the gap between the old promise of a local
node and the thin standalone shell we had on April 5," the answer is mostly
yes.

### What The Claim-Contract Layer Actually Did

The Transcript 222 launch-truth contract did not claim that all the
training-network promises were now implemented inside this branch.

The broader workspace hardening program did more than this. It also tracked and
closed a larger retained training-launch package. But the contract document
itself had a narrower job.

What it actually did was freeze the allowed claim boundary and make the public
stats contract stricter. The key move was:

- stop inferring training truth from presence and payout counters
- explicitly forbid stronger public statements until stronger fields exist

That means some Episode 222 promise gaps were closed by narrowing the language,
not by proving the originally implied system.

### Promise-By-Promise Reading

`The Pylon network has N nodes online and sats paid out`

- Partly real.
- The online-node and payout counters are real public fields in Nexus.
- The follow-on truth contract says those fields are presence and payout truth
  only, not proof of training assignment, accepted work, or model progress.
- The contract also says the payout counter must not be described as
  accepted-work-only payout until payout classes are split explicitly.

`Your Pylon should auto-update anyway`

- True only on the existing launcher/bootstrap path.
- The broader Episode 222 hardening docs treat this as satisfied by the
  existing bootstrap release path, not by a new in-band update protocol.
- What exists now is launcher-based update resolution from GitHub Releases.
- What does not exist is Nexus-driven auto-update, peer-to-peer update
  distribution, or in-place fleet self-update.

`We are about to send you real work, real pieces of a decentralized training run`

- Not substantiated by the current standalone Pylon implementation.
- What exists today is a relay-based NIP-90 compute-market flow for the
  retained local Gemma inference lane.
- The current standalone release does not show training-shard assignment,
  training-window admission, accepted training contribution, or model-progress
  closeout through the Pylon network.

`Weak devices can get a slice of the work`

- Defined at the claim-contract level, not proven as a current public runtime
  truth surface.
- The launch-truth contract says `validation_replay` is the default weak-device
  lane for hardening purposes until a later issue changes that.
- The same contract also says the public stats path does not yet expose weak-
  device assigned or accepted contributor counts.
- Current standalone Pylon docs and code do not present a released weak-device
  training-work lane to operators.

`This is or will imminently be the largest decentralized training run in the world`

- Not an honest current claim.
- The launch-truth contract explicitly forbids unqualified "largest run"
  language.
- It says any future "largest" claim must name the count family:
  online, assigned, accepted, or model-progress contributors.
- The contract also says the public stats surface still lacks the fields needed
  for assigned-contributor, accepted-contributor, and model-progress-contributor
  claims.

`The stats page will show what work and math is happening on your computer`

- Only partly true today.
- Current public stats do expose recent Pylon presence and recent Pylon
  diagnostic summaries.
- The Transcript 222 contract says the public stats path still does not expose
  full training assignment, accepted-work, or model-progress truth.

## Transcript 222 Bottom Line

If the question is "did we build everything Episode 222 implied," the answer is
no.

If the question is "did we build the narrower standalone Pylon node that the
later issue program asked for," the answer is mostly yes.

If the question is "did the launch-hardening work make the public claims more
honest," the answer is yes. But it did that mainly by tightening definitions
and forbidding overclaims, not by proving that the public distributed-training
story was already live in this standalone stable branch by itself.

## What Works

- Standalone install/bootstrap works through `@openagentsinc/pylon`.
- Standalone source/release binary paths work.
- `pylon init`, `status`, `inventory`, `online`, `pause`, `resume`, `offline`,
  `jobs`, `earnings`, `receipts`, `activity`, `wallet`, `payout`, `announce`,
  `provider scan`, `provider run`, `job submit`, `job watch`, `job approve`,
  and `job deny` are real checked-in surfaces.
- Buyer requests can be published over relays.
- Provider nodes can see matching requests and execute them locally.
- Payment-required flow is implemented and verified in the local test lane.
- Paid request settlement and post-payment execution are implemented and
  verified in the local test lane.
- The standalone release documentation is now aligned with the actual narrow
  network truth.
- `cargo clippy -p pylon --all-targets -- -D warnings` passes.

## What This Standalone Stable Branch Does Not Work Or Is Not Honest To Claim

- Standalone `Pylon` does not currently have hosted-Nexus starter-demand parity
  with `Autopilot Desktop`.
- Standalone `Pylon` does not currently have hosted fleet scheduling.
- Standalone `Pylon` does not currently coordinate distributed training or
  multi-node shard execution across a fleet.
- Standalone `Pylon` does not currently publish the Transcript 222 training
  truth surfaces needed for admitted, assigned, accepted, or model-progress
  contributor claims.
- Standalone `Pylon` does not currently publish payout-class splits needed to
  separate placeholder sats from accepted-work sats.
- Standalone `Pylon` does not currently expose a released weak-device training
  lane with public assigned/accepted accounting.
- Standalone `Pylon` does not currently auto-update peers over the network.
- This audit does not prove a live public multi-node production fleet is
  currently online and doing real customer work together.

## Nexus Truth

The hosted Nexus relation is still asymmetric.

Standalone `Pylon` does talk to Nexus for relay/default-network participation
and provider presence reporting.

Standalone `Pylon` is not yet a first-class hosted Nexus client in the same way
`Autopilot Desktop` is.

The strongest concrete evidence is the starter-demand proof path in
`apps/nexus-control/src/lib.rs`, which currently requires an
`autopilot-desktop...` session identity before hosted starter-demand is
considered valid. That means:

- hosted starter-demand is real
- but it is still effectively `Autopilot Desktop`-gated
- standalone `Pylon` should not currently be sold as receiving hosted starter
  demand automatically from Nexus

## Update Distribution Truth

The update story is package/bootstrap distribution, not network distribution.

If you want operators to have the updated `Pylon`, the current honest path is:

1. publish the tagged stable release asset
2. make operators launch through `npx @openagentsinc/pylon`, `bunx`, or the
   global npm/bun package path
3. have them relaunch so the bootstrap checks the latest tagged release

That can keep operators current enough for launch.

That is not the same thing as:

- in-band peer-to-peer auto-update
- Nexus pushing binaries to Pylons
- online Pylons automatically upgrading in place

## Public Truth Surface Status On This Stable Branch

Within this stable standalone branch, the current public-facing Nexus truth
surface does expose:

- `pylonsOnlineNow`
- `pylonsSeen24h`
- `pylonSessionsOnlineNow`
- `sellablePylonsOnlineNow`
- `recentPylons[*]`
- `recentPylonDiagnostics[*]`
- `nexusPayoutSatsPaidTotal`

The Transcript 222 launch-truth contract in this branch explicitly says the
public stats contract still does not expose:

- admitted contributor count
- assigned contributor count
- accepted contributor count
- model-progress contributor count
- weak-device assigned contributor count
- weak-device accepted contributor count
- accepted-work sats total
- placeholder sats total
- beta-bonus sats total
- active run id or run family
- active window id or window family

That is the strongest repo-local evidence that the Episode 222 distributed-
training claim family is still narrower in implementation than it was in public
implication.

## Verified Evidence

These checks were rerun successfully on the current branch tip for this audit:

```bash
cargo clippy -p pylon --all-targets -- -D warnings
scripts/pylon/verify_standalone.sh
scripts/pylon/verify_nip90_wallet.sh
cargo test -p pylon -- --nocapture
bun test packages/pylon-bootstrap/test/bootstrap.test.js
```

The bootstrap package README and tests also confirm the current update behavior:

- default launcher path checks GitHub for the latest tagged `pylon-v...` release
- explicit `--version` resolves a specific tagged release
- cached installs are upgraded when a newer tagged release exists

## Code And Doc Facts This Audit Relies On

- `docs/pylon/README.md`
- `docs/pylon/PYLON_VERIFICATION_MATRIX.md`
- `docs/transcripts/222.md`
- `docs/plans/transcript-222-launch-truth-contract.md`
- `docs/plans/compute-market-launch-truth-checklist.md`
- `docs/audits/2026-03-15-decentralized-training-target-sequencing-audit.md`
- `docs/audits/2026-04-05-pylon-nip90-wallet-issue-program-audit.md`
- `workspace/docs/plan222.md`
- `workspace/docs/2026-04-12-transcript-222-launch-status.md`
- `workspace/docs/2026-04-12-transcript-222-vision-implementation-audit.md`
- `workspace/docs/reports/transcript-222/20260412-172432-launch-truth-receipt.json`
- `packages/pylon-bootstrap/README.md`
- `apps/pylon/src/nip90_runtime.rs`
- `apps/pylon/src/lib.rs`
- `apps/nexus-control/src/lib.rs`
- `NEXUS_PYLON_AUDIT.md`

The specific current network truths are:

- buyer submit records an explicit provider target only when `service_providers`
  is populated
- provider intake drops jobs targeted at some other pubkey
- docs explicitly state that no standalone hosted-Nexus single-assignee/fair
  scheduling claim is being made
- Nexus starter-demand proof still requires an `autopilot-desktop` session
- the Transcript 222 launch-truth contract explicitly narrows public stats and
  payout counters away from stronger training-run claims
- the sibling workspace repo marks the broader Transcript 222 hardening package
  `GREEN`, but that broader package is not the same thing as this standalone
  stable branch alone

## Launch Recommendation

If you launch now, the honest claim is:

`Pylon` is a standalone provider connector with a working relay-based NIP-90
buyer/provider path for the retained local Gemma lane, plus a working
launcher-based stable release distribution path.

Do not claim:

- automatic work distribution across all Pylons
- hosted standalone Nexus starter-demand parity
- distributed training across a public Pylon fleet
- automatic in-network peer upgrade rollout

## Bottom Line

If your question is:

- "Can I launch a narrow standalone Pylon release that actually works?"  
  Yes.

- "Can I send relay-based work to standalone Pylons?"  
  Yes.

- "Can I rely on Nexus to distribute one run coherently across a fleet of
  Pylons?"  
  No.

- "Can I rely on the network itself to make everyone run the updated Pylon?"  
  No.

- "Can I get operators onto the updated Pylon if they use the launcher path?"  
  Yes. That is the real update path today.
