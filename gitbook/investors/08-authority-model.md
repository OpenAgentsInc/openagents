[Home](../README.md) · [Investor Path](README.md) · **08. Audit-Grade Trust**

# 8. Audit-Grade Trust

> _"The app must never 'feel like it paid you' unless it actually did. The architecture exists to enforce that honesty."_
>
> — [`docs/MVP.md`, OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md)

**You will learn:**

- Why the receipts can't lie even if the app does
- The non-negotiable invariant that protects the user's money
- What "self-hosting" really means here

## The honesty invariant

Authority — the power to change economic truth — is kept out of the app on the user's machine. Money movement, settlement finalization, verdict closure — all of it happens on the backend, on services we run (or that someone else runs in our place).

That sounds bureaucratic. It's not. It's the single non-negotiable invariant of the OpenAgents stack.

Why? Because a desktop client is not trustworthy. Devices get compromised. UIs lag. Code has bugs. If the client could change money, then a glitchy display _would_ be a stolen sat. By moving authority to a separate service, we make sure that **a broken client can never create a real loss**. The wallet number can be late. The wallet number cannot be wrong.

## What this means for an investor

Three things follow:

1. **The audit trail lives in the receipts**, not in UI state. Every settled payout in the public reports folder is a kernel-signed receipt — not a screenshot, not a database export. A diligence partner reads receipts.

2. **The client is untrusted by design.** A malicious or broken Autopilot install cannot create monetary gain. Only the kernel can authorize payment, and the kernel demands authenticated command lanes. There is no "go around" path.

3. **Self-hosting is real.** The desktop can be pointed at a user-owned backend with their own relay set. OpenAgents does not sit in the middle of authorized work. We are the buyer of first resort _on_ the open network — we are not the network.

## A concrete example

The first earning proof, dated 2026-04-23 (full receipt in [Chapter 9](09-proof-receipts.md)), exposed the architecture working as designed.

Several minutes _after_ the kernel had already confirmed and settled the 25-sat payout, the local desktop projection still showed the run as _"dispatched / pending."_ The backend was already done. The UI was catching up.

If the desktop had been the authority, that delay would have been a confused user staring at a wallet that "hadn't paid yet." Instead, the wallet balance was correct. The kernel knew. The receipt was already signed. The user-facing status said _"Homework paid"_ because that's what the receipts said.

The proof writes this up by name as proof that the architecture works:

> _"That is an observation about the worker-local projection lag, not a payout failure. The user-facing status was already 'Homework paid', and the wallet balance/history confirmed receipt."_

UI lag is fine. Authority lag would be a bug. We've designed for the first and ruled out the second.

## What runs where

Not everything has to be authoritative. The system uses a different layer for each job:

- **Money, settlement, wallet truth, policy verdicts** — backend authority. Always. Never the client.
- **Provider presence, "is the device online", cursor continuity** — distributed sync, fast and good. Wrong here is cheap.
- **Receipts, audit trails** — signed by the kernel, written to the public repo, verifiable by anyone.

That separation is _why_ we can ship a desktop wedge that handles real Bitcoin without lying to its users. The UI can be wrong; the receipts are right.

## The default backend, today

OpenAgents hosts the default backend. It's open-source. It's self-hostable. Anyone can run their own. Today it ships:

- The starter-job dispatcher (the source of the 25-sat CS336 paid-training jobs)
- The token issuance and session flow
- The primary Nostr relay/index path
- Public stats reporting

A user or organization that wants to operate fully on their own — own backend, own relays, own underwriter — can do that today. The starter-job demand stays with whoever underwrites it. That's the part we run for now. Everything else is configurable.

This matters for investor diligence in a specific way: it means OpenAgents is not the chokepoint. We are providing the buyer of first resort and a reference deployment. The network can outlive any specific operator, including us.

## Why this is the moat, not the feature

The companies that win the AI infrastructure layer will win on trust. Every other lane competes on speed or features; trust competes on architecture.

Most "decentralized AI" projects shipped a feature. They have a token, a marketplace, a dashboard. What they don't have is an architecture where the wallet number matches a signed receipt _the user can verify themselves, against a public repo, on a different machine._

That's what audit-grade trust means here. The receipts are the architecture. The architecture is the company.

---

{% hint style="info" %}
**Under the hood.** The full domain-scoped authority matrix, ADR-0001's Spacetime exception class, the OWNERSHIP map of which crate owns what, and the device-bound attestation roadmap are all in the [Developer Path → Economy Kernel integration](../developers/kernel-integration.md) and [`docs/adr/0001-authority-boundaries.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/0001-authority-boundaries.md).
{% endhint %}

---

**← Previous:** [07. Economy Kernel](07-economy-kernel.md) · **Next:** [09. Proof Receipts](09-proof-receipts.md) **→**
