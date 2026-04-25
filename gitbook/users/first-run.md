[Home](../README.md) · [User Path](README.md) · **First run**

# First run

The first time Pylon (or Autopilot) launches, it does one thing that matters more than everything else combined: it generates a mnemonic. That single twelve-or-twenty-four-word phrase controls your Nostr identity AND your Lightning wallet. Both. The same seed.

Treat that fact as load-bearing. Everything else on this page exists to keep you from losing it.

## The one file that matters

```
~/.openagents/pylon/identity.mnemonic
```

That path is the canonical user-identity authority on the system, defined in [`crates/nostr/core/src/identity.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/core/src/identity.rs#L44-L46). The Spark wallet derives from the same file ([`crates/spark/src/spark_wallet.rs:380`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/spark_wallet.rs)).

One mnemonic. Two roles:

- **Nostr identity** — the keypair that signs every event Pylon publishes (capability ads, job results, access contracts, revocation receipts).
- **Lightning wallet** — the Spark wallet keys that receive your sats and authorize withdrawals.

If you back up `identity.mnemonic`, you have backed up your identity AND your wallet. If you lose it, both are gone.

{% hint style="warning" %}
**Path drift warning.** You may see references in older code or settings to `~/.openagents/nostr/identity.json`. That path is **not** the authoritative identity store — it appears as drift in `SettingsDocumentV1`'s default in `crates/autopilot-desktop/.../app_state.rs:1372` and is flagged in [`docs/audits/2026-02-28-full-codebase-architecture-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-28-full-codebase-architecture-audit.md). Anchor your backups to `~/.openagents/pylon/identity.mnemonic` and only that path.
{% endhint %}

## The first-run checklist

Do these in order. Do not skip step 2.

### 1. Boot the binary once, then stop it

```bash
cargo pylon          # or: npx @openagentsinc/pylon@0.1.13
# Ctrl-C after you see the identity-bootstrap log line
```

This generates the mnemonic and writes it to disk. You only need to do this once.

### 2. Back the mnemonic up — before you do anything else

```bash
cat ~/.openagents/pylon/identity.mnemonic
```

Copy the words. Write them on paper. Store them somewhere that is not on this machine and is not in cloud storage you don't control. A metal seed plate is appropriate here.

This file controls real money the moment your wallet has a balance. Treat it like the seed phrase to a hardware wallet, because that is exactly what it is.

### 3. Verify the file permissions

```bash
ls -la ~/.openagents/pylon/identity.mnemonic
# expect: 600 (rw owner only)
chmod 600 ~/.openagents/pylon/identity.mnemonic   # if it isn't
```

### 4. Set up your credentials vault (optional, for remote inference)

If you plan to route any work to remote model providers, this is where their API keys go. Credentials are stored in the OS keychain under service `com.openagents.autopilot.credentials`, with non-secret metadata in `~/.openagents/autopilot-credentials-v1.conf`. Spec: [`docs/CREDENTIALS.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/CREDENTIALS.md).

Built-in template slots include `OPENAI_API_KEY`, `OPENAGENTS_SPARK_API_KEY`, `BLINK_API_KEY`. You can add custom entries; names are normalized to uppercase and must match `[A-Z_][A-Z0-9_]*`.

Each credential has a **scope** — `CODEX`, `SPARK`, `SKILLS`, or `GLOBAL` — that controls which runtimes receive it. Scope your keys narrowly. A `BLINK_API_KEY` does not need to be visible to the Spark runtime.

### 5. Decide your inference posture

You are about to choose where the model that does your paid work actually runs. See [Going online](go-online.md) for the self-hosted vs remote tradeoff. You can switch later, but pick a default now.

## For agentic users

If an agent is doing the first run on behalf of a principal, three things change:

1. **Use an isolated home directory.** Set `OPENAGENTS_HOME` (or run with `HOME=/path/to/agent-home`) so the agent's `~/.openagents/pylon/identity.mnemonic` does not collide with a human user's. The bilateral-loop runbook ([`docs/autopilot-earn/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot-earn/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md)) is the canonical example of running two independent identities side-by-side.
2. **Mnemonic custody is your principal's problem, not yours.** The agent generates the mnemonic, but the principal owns the recovery posture. The agent should hand the mnemonic up to the principal's secret store on first boot and then forget it.
3. **Scope credentials to the agent's mandate.** An agent that only sells compute does not need `OPENAI_API_KEY` in its vault. Less surface area is less liability.

The full agentic posture is in [Sovereignty & OpSec](sovereignty.md).

{% hint style="info" %}
**Under the hood.** Identity construction lives in [`crates/nostr/core/src/identity.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/core/src/identity.rs). Wallet derivation: [`crates/spark/src/spark_wallet.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/spark/src/spark_wallet.rs). Credential storage and resolution rules: [`docs/CREDENTIALS.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/CREDENTIALS.md).
{% endhint %}

---

**← Previous:** [Download](download.md) · **Next:** [Sovereignty & OpSec](sovereignty.md) **→**
