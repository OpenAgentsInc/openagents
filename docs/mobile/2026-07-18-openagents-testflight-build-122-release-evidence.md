# OpenAgents mobile TestFlight build 122 release evidence

Date: 2026-07-18

Build 122 is the replacement candidate for the superseded Sarah builds 120
and 121. It combines the immediate hosted-turn dispatch fix, the quiet and
interruptible Sarah conversation UI, and persistent system-login reuse.

## Artifact identity

- Application: OpenAgents mobile
- Bundle identifier: `com.openagents.app`
- Marketing version: `0.5.2`
- Build number: `122`
- Mobile source commit: `886244603788b180461203ab3436ef0f2276fec2`
- Immediate-dispatch/UI source commit: `61f9f054ec64f663c6c29b52c28ec467e543356d`
- Production API revision: `openagents-monolith-00192-2ls` (100% traffic)
- Expo fingerprint: `29d1df1634690f559a461cba6cb4a68cf231a529`
- Archive: `/tmp/OpenAgents-122-8862446.xcarchive`
- IPA: `/tmp/OpenAgents-122-8862446-export/OpenAgents.ipa`
- IPA bytes: `19,449,563`
- IPA SHA-256: `66accf58c7711e18f99f453770b7fcfaf90a92d454081f8e799e0ae920ca03f6`

## Apple receipt

- Delivery/build ID: `4461d824-5aca-406d-92aa-5351a8bb0b9f`
- Uploaded at: `2026-07-18T17:50:00-07:00`
- App Store Connect processing state: `VALID`
- Expired: `false`

The App Store Connect API returned the exact `0.5.2 (122)` build record in
`VALID` state. This proves Apple accepted the exact IPA above for TestFlight;
it does not claim stable App Store promotion.

## Corrections in this candidate

- An accepted `runtime.startTurn` now schedules the hosted dispatcher
  immediately after the Sync transaction commits. The minute cron is recovery
  only; it is no longer the normal message-start mechanism.
- Sarah's message list jumps to its target without a long forced animation.
  Manual scrolling is no longer repinned while a turn is active.
- The active conversation presents one quiet `Thinking…` status instead of
  exposing hosted-runtime startup jargon. Failure copy is similarly bounded.
- The composer remains conversational and uses `Continue conversation` while
  a turn starts.
- Native GitHub authorization now uses the persistent iOS system session
  (`preferEphemeralSession: false`). After a successful authorization,
  OpenAgents access and refresh credentials remain in the app's encrypted
  SecureStore service `com.openagents.mobile.session` and are server-verified
  on every launch.

The previous build's ephemeral browser session could not be recovered after
the fact because iOS had already discarded it. Build 122 prevents the same
loss for subsequent successful sign-ins.

## Real production proof

A real `hosted_khala` turn was admitted through the same `/api/sync/push`
message plus `runtime.startTurn` batch used by mobile:

- Turn: `8e8396df-0f69-49f4-a4b1-61317b4a52c2`
- Prompt: `Reply with exactly SARAH_FAST_122_OK.`
- Reply: `SARAH_FAST_122_OK`
- Created: `2026-07-19T00:49:42.282Z`
- Started: `2026-07-19T00:49:42.431Z` (149 ms after admission)
- Settled: `2026-07-19T00:49:45.966Z`
- Script-observed end-to-end time: 5.85 seconds
- Terminal state: `completed`, finish reason `stop`
- Runtime events: `turn.started`, `text.delta`, `text.completed`,
  `usage.recorded`, `turn.finished`
- Model/provider provenance: `gemma-4-31b-it` / `google-ai-studio`

This replaces the captured pre-fix owner turn whose runtime start was delayed
49 seconds by the minute scheduler.

## Gates

- OpenAgents mobile suite: 58 files, 286 tests passed.
- Immediate-dispatch targeted suite: 47 tests passed.
- Mobile, API Worker, and Cloud Run TypeScript checks: passed.
- Repository pre-push guard: passed.
- Production health smoke after deployment: passed.
- Simulator Release build: passed with `CFBundleVersion=122`.
- Xcode archive: passed.
- Manual App Store distribution export/signing: passed.
- Apple upload: passed.
- App Store Connect exact-build verification: passed (`VALID`).
- Real production hosted-turn proof: passed.

Build 120 was uploaded and then superseded. Build 121 was archived locally but
was not uploaded after the login-reuse defect was identified. Neither build is
promoted by this receipt.

## Authority and limitations

The owner explicitly directed the Sarah fixes and a new TestFlight upload in
this thread. Release actor: operating/release agent. This is a mobile candidate
only; it does not promote Desktop stable, alter the signed Desktop update feed,
or authorize stable App Store release.
