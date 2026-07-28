# OpenAgents mobile build 127 Sarah voice preflight receipt

Date: 2026-07-28

Issue:
[OpenAgentsInc/openagents#9273](https://github.com/OpenAgentsInc/openagents/issues/9273)

Status: Local implementation and native build checks pass. The TestFlight
candidate is not built or uploaded.

## Scope

This receipt records the local preflight for managed Sarah voice in
`apps/openagents-mobile`.

The app uses its protected Nostr device identity and a normal OpenAgents
session. It does not contain an OpenAI API key. Expo Audio captures mono 24 kHz
PCM. The local native module plays mono 24 kHz PCM. The Sarah screen has live
transcripts, mute, interrupt, end, and retry controls.

The `mobile_voice_only` profile has no tools. It does not permit editor, file,
URL, shell, Git, payment, account, or device actions. Normal OpenAgents credit
continues to apply.

## Candidate identity

- App name: `OpenAgents`
- Bundle identifier: `com.openagents.app`
- Marketing version: `0.5.2`
- Prepared build number: `127`
- Apple team: `HQWSG26L43`
- App Store Connect app ID: `6748620735`

Build 126 was `VALID` in App Store Connect at the preflight observation.
Build 127 was not present at that observation. This receipt does not reserve
the number in Apple systems.

## Local verification

These checks passed:

```text
pnpm --dir apps/openagents-mobile run typecheck
pnpm --dir apps/openagents-mobile run test
pnpm exec vp test --run packages/audio-contract/src/sarah-realtime.test.ts apps/openagents.com/workers/api/src/sarah-realtime-voice-routes.test.ts apps/openagents.com/workers/api/src/cloudrun/sarah-realtime-bridge.test.ts
pnpm --filter @openagentsinc/audio-contract typecheck
pnpm --filter @openagentsinc/khala-sync-server test -- sarah-realtime-voice-store.test.ts
pnpm --filter @openagentsinc/khala-sync-server typecheck
pnpm --dir apps/openagents.com/workers/api typecheck:cloudrun
```

The focused shared-contract, route, and gateway suite passed 22 tests.
The final mobile suite passed 55 tests in 11 files.

The documented Khala Sync package command expanded to its complete package
suite. It passed 683 tests in 81 files. Its local
Postgres harness created a temporary database, applied the migration set, and
exercised Sarah credit reservation, profile persistence, usage idempotency,
settlement, and replay safety. This is the real local database service smoke.
It does not include the external Realtime provider.

A clean Expo prebuild generated both native projects. The derived iOS
`Info.plist` contains this microphone purpose:

```text
Allow OpenAgents to use the microphone for live conversations with Sarah.
```

The iOS file has no `UIBackgroundModes` entry. The Android source manifest
requests `RECORD_AUDIO`. It does not declare a microphone foreground service.
The generated iOS entitlement has the default OpenAgents keychain access group.
The app config and identity test lock that group.

The iOS simulator app build passed after the native interruption changes.
The Android `:app:assembleDebug` task also passed. The Android run completed
392 tasks.

## Simulator pixel check

An isolated iPhone 17e simulator started with an empty app container. The
entitlement-enabled app created its protected device identity. The home screen
showed `Talk to Sarah`. The Sarah screen enabled `Start voice`.

The first Start action showed the exact iOS microphone purpose text. After the
operator selected Allow, the app sent a challenge request to production.
Production returned `404`. The screen showed the bounded unavailable-service
message, enabled Retry, and kept microphone capture off.

The operator closed and reopened the Sarah screen. The second mount stayed
ready and did not produce a disposed-audio-object error.

Pixel evidence:

```text
docs/mobile/receipts/2026-07-28-build127-sarah-voice-ready.png
docs/mobile/receipts/2026-07-28-build127-sarah-voice-service-unavailable.png
```

SHA-256 values in the same order:

```text
f1e24b3bd247568b2e0ee5e4f0fbbcf86c2832f9a73ffef10e0f7f9196244787
1a7c8e1ac237e643ef707574c8e73fd12eda24703b616e3fd0f1cdc46174652f
```

## Service observation

Production returned `404` for the Sarah challenge and session paths.

The current staging Cloud Run service accepted its platform identity token.
The Sarah challenge path then returned `503` with:

```json
{"error":"sarah_voice_auth_challenge_unavailable"}
```

The staging log recorded this event:

```text
sarah_voice_nostr_challenge_storage_unavailable
```

The staging session path returned `401` for a request without an OpenAgents
session. This response confirms the route boundary. It does not prove a managed
voice session.

The preflight did not test an authenticated gateway session, provider
connection, credit debit, live transcript, microphone stream, or speaker
stream. The current staging challenge-store failure blocks that proof.

## Apple credential boundary

The App Store Connect environment file and private key file have owner-only
permissions. Their local modes are `0600`. This receipt does not contain the
credential values.

The preflight did not use a Keychain probe. The presence of an exportable
Apple Distribution private key is therefore not established. Archive and
export must establish that signing fact after the product gates pass.

## Remaining release gates

Do these steps in order:

1. Land the implementation on `main`.
2. Deploy a staging revision from the required server commit.
3. Apply migrations `0103_sarah_realtime_voice.sql` and
   `0104_sarah_voice_client_profile.sql`.
4. Repair the staging Nostr challenge store.
5. Complete an authenticated mobile gateway session with normal credit.
6. On a physical iPhone, verify the microphone permission copy.
7. Verify microphone capture, speaker playback, and transcripts.
8. Verify mute, interrupt, end, retry, reconnect, credit denial, and network
   loss.
9. Send the app to the background and confirm that the session stops.
10. Return to the foreground and confirm that the session stays stopped.
11. Use the explicit Start voice action and confirm that a new session works.
12. Get a distinct reviewer for the candidate-bound evidence.
13. Use a clean worktree at exact `origin/main`.
14. Build, archive, export, and upload build 127 with the local release runbook.
15. Record the IPA digest, delivery UUID, App Store Connect build ID, and
    `VALID` state.

Do not use an OTA update for this change. The change adds microphone permission
and native audio code.

This receipt is preflight evidence only. Do not rename it or promote it to
TestFlight release evidence.
