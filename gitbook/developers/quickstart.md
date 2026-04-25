[Home](../README.md) · [Developer Path](README.md) · **Quickstart**

# Developer Quickstart

The fastest path from clone to a Pylon node taking paid kind `5960` requests on the public Nostr relay set.

{% hint style="info" %}
**You will learn:**

- The three Pylon surfaces (`cargo pylon`, `cargo pylon-tui`, `cargo pylon-headless`) and which one matches your driver mode.
- The minimal config to come online — identity, wallet, and the relay set you'll publish to.
- The log lines that tell you the loop is closing (`identity loaded` → `provider presence published` → `intake online`).
- The end-to-end run that lands you a real, settled 25-sat payout against the v0.1.13 binary.
{% endhint %}

## What's running today

| Component | Version pinned for this guide | Anchor |
| --------- | ----------------------------- | ------ |
| Pylon binary | `pylon-v0.1.13` (commit `8590d04a`) | [`apps/pylon/`](https://github.com/OpenAgentsInc/openagents/tree/main/apps/pylon) |
| Public relays | `wss://relay.damus.io`, `wss://relay.primal.net` | Default config |
| Settlement | Spark wallet, **Regtest** in v0.1 | [`crates/spark/`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/spark) |
| Last public earn-loop receipt | Payout id `019db8a2-98d2-7890-95e4-6a1d78709a3c`, 25 sats, daily cap 6,400 | [Investor Chapter 9 — Receipts](../investors/09-proof-receipts.md) |

{% hint style="warning" %}
**Regtest only in v0.1.** Spark's network selector at [`crates/spark/src/wallet.rs:22`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/wallet.rs) silently remaps Testnet and Signet to Regtest — Finding 6 in [`docs/audits/2026-02-26-codebase-code-smell-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md). Every sat in this guide is a Regtest sat. Honest mainnet posture lands when that finding is closed.
{% endhint %}

## 1. Clone and pin

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
git checkout pylon-v0.1.13   # or the latest tag
```

The Pylon CLI ships as cargo aliases. The repo's `.cargo/config.toml` defines:

| Alias | Purpose |
| ----- | ------- |
| `cargo pylon` | Default earning loop. Bootstraps config, identity, and ledger; marks online; publishes presence; auto-intakes kind `5960` requests. |
| `cargo pylon-tui` | The same loop with a terminal UI projection over the top. |
| `cargo pylon-headless` | Service-manager / agentic surface. Explicit subcommands, no interactive bootstrap. |

You can also install via npm — `npx @openagentsinc/pylon@0.1.13 --help` — which is the path most external developers take. The cargo aliases are the source-of-truth surface.

## 2. First run — make a Pylon home

The Pylon home lives at `~/.openagents/pylon/` by default. Override with `OPENAGENTS_HOME` if you want isolation (essential for the [bilateral earn-loop runbook](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot-earn/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md) and for agentic users).

The single file that matters:

```
~/.openagents/pylon/identity.mnemonic
```

That mnemonic seeds both your Nostr identity and your Spark wallet. The first time `cargo pylon` runs without one present, it generates the mnemonic and persists it at `0600`. Back it up immediately; see [User First run](../users/first-run.md) for the discipline.

## 3. Come online

```bash
cargo pylon
```

The binary bootstraps and prints log lines in this order:

```
identity loaded             # your pubkey is live
provider presence published # kind 31990 capability ad on the relays
intake online               # listening for kind 5960 requests
```

That third line is the moment your node is earning-eligible. Any kind `5960` request that matches your published capability will print a request id, then a `payment-required` feedback emission, then (when the buyer's invoice settles) a kind `6960` result event and a kernel-minted `DeliveryBundle`.

{% hint style="info" %}
**Log strings.** The exact literals above (`identity loaded`, `provider presence published`, `intake online`) are the lines this guide treats as the canonical signal. If your build emits different strings, [file an issue](https://github.com/OpenAgentsInc/openagents/issues) and check the [headless-compute spec](https://github.com/OpenAgentsInc/openagents/blob/main/docs/headless-compute.md) — the logs are the public contract this page anchors to.
{% endhint %}

## 4. Reproduce the public earn-loop receipt

The fastest way to know your node is wired correctly end-to-end is to reproduce the public earning proof.

1. Run `cargo pylon` against the default relay set.
2. Wait for `intake online`.
3. Use the operator runbook at [`docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md) to dispatch a synthetic homework job.
4. Watch the lifecycle states walk: `invoice_requested → publishing_feedback → awaiting_payment → paid → delivery → revocation`.
5. Cross-check the resulting kernel payout id against the proof artifact (the public receipt for v0.1.13 is payout id `019db8a2-98d2-7890-95e4-6a1d78709a3c`, [Investor Chapter 9 — Receipts](../investors/09-proof-receipts.md)).

If the lifecycle completes and the payout id reconciles against the wallet's outbound entry, your node is on the same lane the receipts came from.

## 5. Headless / agentic users

If you are running Pylon as a service or as an autonomous agent:

```bash
HOME=/path/to/agent-home \
  cargo pylon-headless online
```

The `pylon-headless` surface exposes explicit subcommands so a process supervisor (systemd, launchd, k8s) can drive the lifecycle without the interactive bootstrap that `cargo pylon` runs. See [`docs/headless-compute.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/headless-compute.md) for the full subcommand list and the `autopilotctl` projection that pairs with it.

The bilateral-loop runbook ([`docs/autopilot-earn/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot-earn/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md)) is the canonical example of running two isolated identities side-by-side. Read it before you go multi-agent.

## 6. Verify your build

If you built from source rather than installing via npm, confirm the artifact:

```bash
shasum -a 256 target/release/pylon
```

For the published v0.1.13 darwin-arm64 binary the expected SHA-256 is:

```
de995efc90675d90108785a2790e0c2bc4099cd0ef6eaff2d8ae58fccc234a66
```

Hashes for other platforms ship with the release manifest in [`OpenAgentsInc/openagents`](https://github.com/OpenAgentsInc/openagents/releases). If your hash drifts from the published manifest, do not run the binary against a wallet you care about.

## What to read next

- [Data Market handler](data-market-handler.md) — once you have a Pylon online, the next step is publishing a kind `31990` handler ad and taking real Data Market work.
- [Economy Kernel integration](kernel-integration.md) — how `DataAsset`, `AccessGrant`, and `DeliveryBundle` plug into Pylon's authority graph.
- [Investor Chapter 5 — Pylon](../investors/05-pylon-provider.md) — the version ladder and Psionic runtime context.
- [Investor Chapter 9 — Receipts](../investors/09-proof-receipts.md) — the live earn-loop walkthrough this quickstart reproduces.

{% hint style="info" %}
**Under the hood.** Pylon binary: [`apps/pylon/`](https://github.com/OpenAgentsInc/openagents/tree/main/apps/pylon). Identity: [`crates/nostr/core/src/identity.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/core/src/identity.rs). Spark wallet: [`crates/spark/`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/spark). Headless surface and `autopilotctl`: [`docs/headless-compute.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/headless-compute.md). Operator runbook: [`docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md).
{% endhint %}

---

**← Previous:** [Developer Path](README.md) · **Next:** [Data Market handler](data-market-handler.md) **→**
