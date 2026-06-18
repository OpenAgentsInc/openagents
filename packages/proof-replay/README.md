# @openagentsinc/proof-replay

Shared deterministic proof replay primitives for OpenAgents web and Autopilot
Desktop. The package consumes public-safe `proof_replay_bundle.v1` payloads and
produces replay clock state, ordered timeline state, stage and actor layout
plans, camera cue plans, hit targets, and payment visual classifications.

The package is presentation-only. It does not validate proofs, authorize
settlement, dispatch payments, read wallet state, or promote product claims.

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
