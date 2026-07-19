# OpenAgents mobile TestFlight build 123 stuck-send recovery evidence

Date: 2026-07-18

Build 123 replaces build 122 after a real owner-device message appeared
optimistically, disappeared, and never started a Sarah hosted turn. The failure
was reproduced against production state and corrected in both the shared Sync
runtime and the mobile conversation UI.

## Production incident finding

- The installed build 122 was authenticated and repeatedly reached production
  `POST /api/sync/push`; the requests returned HTTP 200.
- The server's authoritative mutation ledger did not advance, no new confirmed
  chat message appeared, and no Sarah/Google AI Studio hosted turn was created.
- The push response contained per-mutation `rejected/out_of_order` results:
  the device's preserved local outbox started above the server's exact next
  mutation ID after a prior local/server history divergence.
- HTTP success described the successfully decoded Sync envelope, not acceptance
  of every mutation inside it. The old client retried that terminal ordering
  mismatch indefinitely and did not surface the in-band rejection.
- The mobile UI initially rendered an optimistic user bubble, then rebuilt the
  transcript from a confirmed-only Sync snapshot. That erased the only visible
  evidence of the pending send even though the local outbox still retained it.

No message content, account identifier, credential, or private device identity
is included in this receipt.

## Corrections in this candidate

- The shared Khala Sync client detects the authenticated `out_of_order` head,
  atomically rebases a locally dense pending mutation queue to the server's
  exact watermark plus one, rebuilds the optimistic overlay, and immediately
  retries. Mutation bodies, order, timestamps, and authority remain unchanged.
- The repair refuses empty, invalid, already aligned, or non-dense queues. The
  server's strict ordering policy is not weakened.
- Web-worker protocol/proxy parity is included so every shared-client adapter
  exposes the same repair behavior.
- Mobile replaces the temporary user-message identity with its exact durable
  message reference after local admission.
- Pending user messages survive confirmed-only intermediate Sync snapshots and
  display `Sending…` or `Still trying to send…`.
- A terminal send failure leaves the user's bubble visible, shows bounded safe
  error copy, and restores the draft for correction or retry. Only exact server
  confirmation clears local delivery state.
- The active app UX registry is `2026-07-18.1` and enforces
  `openagents_mobile.conversation.send_delivery_visibility.v1`.

## Artifact identity

- Application: OpenAgents mobile
- Bundle identifier: `com.openagents.app`
- Marketing version: `0.5.2`
- Build number: `123`
- Source commit: `f85f56edf857fc91e7fda98bf9b2c90e37a30335`
- Archive: `/tmp/OpenAgents-123-f85f56e.xcarchive`
- IPA: `/tmp/OpenAgents-123-f85f56e-export/OpenAgents.ipa`
- IPA bytes: `19,452,522`
- IPA SHA-256: `68fd17c51f01bceaf5fc39bb7198db9c237e2bb4a6aeb519598fa5dbb50613530e78a0168b37c3b3946388fec9ea2a8464cfa26a`
- Expo runtime fingerprint: `68fd17c51f01bceaf5fc39bb7198db9c237e2bb4`

## Apple receipt

- Delivery/build ID: `347bfac7-5375-45db-99f2-0e482fda74ed`
- Uploaded at: `2026-07-18T18:48:12-07:00`
- App Store Connect processing state: `VALID`
- Expired: `false`

The App Store Connect API returned the exact `0.5.2 (123)` build record in
`VALID` state. This proves Apple accepted this exact candidate for TestFlight;
it does not authorize or claim stable App Store promotion.

## Gates

- Shared Sync suite: 31 files passed and 1 skipped; 221 tests passed and 3
  skipped (224 total).
- Mobile suite: 58 files and 286 tests passed.
- Mobile and shared Sync client typechecks: passed.
- Behavior-contract suite: 36 tests passed.
- Root `pnpm run check`: passed.
- Repository pre-push mobile gate: passed.
- Release iPhone simulator build and launch: passed.
- Signed generic-device Xcode archive: passed.
- Manual App Store distribution export/signing: passed.
- Apple upload: passed.
- App Store Connect exact-build verification: passed (`VALID`).

The regression test recreates a preserved outbox whose pending mutation IDs
start at 3 and 4 while the authenticated server watermark is 0. It proves the
client repairs them to 1 and 2, delivers the original intent bodies in order,
and drains the queue.

## Authority and limitations

The owner explicitly directed diagnosis, correction, and a new TestFlight
candidate in this conversation. Release actor: operating/release agent. This
authority covers the mobile TestFlight candidate and its incident
communication only; it does not promote an App Store stable release, Desktop
stable, or the signed Desktop update feed.
