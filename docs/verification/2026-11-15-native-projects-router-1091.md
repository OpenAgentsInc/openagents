# Private Projects and computer routing, iOS 1091

## Release

OpenAgents 1.0.0 (1091), `com.openagents.app`, is **VALID / IN_BETA_TESTING**.
App Store Connect explicitly verified membership in the existing **Owner** group
on November 15, 2026. The upload completed without errors. No external group or
public App Store release was submitted.

Source: `ebcb7ba3083f7c276add4b5e94347193b7af7a79`, tag
`openagents-ios-1091`. The archive's product inputs were frozen before upload;
the final source commit also contains the verified UI driver's keyboard fix.
The intervening main changes affect Bitcoin/cryptographic checks only.

- Runtime: `0e3432ead0dde542a994c1bf7528419a3c4b3f595c870a12145f548bd544969d`.
- IPA: 19,856,829 bytes; SHA-256
  `bd90cf4e6fda907b7221002e42190d15d1dec66497cc677b2b6bd7b3f3648272`.
- Archive, IPA, generated resources, signed package and detailed logs:
  `~/.openagents/releases/openagents-ios-1091/` (private local artifacts).

## Behavior

The installed app creates private Projects, edits instructions, adds text or
supported Markdown/text-file knowledge, and starts, continues and reopens saved
Project chats. The CLI and phone use the same account-owned API. The existing
public draft/review/publish workflow remains available separately. See the
[API bounds](../protocol/projects-api-v1alpha1.md) and
[live CLI/API evidence](2026-11-15-projects-cli.md).

Account chat includes main's live computer reports and routing preview. The
phone sends available memory/disk, load, hardware and signed-in coding agents
to the router and displays the chosen computer, agent, model and effort.
**Routing does not yet start work on the chosen computer.** This release
does not claim that a preview is an executed task.

## Verification

- The app's Coax suite passed all 697 checks on a remote build host. The existing
  native runtime oracle passed JavaScript/Wasm frame and action comparisons for
  app and extras.
- Eleven native Project/navigation checks passed, together with the installed
  public draft/publish/chat UI flow. The new private Project UI flow passed
  through the real native HTTP adapter and an independent local fixture:
  create, edit instructions, add knowledge, complete a chat, save and reopen.
  The fixture observes both changed instructions and knowledge in model input.
- The installed routing UI check passed, including real-shaped Mac report
  facts and the unavailable-report fallback for CoderOS.
- The first private UI run failed because tapping Add knowledge while the
  keyboard was disappearing only dismissed the keyboard. The test driver now
  dismisses it and waits before tapping; the targeted retry passed in 38 seconds.
  A diagnostic run was canceled. No app workaround or claim of an entirely
  passing first invocation is made; unchanged passing slices were retained.
- Release signing, archive and IPA export succeeded. The archive-level link
  check found zero missing symbols in all four world frameworks. All five
  matching dSYMs were retained locally and in the existing symbols bucket.
- All twenty resources in the archived Starter directory match the signed
  generation-1 manifest by size and SHA-256. Both configured public update
  origins returned the exact published signed envelope. The two newly generated
  world base packs were uploaded as immutable content-addressed objects before
  publication; their public sizes were checked. Existing objects were retained.
- A fresh installed app then passed
  `StarterUITests/testIdenticalLivePackageDoesNotPrompt` against the live signed
  feed, including relaunch and explicit checks. It offers no redundant update
  for resources already bundled in 1091. This brings the focused native total
  to fifteen distinct passing checks across the retained slices.

Native tests use a dedicated headless iPhone 17 Pro / iOS 26.5 simulator and
synthetic fixture accounts. Real staging and owner-account inference evidence
belongs to the linked CLI/API record; no private account content is committed.
Physical-phone touch, document-picker consent and appearance remain owner
observations, recorded in [NEEDS_OWNER.md](../../NEEDS_OWNER.md).
