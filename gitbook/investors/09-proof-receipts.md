[Home](../README.md) · [Investor Path](README.md) · **09. Proof Receipts**

# 9. Proof Receipts

> _"Founders can say anything. Code, version tags, and signed payout receipts can only say what actually happened."_

**You will learn:**

- The `pylon-v0.1.13-release.json` release receipt
- The 2026-04-23 Autopilot-controlled earning proof (0 → 25 sats, payout id `019db8a2-…`)
- How each receipt is reproducible on a `darwin-arm64` machine

## Why we lead with receipts

Founders can say anything. Code, version tags, and signed payout receipts can only say what actually happened.

OpenAgents is an odd kind of early-stage company: the software is live, the market is live, and the protocol is public. A diligence conversation does not need to stay at the slide-deck layer. Two artifacts — one release receipt, one production earning proof — answer the two questions an investor has to ask:

1. _Can you ship?_
2. _Does the money actually move?_

Both artifacts live in the same repo that published the code they describe. Both are reproducible.

## Receipt 1 — Pylon v0.1.13 release

The [`docs/reports/pylon-v0.1.13-release.json`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports) receipt records the commit, the published artifacts, the npm package hashes, and the verification commands that ran before cut.

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
    "name": "Pylon v0.1.13",
    "url": "https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.1.13",
    "target_commitish": "8590d04a78fb69e984274fb9bddc6387d4edd440"
  },
  "npm_package": {
    "name": "@openagentsinc/pylon",
    "version": "0.1.13",
    "shasum": "e728207558855e1549e72a59685476141aec009b",
    "integrity": "sha512-mRyzDvK5nR5g9fK/nSM21DCPb/b7kSI/KWs22UTmsncNoio3ikAd60RwoPNU1VbllWFiKQCg8vwjQ4G8IaqmEw=="
  }
}
```

### What it proves

- The public binary `pylon-v0.1.13-darwin-arm64.tar.gz` was built from commit `8590d04a`, and its SHA-256 is `de995efc90675d90108785a2790e0c2bc4099cd0ef6eaff2d8ae58fccc234a66`.
- The npm package `@openagentsinc/pylon@0.1.13` has shasum `e728207558855e1549e72a59685476141aec009b` and was published to the public npm registry.
- The smoke test `npx @openagentsinc/pylon@0.1.13 --help` was run successfully against the published package, not a local build.
- Verification commands that must pass before cut — `bun test packages/pylon-bootstrap/test/bootstrap.test.js`, `cargo test -p openagents-kernel-core -p openagents-provider-substrate -p pylon -p nexus-control --no-run`, `bun run lint:check`, `bun run types:check`, `bash -n scripts/bootstrap-gcp-pylon.sh` — are all named in the receipt.

The release notes that accompany the tag include this deliberately narrow scope:

> _"This receipt covers the darwin-arm64 binary release plus npm publish. Hosted Nexus homework scheduling still uses `min_pylon_version=0.1.12` at the server side; 0.1.13 is the current recommended public build."_
>
> — [`pylon-v0.1.13-release.json`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports)

That is the "honest-scope posture" we run the whole project on. The receipt tells you exactly which platform is cut (darwin-arm64), exactly what the server-side minimum is (0.1.12), and exactly what changed (legacy runtime wording was scrubbed out of the public onboarding path).

### Why an investor should care

Release discipline is the cheapest leading indicator of engineering discipline. A 0.1.x project that can publish a signed, verifiable, reproducible release receipt _for every cut_ — with hashes, commands, and explicit scope caveats — is a project that will not be caught lying to itself about readiness later. The same pattern scales to every release the company will ever cut: the kernel, the Autopilot app, Nexus, Psionic binaries, Treasury updates.

## Receipt 2 — 2026-04-23 Autopilot earning proof

The companion artifact is [`docs/reports/earning-proof.md`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports), a live end-to-end proof that the _economic_ loop works — not that code compiles, but that sats move.

### What the proof ran

Quoting the report directly:

> _"This proof used the Autopilot Tauri control surface to keep a Pylon worker online, dispatched a bounded CS336 A1 homework/training run through hosted Nexus, validated the worker contribution with a separate Pylon validator process, confirmed accepted-work payout through Treasury, and verified that the worker Pylon wallet balance increased."_
>
> — [`earning-proof.md`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports)

Every actor in the five-market diagram from [Chapter 2](02-five-markets.md) showed up on a separate port:

| Actor                   | Isolated home                                                             | Role                           |
| ----------------------- | ------------------------------------------------------------------------- | ------------------------------ |
| Autopilot Tauri app     | `control manifest` + isolated Pylon home at `127.0.0.1:55477`             | kept provider online           |
| Worker Pylon            | `be83d8974051cf6874e12117d04773cd1d0bb3b98acacac801a98ef0d5bf69e9` pubkey | completed the CS336 A1 run     |
| Validator Pylon         | `127.0.0.1:55880` / `127.0.0.1:55881`                                     | checked work independently     |
| Hosted Nexus            | `POST /v1/admin/homework/cs336-a1/dispatch`                               | dispatched and accepted        |
| Treasury                | `accepted_work:…:019db8a2-98d2-7890-95e4-6a1d78709a3c`                    | cut and settled the 25-sat pay |
| Worker Pylon wallet     | `spark1pgssyt9agft907ew09l6kndl59gtguccvpyuv6h90489ct7hm0drz7rzmswm7g`    | received the 25 sats           |

### The decisive balance proof

```json
{
  "before_total_sats": 0,
  "after_total_sats": 25,
  "delta_total_sats": 25,
  "before": { "spark_sats": 0, "lightning_sats": 0, "onchain_sats": 0, "total_sats": 0 },
  "after":  { "spark_sats": 25, "lightning_sats": 0, "onchain_sats": 0, "total_sats": 25 }
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

The proof names those two ids so any reader can reconcile them: the Treasury accepted-work payout receipt id and the worker wallet receive payment id match exactly — `019db8a2-98d2-7890-95e4-6a1d78709a3c`. That match is the whole point of the proof.

### Where authority showed up

Read alongside [Chapter 8 — Authority Model](08-authority-model.md), every authority lane on the diagram fired once in this proof, and each left a named artifact:

- **Kernel authority**: the run id, window id, and assignment id (`run.cs336.a1.…65bb3390`, `window.…65bb3390.0001`, `assign.run.…attempt1`) were issued by Nexus / the kernel. They are present in both the validator closeout and the Treasury payout.
- **Provider identity**: the worker pubkey `be83d8974051cf6874e12117d04773cd1d0bb3b98acacac801a98ef0d5bf69e9` is tied by kernel to both the run and the Spark destination. The payout id encodes this binding: `accepted_work:…:e5f851f79f0d0d31afde7acb9687ed0c133036bc78459d4bd9504df02b862984:be83d8974051cf6874e12117d04773cd1d0bb3b98acacac801a98ef0d5bf69e9`.
- **Treasury authority**: the payout state is `confirmed` and reconciliation is `settled`, cut at the kernel layer — not at the Autopilot UI layer.
- **Spacetime is nowhere in the money lane**. The Autopilot local cache still said `dispatched / pending_confirmation` after the kernel had already marked the payout `confirmed / settled / completed`. The proof calls this out by name:

> _"That is an observation about the worker-local projection lag, not a payout failure. The user-facing status was already `Homework paid`, and the wallet balance/history confirmed receipt."_
>
> — [`earning-proof.md`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports)

This is exactly the guarantee [ADR-0001](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/0001-authority-boundaries.md) was written to preserve: the UI can be late, and the money is still correct.

### The honest caveats in the proof

The proof is deliberately not a victory lap. Its own text names three things that are _not_ quite yet clean, and commits to fixing them:

1. **Run-level status stayed `running`.** The broader hosted run object can stay open while a specific window is already reconciled and payout-eligible. The window-level evidence is the terminal evidence, not the run-level status.
2. **A `treasury_degraded` caveat surfaced on `wallet_snapshot_stale`.** The wallet runtime was connected and placeholder payouts were disabled, so this did not block the proof — but it is on the list.
3. **A first validator pass reported `artifact_incomplete`.** That was retryable and consistent with claiming the aggregate challenge before the worker artifact bundle was fully visible. The proof is explicit that a `training refresh / training sync` is required before retrying validator intake in this situation.

The accompanying code fixes cover exactly these classes of issue:

> _"Pylon now creates a default local Spark payout destination during the long-lived serve path when the config has no payout destination, instead of coming online without a usable payment target."_
>
> _"Pylon validator replay now reuses an existing retained content-addressed snapshot if a mutable local source path drifted between attempts, avoiding retry-time artifact mismatch."_
>
> _"Autopilot only shows `Homework paid` when closeout/payout evidence is terminal enough, and stale historical issues no longer override the current paid proof projection."_
>
> — [`earning-proof.md`](https://github.com/OpenAgentsInc/openagents/tree/main/docs/reports)

These are not marketing lines. They are commit messages described in prose.

## The reproducibility cliff

Both receipts are written so a third party could re-run them.

The release receipt names:

- `bun test packages/pylon-bootstrap/test/bootstrap.test.js`
- `cargo test -p openagents-kernel-core -p openagents-provider-substrate -p pylon -p nexus-control --no-run`
- `bun run lint:check -- resources/js/pages/welcome.tsx`
- `bun run types:check`
- `bash -n scripts/bootstrap-gcp-pylon.sh`
- `npm pack --dry-run (packages/pylon-bootstrap)`
- `npx @openagentsinc/pylon@0.1.13 --help`

The earning-proof receipt names the full operator runbook at [`docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md`](https://github.com/OpenAgentsInc/openagents/tree/main/docs) and the focused verification commands that accompanied the code fixes:

- `cargo test -p pylon config_set_updates_payout_destination`
- `cargo test -p pylon default_payout_destination_uses_wallet_spark_address`
- `cargo test -p pylon snapshot_training_retained_artifact_binding`
- `cargo check -p pylon`
- `cargo check -p autopilot`
- `cargo test -p autopilot --lib`
- `scripts/autopilot/tauri-control-smoke.sh --homework-handshake --timeout-ms 600000`

A diligence partner with a `darwin-arm64` machine and a fresh Pylon home can run the exact path that moved the 25 sats. The artifact ids will differ — a new run id, a new payout receipt, a new payment id — but the shape of the receipt, and the ability to reconcile kernel record with wallet history, will not.

## What the receipts do not claim

They do _not_ claim:

- That the five-market marketplace is mature. It is not. Compute is live at small scale; Data is live on kinds 5960 / 6960 / 31990 on `wss://relay.damus.io` and `wss://relay.primal.net`; Labor is in design; Liquidity and Risk are roadmap. See [Chapter 2](02-five-markets.md).
- That pricing is at a venue-maker equilibrium. A 25-sat per-contribution anchor with a 6,400 sat cap per cycle and an OpenAgents-hosted subsidy is a starter-grade price floor, not a clearing price. See [Chapter 4](04-earn-loop.md).
- That self-hosting is fully turnkey. The desktop can be pointed at a user-owned Nexus + relay set, but the starter-job subsidy only flows from the OpenAgents-hosted Nexus. See [Chapter 8](08-authority-model.md).

What they _do_ claim is that (a) the team can cut clean, reproducible releases and (b) a bounded end-to-end unit of paid work can be scheduled, completed, validated, paid, and reconciled across six independently-running processes — today.

---

**← Previous:** [08. Authority & Ownership](08-authority-model.md) · **Next:** [10. Roadmap & Ask](10-roadmap-and-ask.md) **→**
