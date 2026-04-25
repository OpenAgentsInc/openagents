[Home](../README.md) · [User Path](README.md) · **Sovereignty & OpSec**

# Sovereignty & OpSec

This is the chapter that determines whether the rest of the pathway means anything. The protocols are sovereign by design — Nostr can't lock you out, Lightning can't reverse your sats, the kernel runs locally — but the disciplines have to be yours.

If your keys leak, the protocol can't save you. If your credentials sprawl, the protocol can't save you. This page is the discipline.

## The sovereignty goal

The goal is precise and worth stating out loud:

> You should be able to walk away from OpenAgents at any time, with your identity intact, your wallet intact, and your reputation portable, and the network should keep working without you.

Every recommendation on this page is in service of that goal.

## Key custody — the layered model

<figure><img src="../assets/graphics/key-custody-tiers.svg" alt="Three custody tiers ordered by blast radius"><figcaption>Spend custody effort proportional to the damage a leak would cause. Tier 1 deserves a metal seed plate; Tier 3 deserves a rotation schedule.</figcaption></figure>

There are three keys (well, three roles for one key) that you control. Order them by blast radius.

| Tier | Material                                  | Blast radius if lost                                                  |
| ---- | ----------------------------------------- | --------------------------------------------------------------------- |
| 1    | The mnemonic at `~/.openagents/pylon/identity.mnemonic` | Total. Identity, wallet, history, reputation. Game over. |
| 2    | API keys in the credentials vault (Codex, Spark gateway, Blink, custom) | Whatever the upstream provider lets the key do — drain a remote balance, exfiltrate prompts, etc. |
| 3    | Session tokens, ephemeral relay subscriptions | Limited. Cycle them and move on. |

Custody discipline is just "spend more effort on Tier 1 than on Tier 3."

### Tier 1 — the mnemonic

- **Back it up before the wallet is ever funded.** [First run](first-run.md) covers the mechanics. Metal plate beats paper. Paper beats cloud. Cloud you don't control beats nothing.
- **Never paste it into a chat, an agent prompt, or a screen-shareable buffer.** If an agent asks for it, the agent is wrong.
- **Treat the file mode as load-bearing:** `chmod 600`, owner-only. If a backup process can read it, that backup process is in your trust boundary.
- **Rotate by migrating, not by editing.** If you need a new identity (compromise, separation of concerns), generate a fresh mnemonic in a fresh home directory and migrate balances out via Lightning. Do not try to edit the existing file in place.

### Tier 2 — credentials

The credentials vault ([`docs/CREDENTIALS.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/CREDENTIALS.md)) stores values in the OS keychain (service `com.openagents.autopilot.credentials`) with non-secret metadata in `~/.openagents/autopilot-credentials-v1.conf`.

Three disciplines:

1. **Scope narrowly.** Each entry has a scope bitset — `CODEX`, `SPARK`, `SKILLS`, `GLOBAL`. Codex gets `CODEX | SKILLS | GLOBAL`, Spark gets `SPARK | GLOBAL`. Don't put everything in `GLOBAL`. A leaked `GLOBAL` key leaks to every runtime.
2. **Prefer keychain over env fallback.** The resolver prefers keychain when both are populated, but env vars persist in shell history, dotfiles, and process listings. Use the vault.
3. **Disable, don't delete, when sunsetting a key.** The `enabled` flag turns a credential off without losing the metadata that tells you what it was scoped to. Useful when you're rotating keys and want to confirm nothing is still requesting the old one.

### Tier 3 — sessions

Cycle them. Don't be sentimental.

## Credential scopes — the runtime mapping

When the desktop syncs credentials into runtime environments, the routing rule is exact ([`docs/CREDENTIALS.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/CREDENTIALS.md)):

| Runtime | Receives entries scoped to       |
| ------- | -------------------------------- |
| Codex   | `CODEX` or `SKILLS` or `GLOBAL`  |
| Spark   | `SPARK` or `GLOBAL`              |

If you want a key visible to Codex but not Spark, scope it `CODEX`. If you want it everywhere, scope `GLOBAL`. If you find yourself reaching for `GLOBAL` reflexively, stop and ask whether you actually need it.

## Self-hosted vs remote inference — a sovereignty axis

<figure><img src="../assets/graphics/inference-posture-matrix.svg" alt="Custody matrix comparing self-hosted vs remote inference"><figcaption>Same sats earned either way. The difference is who else sees the work.</figcaption></figure>

The choice of where the model runs is not just a cost question. It's a custody question.

| Posture        | Custody of weights | Custody of prompt | Custody of output | Sats earned |
| -------------- | ------------------ | ----------------- | ----------------- | ----------- |
| **Self-hosted** | You                | You               | You               | Same        |
| **Remote**     | Provider           | Provider sees it  | Provider sees it  | Same        |

Both posture earn the same sats. The difference is who else sees the work.

If you are running paid jobs that touch sensitive prompts (your own data, a client's data, training corpora you are obliged to protect) — self-host. If you are running commodity inference where the prompt is throwaway and you just need horsepower — remote is fine.

The credentials vault is what enforces the posture. No remote API key in scope means no remote inference path.

## Agentic identity isolation

If you are running multiple agents (or running your own identity alongside an agent's), isolate their homes:

```bash
HOME=/path/to/agent-a cargo pylon-headless ...
HOME=/path/to/agent-b cargo pylon-headless ...
```

This is the same pattern the bilateral earn-loop runbook uses ([`docs/autopilot-earn/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot-earn/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md)). Each `HOME` gets its own `~/.openagents/pylon/identity.mnemonic`, its own keychain entries, its own ledger.

Why this matters:

- **Blast radius.** A compromised agent only loses its own balance, not yours.
- **Reputation hygiene.** Each agent builds (and can lose) reputation under its own pubkey, without dragging your identity with it.
- **Audit clarity.** The receipts the kernel mints are scoped to a pubkey. Mixed homes mean mixed receipts.

## OpSec disciplines (the short list)

These are non-negotiable if real sats are involved.

1. **Verify binaries before you run them.** The pylon-v0.1.13 release receipt and SHA-256 are published; check them.
2. **Run on a machine you control.** Not a shared workstation, not a borrowed laptop. The mnemonic is on disk and the wallet is hot.
3. **Outbound-only firewall posture.** Pylon needs WSS out to relays and Lightning out to peers. Nothing should need inbound to your provider node.
4. **Update path is git/cargo, not curl-pipe-bash.** Pull the repo, verify the tag signature when one is available, build.
5. **Backups are read-only after creation.** Once your seed is on a metal plate, it is not getting edited. Rotate identities, don't rotate seeds in place.
6. **Don't run a buyer node and a provider node from the same identity.** Use two homes, two mnemonics. The bilateral-loop runbook is the template.
7. **Audit your own receipts.** The kernel mints `RevocationReceipt` and `DeliveryBundle` events for everything. Read them periodically. They are your own audit trail and they exist for exactly this reason.
8. **Honest scope right now (v0.1):** the renderer at [`apps/autopilot-deprecated/src/render.rs:329, 350`](https://github.com/OpenAgentsInc/openagents/blob/main/apps/autopilot-deprecated/src/render.rs) prints full secret material by default — Finding 5 in [`docs/audits/2026-02-26-codebase-code-smell-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md). Until that lands hardened, do not screen-share Pylon log streams. Treat the terminal as secret material.

## What sovereignty buys you

A user who does all of the above:

- Can leave OpenAgents tomorrow and take their identity, wallet, and history with them.
- Cannot be locked out by us, by a relay, or by an upstream provider going dark.
- Has a clean audit trail of every paid job and every revocation, signed by their own key.
- Is the sole party that can authorize a withdrawal of their sats.

That is the entire point.

{% hint style="info" %}
**Under the hood.** Identity authority: [`crates/nostr/core/src/identity.rs`](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/core/src/identity.rs). Credential resolution rules: [`docs/CREDENTIALS.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/CREDENTIALS.md). Code-smell audit (Findings 5 and 6, the renderer + Spark network bugs): [`docs/audits/2026-02-26-codebase-code-smell-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md). Broader hardening audit: [`docs/audits/2026-02-27-full-system-hardening-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-full-system-hardening-audit.md). Architecture audit (path-drift findings): [`docs/audits/2026-02-28-full-codebase-architecture-audit.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-28-full-codebase-architecture-audit.md).
{% endhint %}

---

**← Previous:** [First run](first-run.md) · **Next:** [Go online](go-online.md) **→**
