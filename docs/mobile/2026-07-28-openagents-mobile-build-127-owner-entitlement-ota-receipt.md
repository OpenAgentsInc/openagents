# OpenAgents mobile build 127 owner recovery OTA receipt

Date: 2026-07-28

Issue:
[OpenAgentsInc/openagents#9273](https://github.com/OpenAgentsInc/openagents/issues/9273)

Status: Published. Physical iPhone acceptance is open.

## Outcome

TestFlight build 127 can recover after a new account session or an entitlement
change. The app refreshes its protected OpenAgents session and retries the
Sarah session request one time. If the second request fails, the app shows the
real final error.

The app does not make a credit or entitlement decision. It does not bypass a
server credit check. A user who needs credits sees this message:
`Sarah voice needs available OpenAgents credits. Add credits, then retry voice.`

The staging server can give the canonical owner account one audited and
revocable seven-day Sarah voice entitlement. The server applies this rule only
on the exact staging host. The entitlement has no credit hold or debit. All
authentication, session, time, use, and abuse limits continue to apply.

Production billing and the production API did not change.

## Source and verification

- Mobile commit: `fdeec4e380`
- Server merge commit: `f553cac1d92ce1238d6103dfd62ed9e202615578`
- Mobile tests: 63 of 63 passed.
- Focused server and store tests: 11 of 11 passed.
- Mobile TypeScript check: passed.
- API TypeScript check: passed.
- Cloud Run TypeScript check: passed.
- OTA TypeScript check: passed.
- Main pre-push policy: passed.

The mobile tests check a successful one-time recovery after HTTP 402 and HTTP
403. They also check that a second HTTP 402 keeps the credit error. The client
does not retry more than one time.

## Staging service

- Service: `openagents-monolith-staging`
- Revision: `openagents-monolith-staging-00150-fq9`
- Traffic: 100 percent
- Image digest:
  `sha256:2f91e4496dfb08c4956adac3ec09cc3f486a3ddbb911466bc7cd3bc9f9373e2c`

The health route returned HTTP 200. An unauthenticated Sarah session request
returned HTTP 401 with `cache-control: no-store`. The staging owner entitlement
gate is on. The revision has no Nostr device self-provision environment value.

The redacted authenticated smoke passed. The canonical primary owner received
an active seven-day entitlement. The owner opened a voice session with
`reservedCreditMsat=0`. The session made no payment row, and the owner balance
did not change.

A non-owner with a zero balance remained rejected. An unknown NIP-98 key also
remained rejected. Nostr device self-provision remained off.

The operator did not extract the owner's mobile bearer. Therefore, this
receipt does not claim a complete voice session from the installed iPhone.

## OTA publication

- Installed native target: OpenAgents iOS TestFlight `0.5.2 (127)`
- Runtime fingerprint:
  `4d325a423edf99c9fcdadf67ed6b8b0bcb743dfc`
- Channel: `openagents-production`
- Cloud Run service: `oa-updates`
- Revision: `oa-updates-00126-kqj`
- Traffic: 100 percent
- Image digest:
  `sha256:ebafbff9c6bcbc1358e60bee9660e8fe4f3a57ceb1a350e60bd968cc79bf6492`
- Update ID: `36c6c979-28d1-40ed-b17d-f66d96f085af`
- Created at: `2026-07-29T00:13:39.168Z`
- Launch asset: `index-2cb77abf07aefc8e686d3a62f71f484b.hbc`
- Launch asset bytes: `4,691,658`
- Launch asset SHA-256 (base64url):
  `e7a6dqkrk8vk6EkOD70amR5JI8Rtneb3h-Owqu60L0M`

The exact build-127 runtime gate passed before publication. The signed Expo
protocol v1 manifest returned HTTP 200 with key ID `main`. The launch asset
returned HTTP 200. Its byte length and SHA-256 value matched the manifest. A
different runtime returned `noUpdateAvailable`.

The Desktop RC `release.json`, `manifest.json`, and `manifest.sig.json` routes
continued to return HTTP 200 after the OTA promotion.

## Residual physical acceptance

The owner must fully close and open TestFlight build 127. The first open can
download the update. Fully close and open the app a second time to apply it.

Then open Sarah voice. Confirm that the credit message is gone and that Sarah
starts. Confirm microphone capture, an audible reply, transcripts, interrupt,
end, and background cleanup.

This physical voice flow is the final user acceptance. The server entitlement
and zero-credit checks are complete.
