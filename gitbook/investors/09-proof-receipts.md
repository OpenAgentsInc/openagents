[Home](../README.md) · [Investor Path](README.md) · **09. Proof Receipts**

# 9. Proof Receipts

> _"Founders can say anything. Code, version tags, and signed payout receipts can only say what actually happened."_

**You will learn:**

- The two artifacts that answer "can you ship" and "does the money move"
- How to read each receipt
- How a third party can re-run the same proof on their own machine

## Why we lead with receipts

Founders can say anything. Code, version tags, and signed payout receipts can only say what actually happened.

OpenAgents is an unusual early-stage company: the software is live, the marketplace is live, the protocol is public, and a diligence conversation does not need to stay at the slide-deck layer. Two artifacts answer the only two questions an investor has to ask:

1. **Can you ship?**
2. **Does the money actually move?**

Both artifacts live in the same repo that published the code they describe. Both are reproducible.

## Receipt 1 — Pylon v0.1.13 release

The [release receipt](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports) records the commit, the published artifacts, the npm package hashes, and the verification commands that ran before cut. The relevant fragment:

```json
{
  "generated_at": "2026-04-24T07:05:30Z",
  "status": "completed",
  "owner_repo": "OpenAgentsInc/openagents",
  "release_commit": {
    "git_sha": "8590d04a78fb69e984274fb9bddc6387d4edd440",
    "summary": "Scrub legacy runtime references from Pylon"
  },
  "release": {
    "tag": "pylon-v0.1.13",
    "url": "https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.1.13"
  },
  "npm_package": {
    "name": "@openagentsinc/pylon",
    "version": "0.1.13",
    "shasum": "e728207558855e1549e72a59685476141aec009b"
  }
}
```

### What it proves

- The public binary `pylon-v0.1.13-darwin-arm64.tar.gz` was built from commit `8590d04a`, with a known SHA-256.
- The npm package `@openagentsinc/pylon@0.1.13` was published with a verifiable shasum.
- The smoke test `npx @openagentsinc/pylon@0.1.13 --help` was run successfully against the published package — not a local build.
- The verification commands that must pass before cut (cargo tests, lint, type checks, package dry-run) are all named in the receipt and pass.

The release notes carry a deliberately narrow scope statement:

> _"This receipt covers the darwin-arm64 binary release plus npm publish. Hosted Nexus homework scheduling still uses `min_pylon_version=0.1.12` at the server side; 0.1.13 is the current recommended public build."_

That is the **honest-scope posture** we run the whole project on. The receipt tells you exactly what platform was cut, exactly what changed, and exactly what's still on the prior version.

### Why this matters for diligence

Release discipline is the cheapest leading indicator of engineering discipline. A 0.1.x project that can publish a signed, verifiable, reproducible release receipt _for every cut_ — with hashes, commands, and explicit caveats — is a project that won't be caught lying to itself about readiness later. The same pattern scales to every release we'll ever cut.

## Receipt 2 — The 2026-04-23 earning proof

The companion artifact is the [earning proof](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports). This one says something more important: that the _economic_ loop works. Not "the code compiles." **Sats moved.**

### What the proof ran

> _"This proof used the Autopilot Tauri control surface to keep a Pylon worker online, dispatched a bounded CS336 A1 homework/training run through hosted Nexus, validated the worker contribution with a separate Pylon validator process, confirmed accepted-work payout through Treasury, and verified that the worker Pylon wallet balance increased."_

Every actor in the marketplace diagram showed up on a separate, isolated process:

| Actor             | What it did                       |
| ----------------- | --------------------------------- |
| Autopilot         | Kept the worker Pylon online      |
| Worker Pylon      | Completed the CS336 A1 run        |
| Validator Pylon   | Independently checked the work    |
| Backend dispatcher | Dispatched the job and accepted it |
| Treasury          | Cut and settled the 25-sat payout |
| Worker wallet     | Received the 25 sats              |

Six independent actors, six different roles, one settled payout. That's a real end-to-end run.

### The decisive balance proof

```json
{
  "before_total_sats": 0,
  "after_total_sats": 25,
  "delta_total_sats": 25
}
```

And the matching wallet history:

```json
{
  "payment_id": "019db8a2-98d2-7890-95e4-6a1d78709a3c",
  "direction": "receive",
  "status": "completed",
  "amount_sats": 25,
  "fees_sats": 0,
  "method": "spark"
}
```

The Treasury accepted-work payout id and the worker wallet receive payment id match exactly: `019db8a2-98d2-7890-95e4-6a1d78709a3c`. That match is the whole point of the proof. The kernel said "I paid 25 sats to this destination." The wallet said "I received 25 sats from that source." The two records reconcile to the same string.

### The honest caveats

The proof is not a victory lap. It names three things that are _not_ quite clean yet, and commits to fixing them:

1. The broader run-level status stayed `running` while the specific window was already paid. The proof is explicit that the window-level evidence is what matters, not the run-level status.
2. A `treasury_degraded` caveat surfaced on a stale wallet snapshot. Did not block the proof. On the list.
3. A first validator pass reported `artifact_incomplete` and required a refresh before retrying. Retryable, expected, documented.

Every one of those caveats is matched by a code fix in the same release. They're commit messages, written in prose:

> _"Pylon now creates a default local Spark payout destination during the long-lived serve path when the config has no payout destination, instead of coming online without a usable payment target."_
>
> _"Pylon validator replay now reuses an existing retained content-addressed snapshot if a mutable local source path drifted between attempts, avoiding retry-time artifact mismatch."_
>
> _"Autopilot only shows 'Homework paid' when closeout/payout evidence is terminal enough, and stale historical issues no longer override the current paid proof projection."_

That is what we mean by audit-grade. The receipt names what didn't go perfectly, the same release fixes it, and the next receipt will reflect the fix.

## Reproducible on a fresh machine

Both artifacts are written so a third party can re-run them. The release receipt names the exact verification commands. The earning proof points at the operator runbook in the public repo. A diligence partner with a `darwin-arm64` machine can install Pylon, dispatch a starter run, and produce their own matching pair of receipts. The artifact ids will differ — new run id, new payout id, new payment id — but the shape will not.

## What the receipts do not claim

They do _not_ claim the marketplace is mature. Compute is live at small scale. Data is live on two public Nostr relays. Labor, Liquidity, and Risk are roadmap. The 25-sat anchor is a price floor, not a clearing price. Self-hosting works but the starter-job demand only flows from the OpenAgents-hosted backend.

What they _do_ claim is that the team can cut clean, reproducible releases, and that a bounded end-to-end unit of paid machine work can be scheduled, completed, validated, paid, and reconciled across six independent processes. **Today.**

That is the answer to the two questions diligence has to ask.

---

{% hint style="info" %}
**Under the hood.** Engineers can read the full reproduction recipe — every cargo test, every smoke command, the complete operator runbook — in the [Developer Path → Quickstart](../developers/quickstart.md) and at [`docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs).
{% endhint %}

---

**← Previous:** [08. Audit-Grade Trust](08-authority-model.md) · **Next:** [10. Roadmap & Ask](10-roadmap-and-ask.md) **→**
