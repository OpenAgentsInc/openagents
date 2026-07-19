# OpenAgents mobile Sarah live tool-activity OTA receipt

Date: 2026-07-19

## Outcome

The owner asked Sarah `Ok inspect it`. Production evidence shows that the turn
really called `sarah_harness_status`, recorded a successful provider-executed
tool result, and only then produced the answer. Mobile's compact Sarah mode was
discarding every work entry, so the truthful tool evidence existed in Sync but
was invisible in the conversation.

Commit `c2ff92159c` changes compact Sarah presentation to retain confirmed tool
call/result/failure entries and render them as short conversation activity.
For this turn the visible row changes from `Inspecting Sarah's harness…` to
`Sarah's harness inspected` with `Tool result received`. It does not expose raw
broker names, call/result refs, arguments, results, provider plumbing, usage,
or generic runtime cards.

## Production turn evidence

- User message created: `2026-07-19T04:56:44.984Z`
- Turn started: `2026-07-19T04:56:46.151Z`
- Tool call recorded: `2026-07-19T04:56:49.307Z`
- Tool result recorded: `2026-07-19T04:56:49.540Z`
- Turn completed: `2026-07-19T04:57:01.124Z`
- Provider/model: Google AI Studio / Gemma 4 31B
- Result: the tool was genuinely used; the prior UI hid the evidence

Private account identifiers, prompts beyond the owner-provided phrase above,
tool payloads, credentials, and raw results are omitted.

## OTA publication

- Source commit: `c2ff92159c1d6f39dd571add8980efff18f42b9b`
- Bundle tag: `2026-07-19.sarah-live-tool-activity-10`
- Installed native target: OpenAgents iOS TestFlight `0.5.2 (123)`
- Runtime fingerprint: `68fd17c51f01bceaf5fc39bb7198db9c237e2bb4`
- Channel: `openagents-production`
- Cloud Run service/revision: `oa-updates` / `oa-updates-00118-lz7`
- Update ID: `3fc42779-c807-4b47-ab0e-d8b399774039`
- Launch asset: `index-96fa0474d98a99812adb1b859aad22fc.hbc`
- Launch asset bytes: `6,513,721`
- Rollback revision: `oa-updates-00117-5c7`

Production serves 100% from the new revision. The exact signed manifest and
launch asset returned HTTP 200, the manifest bound the exact installed runtime,
the launch asset returned `application/javascript`, a deliberately mismatched
runtime returned `noUpdateAvailable`, and the shared Desktop RC manifest still
returned HTTP 200.

## Verification

- Mobile focused tests: 15 passed
- Full mobile suite: 58 files, 288 tests passed
- Mobile typecheck: passed
- Behavior-contract suite: 36 passed
- Pre-push policy/mobile gate: passed
- OTA fingerprint equality gate: passed
- Signed production manifest and launch-asset retrieval: passed
- Runtime mismatch rejection: passed
- Desktop RC feed regression smoke: passed

## Attribution and authority

- Trigger kind: owner-directed production UX correction
- Trigger actor: authenticated owner
- Release actor role: operating agent and release operator
- Authority profile: `AUTHORITY.md` revision 5
- Program: `program.full_auto_release`
- Grant: `grant.autonomous_rc_release_and_communication`
- Source feedback: owner session, Sarah tool visibility request, 2026-07-19
- Outcome: succeeded

This was a JavaScript-only, fingerprint-compatible mobile OTA. It did not
promote an App Store stable release, create a new TestFlight binary, modify the
Desktop signed feed, or expand Sarah's authority.
