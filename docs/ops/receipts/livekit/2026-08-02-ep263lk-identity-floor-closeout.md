# EP263-LK-04 identity and floor closeout

- Issue: `OpenAgentsInc/openagents#9286`
- Acceptance date: 2026-08-02
- Acceptance basis: owner-approved basic production and focused automated testing
- Source baseline: `84fc4f8434`

## Accepted implementation

Sarah community rooms use a server-owned presence lease, stable public Nostr
identity, narrow Workload Identity signer, durable membership and floor state,
and worker-side microphone selection bound to the authoritative floor holder.
Floor transfer, moderator stop, timeout, member removal, Sarah removal, nonce
replay, stale revision, forged participant mapping, and cross-group isolation
have typed fail-closed paths.

The signer accepts only the public-safe presence and kind-9 projection
templates. Sarah key material is not supplied to the LiveKit Agents pods, and
signed room text remains a projection rather than command, membership, audio,
or settlement authority.

## Evidence

- `production-sarah-headless-20260801-rc30.json` records concurrent private and
  community generations, shared community audio, transcription, interruption,
  exact usage, and terminal settlement.
- `2026-07-31-ep263lk-community-join-acceptance.json` records the production
  community join and two-room journey.
- `production-runtime-signer-worker-20260731T113000Z.json` and
  `production-secret-scan-20260801.json` record the deployed signer/worker
  boundary and passing production privacy scan.
- Focused identity, signer, membership, floor, worker-selection, retirement,
  replay, impersonation, and isolation tests passed: 8 files, 66 tests.

The installed three-desktop journey is not a close gate under the owner's
2026-08-02 basic-testing direction. Automated authority tests cover the
three-member floor-transfer and negative cases that the headless production
receipt does not exercise interactively.

## Known scope boundary

Omega 0.2.0 does not distribute LiveKit media-frame E2EE keys. The
`e2eeKeyRevision` field binds admitted configuration only; it is not key
material and does not imply media-frame encryption or key rotation. Actual
media-frame E2EE and membership-triggered key distribution remain future work
and are not claimed by this closeout.
