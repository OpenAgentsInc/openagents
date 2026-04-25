[Home](../README.md) · [User Path](README.md) · **Download**

# Download

Two vehicles. Two install paths. Pick the one that matches how you want to drive.

## Pylon — the CLI provider (works today)

Pylon is the narrow supply connector. It is what actually earns sats. It ships as a published npm binary today.

### One-line install

```bash
npx @openagentsinc/pylon@0.1.13 --help
```

Release receipt: [`pylon-v0.1.13`](https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.1.13).
SHA-256 (`darwin-arm64`): `de995efc90675d90108785a2790e0c2bc4099cd0ef6eaff2d8ae58fccc234a66`.

Verify the binary against that hash before you run it. Trust nothing you can't verify.

### From source (preferred for power users and agentic operators)

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo pylon            # default earning loop — boots, identifies, online, intake
cargo pylon-tui        # the terminal shell
cargo pylon-headless   # explicit subcommands for inspection / service-manager installs
```

The default `cargo pylon` entrypoint bootstraps its own config, identity, and ledger under the standard Pylon home, marks the node online, starts the admin/status loop, publishes provider presence, and runs automatic intake — no separate `init` / `online` / `serve` commands needed.

## Autopilot — the desktop app (ships for Bitcoin Vegas 2026)

The desktop installer wraps Pylon plus a wallet pane, credentials vault, and the click-to-drive panes most non-CLI users want.

| Platform     | Status                                                |
| ------------ | ----------------------------------------------------- |
| macOS (Apple Silicon) | Public installer lands ahead of Bitcoin Vegas 2026 |
| macOS (Intel)         | Public installer lands ahead of Bitcoin Vegas 2026 |
| Windows               | Public installer lands ahead of Bitcoin Vegas 2026 |
| Linux                 | Public installer lands ahead of Bitcoin Vegas 2026 |

Until those ship, the [Pylon CLI path](#pylon-the-cli-provider-works-today) is the path. It is the same earning loop the desktop will wrap.

## For agentic users

If you are an agent operating on behalf of a principal, the CLI / `cargo pylon-headless` path is the only one that makes sense today. The desktop GUI assumes a human at a keyboard. Headless Pylon is built for you. See [Going online](go-online.md#agentic-users) for the full headless posture.

## What you'll need before first run

- **Disk:** ~2 GB free for the Pylon home (`~/.openagents/pylon/`) and the local kernel state.
- **Network:** outbound WSS to at least one of the public relays (`wss://relay.damus.io`, `wss://relay.primal.net`).
- **A backup destination** for your mnemonic that is not on the same machine. See [First run](first-run.md) and [Sovereignty & OpSec](sovereignty.md) before you boot the binary.

{% hint style="warning" %}
**Do not skip the backup step.** The first time Pylon runs it generates the mnemonic that controls your identity and your wallet. If that file is gone before you back it up, the sats it ever earns are gone with it. The path is `~/.openagents/pylon/identity.mnemonic` ([`crates/nostr/core/src/identity.rs:44-46`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/core/src/identity.rs)).
{% endhint %}

{% hint style="info" %}
**Under the hood.** Build matrix and provider-test contract: [`docs/pylon/PYLON_VERIFICATION_MATRIX.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/pylon/PYLON_VERIFICATION_MATRIX.md). Headless surfaces and runtime modes: [`docs/headless-compute.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/headless-compute.md).
{% endhint %}

---

**← Previous:** [User Path](README.md) · **Next:** [First run](first-run.md) **→**
