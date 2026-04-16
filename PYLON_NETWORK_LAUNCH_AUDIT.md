# Pylon Network Launch Audit

Date: 2026-04-16
Audited branch tip: `0fab22919` on `pylon-stable-0.1.1`

## Scope

This audit answers the launch questions directly:

1. Does standalone `Pylon` actually work end to end right now?
2. Does the relay-based Pylon network path actually exist?
3. Can one run be distributed coherently across a fleet of Pylons?
4. Will operators automatically get the updated `Pylon` binary from the network?

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

## What Does Not Work Or Is Not Honest To Claim

- Standalone `Pylon` does not currently have hosted-Nexus starter-demand parity
  with `Autopilot Desktop`.
- Standalone `Pylon` does not currently have hosted fleet scheduling.
- Standalone `Pylon` does not currently coordinate distributed training or
  multi-node shard execution across a fleet.
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
