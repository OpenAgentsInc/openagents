# @openagentsinc/proof-replay

Shared deterministic proof replay primitives for OpenAgents web and Autopilot
Desktop. The package consumes public-safe `proof_replay_bundle.v1` payloads and
produces replay clock state, ordered timeline state, stage and actor layout
plans, camera cue plans, hit targets, and payment visual classifications.

The package is presentation-only. It does not validate proofs, authorize
settlement, dispatch payments, read wallet state, or promote product claims.

## Replay catalog

The package exports the shared replay catalog used by both
`apps/openagents.com` and `apps/autopilot-desktop`:

- `first-real-settlement`
- `launch-recognition-payments`

Use `proofReplayCatalog(origin)` or `proofReplayBundleEndpointForSlug(slug,
origin)` instead of hard-coding replay URLs in individual surfaces. The first
settlement replay keeps its compatibility endpoint at
`/api/public/tassadar-replays/first-real-settlement`; generic replays resolve
through `/api/public/proof-replays?ref=...`.

Before rendering a bundle, call `assertProofReplayBundleShipmentGate(bundle)`.
The gate enforces the public replay contract:

- schema `proof_replay_bundle.v1`;
- `privacyLevel: public_safe`;
- `claimScope: evidence_presentation_only`;
- source refs on events, flows, captions, and gaps;
- no raw wallet/payment/operator/customer material;
- confirmed zaps must cite public receipt or recipient-confirmation evidence;
- blocked settlements cannot carry moving sats;
- simulated payment events cannot claim `realBitcoinMoved:true`.

## Generated Timeline Bundles

`buildProofReplayBundleFromPublicActivityTimeline(envelope, options)` converts
a public-safe `openagents.public_activity_timeline.v1` envelope into a
`proof_replay_bundle.v1`. It maps fleet boot, heartbeat, wallet-ready,
assignment-ready, work, trace, verification, settlement, payment, Forum
discussion, and capacity-snapshot events into deterministic replay events,
camera cues, captions, flows, actors, and stages. Fleet readiness, discussion,
capacity snapshots, and confirmed payments get distinct replay flows.
`projection_gap` rows and stale/unavailable source-lag entries become replay
gaps with source or blocker refs; the builder does not invent motion for
missing data.

The builder validates the timeline envelope first and runs
`assertProofReplayBundleShipmentGate` before returning. A receipt-backed
`real_bitcoin_moved` timeline event is the only path that becomes a confirmed
payment zap. Capacity events remain labeled as aggregate snapshot data, and
Forum body text / author refs stay subject to the public timeline redaction
guard before any replay bundle is emitted.
