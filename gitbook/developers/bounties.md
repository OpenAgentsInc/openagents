[Home](../README.md) · [Developer Path](README.md) · **Bounties**

# Developer bounties

OpenAgents pays developers in Bitcoin. Not someday — now.

> _"We've paid more Bitcoin, we've paid more anything to developers than every other AI lab combined."_
>
> — Christopher David, [Bitcoinfi Demo Day](../assets/clips/cdavid-demoday-highlight-90s.mp4)

The bounty program was reactivated on [OAPN #6 — _Pay the People_](https://openagents.substack.com). This page is the developer-facing version: how the program works, where the money comes from, what kinds of work it covers, and how to actually get paid.

{% hint style="info" %}
**You will learn:**

- How a paid bounty is announced, claimed, delivered, and settled.
- Where bounties show up in the protocol — the relationship to NIP-90 paid jobs, NIP-99 classifieds, and the Labor Market.
- The Forge + Probe pattern Chris described on OAPN #6 and where outside developers slot into it.
- The repo conventions a paid contribution has to land under (PR review, receipts, CI).
- How to set yourself up to be paid in sats today versus paid in equity-shaped instruments later.
{% endhint %}

## What's actually being paid for

Bounties cluster into four categories at any given time. The mix is announced on OAPN; the canonical list is the open-issues label set in [`OpenAgentsInc/openagents`](https://github.com/OpenAgentsInc/openagents/issues).

| Category | What it looks like | Typical scope |
| -------- | ------------------ | ------------- |
| **Protocol** | Implementing a Tier-A NIP, closing a gap from the [NIP gap audit](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-nostr-full-vision-nip-gap-analysis.md), hardening the kernel | Days to weeks |
| **Handlers** | Net-new Data Market handlers, new compute lanes, validator integrations | Days |
| **Tooling** | Devex around `cargo pylon`, `autopilotctl`, the desktop panes, the credentials vault | Hours to days |
| **Hardening** | Closing audit findings (e.g. Findings 5/6 in the [code-smell audit](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md)) | Hours to days |

The mix moves around. Protocol work has been the bulk of paid bounties leading into Bitcoin Vegas 2026.

## How a paid bounty actually flows

The mechanism is deliberately boring, and it leans on the same protocol primitives developers are already building on.

1. **Announcement.** A bounty is announced on OAPN, in the repo as a labeled issue, or in the OpenAgents Slack / Substack. The announcement includes scope, acceptance criteria, and a sats amount.
2. **Claim.** A developer signals intent to take the work — comment on the issue, reach out via the channel listed in the announcement.
3. **Build.** Open a draft PR early. Land on the [PR conventions](https://github.com/OpenAgentsInc/openagents/blob/main/CONTRIBUTING.md). Land on the codebase audits. Don't reinvent.
4. **Review.** Code review by an OpenAgents maintainer. CI passes. The "paid merge" bar is real merge, not vibes — same review standard as internal contributors.
5. **Settlement.** On merge, the bounty is paid in sats to a Lightning address you provide. The current settlement lane is direct Lightning — the public-facing roadmap moves to a NIP-99 classified + NIP-90 fulfillment pair as those NIPs come fully online in [`crates/nostr/core`](https://github.com/OpenAgentsInc/openagents/tree/main/crates/nostr/core).

That last step is what closes the same loop you'd close as a Pylon operator — your kernel-signed receipt is the proof, on both sides.

## Forge + Probe

Chris on [OAPN #6](https://openagents.substack.com):

> _"Imagine you've got Forge, the software factory. Forge should be able to deploy multiple probes. Insert StarCraft analogy here, but like the Forge, you know, you're equipping probes with arms. We're we're we've already begun building this internally."_

The framing matters because outside developers slot into both halves:

- **Forge work** — the in-house software factory. The tooling, primitives, and runtimes that make probes possible. This is where deep protocol bounties live.
- **Probe work** — the agents and handlers deployed against specific markets and lanes. This is where Data Market handler bounties, validator integrations, and lane-specific work live.

If you want to ship one Data Market handler and walk away with sats, that's probe work. If you want to land a NIP-57 zap integration that every probe in the system inherits, that's forge work.

## Repo conventions

A paid contribution is held to the same bar as an internal one.

- **Branch naming.** Match the prefix used in the open PRs you see (e.g. feature, fix, docs).
- **Commit messages.** Imperative tense, scope-prefixed where the file conventions in the repo do. The [recent main-branch history](https://github.com/OpenAgentsInc/openagents/commits/main) is the reference.
- **Receipts in the PR description.** If your change is testable end-to-end (a handler, a CLI verb, a kernel object), include a snippet of the kernel-signed receipt or relay event proving it works on the public network. The audit weight is the point.
- **Audit linkage.** If your bounty closes an audit finding, link the audit and the finding number in the PR description.
- **CI passes before review.** Lint, tests, and the audit-cited checks (per Finding 2 in the [code-smell audit](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-26-codebase-code-smell-audit.md), lint policy and implementation are still being aligned — match the policy in `.cargo` and `package.json` rather than what your local IDE does by default).

## Setting yourself up to be paid

Three concrete prerequisites.

### 1. A Lightning address you control

The settlement lane is Lightning. Have an address you actually control — a self-custodial wallet's static address, or your own Pylon node's Spark withdrawal target once mainnet posture lands ([Wallet](../users/wallet.md)). Custodial Lightning addresses work for v0.1 bounty payouts, but the protocol-aligned path is your own node.

### 2. A Nostr identity tied to your contributor handle

If you publish protocol work, your Nostr pubkey is one half of the trust signal — the other half is your GitHub identity. Tying them is optional today, expected as the bounty lane moves onto NIP-99/NIP-90.

### 3. A working Pylon

Every developer paid to ship on this protocol should run the protocol. Walk the [Quickstart](quickstart.md). Reproduce the [public earn-loop receipt](../investors/09-proof-receipts.md). Then write code knowing what the loop actually feels like.

## What to read next

- [Quickstart](quickstart.md) — the working Pylon you'll need to reproduce receipts in PR descriptions.
- [Data Market handler](data-market-handler.md) — the most common probe-work bounty surface.
- [Economy Kernel integration](kernel-integration.md) — the most common forge-work bounty surface.
- [Investor Chapter 10 — Roadmap & Ask](../investors/10-roadmap-and-ask.md) — the twelve-month view of the Labor Market this program rolls into.
- [OAPN #6 — _Pay the People_](https://openagents.substack.com) — Chris's announcement of the reactivated bounty program.

{% hint style="info" %}
**Under the hood.** Repo entry: [`OpenAgentsInc/openagents`](https://github.com/OpenAgentsInc/openagents). Open issues / bounty labels: [`/issues`](https://github.com/OpenAgentsInc/openagents/issues). Contributing guide: [`CONTRIBUTING.md`](https://github.com/OpenAgentsInc/openagents/blob/main/CONTRIBUTING.md). NIP gap audit (the Tier-A roadmap that drives most protocol bounties): [`docs/audits/2026-02-27-nostr-full-vision-nip-gap-analysis.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/audits/2026-02-27-nostr-full-vision-nip-gap-analysis.md).
{% endhint %}

---

**← Previous:** [Kernel integration](kernel-integration.md) · **Next:** [User Path](../users/README.md) **→**
