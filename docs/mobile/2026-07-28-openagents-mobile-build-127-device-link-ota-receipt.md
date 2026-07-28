# OpenAgents mobile build 127 device-link OTA receipt

Date: 2026-07-28

Issue:
[OpenAgentsInc/openagents#9273](https://github.com/OpenAgentsInc/openagents/issues/9273)

Status: Published. Physical iPhone acceptance is open.

## Outcome

TestFlight build 127 can now link a new protected mobile Nostr key to the
current signed-in OpenAgents account. The user does not manage a Nostr key.

The client first uses the current mobile bearer to request a short-lived
challenge. The challenge binds the account, public key, and device. The client
then signs the exact link request with the protected device key. The server
consumes the challenge and the NIP-98 proof one time.

The server rejects a replay. It also rejects a key that belongs to a different
account. This flow does not enable unrestricted Nostr self-provision.

The unavailable state no longer shows internal device-identity text. A
recoverable failure tells the user to try again. A signed-out user sees a
request to sign in.

## Source and verification

- Source commit: `f7a8f468ac50791a6d3a50632ecd10353def5cd3`
- Mobile tests: 61 of 61 passed.
- Focused server and shared-contract tests: 35 of 35 passed.
- Mobile TypeScript check: passed.
- API TypeScript check: passed.
- Cloud Run TypeScript check: passed.
- Shared audio-contract TypeScript check: passed.
- Main pre-push policy: passed.

The regression tests cover a new signed-in device, an already-linked device, a
key that belongs to a different account, a signed-out user, and a rejected
proof. The tests also check the exact challenge body, link body, NIP-98 URL,
method, payload hash, and device header.

## Staging service

- Service: `openagents-monolith-staging`
- Revision: `openagents-monolith-staging-00146-7pl`
- Traffic: 100 percent
- Image digest:
  `sha256:7eab5a96caa30f177a0611cfda49197f79b942bdd77b9e431d2897e5495e9702`

The health route returned HTTP 200. An unsigned challenge request returned
HTTP 401. An unsigned link request returned HTTP 401. A GET request to the
challenge route returned HTTP 405. The retired unchallenged mobile link route
returned HTTP 404.

The revision has no `OMEGA_NOSTR_SELF_PROVISION` environment. This release did
not change the production API, billing policy, or credit policy.

## OTA publication

- Installed native target: OpenAgents iOS TestFlight `0.5.2 (127)`
- Runtime fingerprint:
  `4d325a423edf99c9fcdadf67ed6b8b0bcb743dfc`
- Channel: `openagents-production`
- Cloud Run service: `oa-updates`
- Revision: `oa-updates-00124-fzf`
- Traffic: 100 percent
- Rollback revision: `oa-updates-00123-lsh`
- Update ID: `0b176b39-2ff4-4eed-a9e8-2d1fbc0735ff`
- Created at: `2026-07-28T23:30:08.407Z`
- Launch asset: `index-cc0be07fb12c332591479befa40e107c.hbc`
- Launch asset bytes: `4,690,389`
- Launch asset SHA-256 (base64url):
  `xvzNl_7OqMpVchjzjAuMIAuhk1B32tnTOfHgDpxvW0A`

The exact build-127 runtime gate passed before publication. The signed Expo
protocol v1 manifest returned HTTP 200 with key ID `main`. The launch asset
returned HTTP 200. Its byte length and SHA-256 value matched the manifest.
A different runtime returned `noUpdateAvailable`.

The Desktop `release.json`, `manifest.json`, and `manifest.sig.json` routes
continued to return HTTP 200 after the OTA promotion.

## Residual physical acceptance

The release operator did not extract the owner mobile bearer. Therefore, this
receipt does not claim a successful account-link request from the installed
iPhone.

The owner must fully close and open TestFlight build 127. The first open can
download the update. A second full close and open can be necessary to apply
it.

After the update starts, open Sarah voice. Confirm automatic session recovery,
microphone capture, an audible response, transcripts, mute, interrupt, end,
retry, background cleanup, and the denied, expired-session, credit, and
network errors.
