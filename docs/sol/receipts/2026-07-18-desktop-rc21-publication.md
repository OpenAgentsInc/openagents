# OpenAgents Desktop RC21 experimental publication receipt

Date: 2026-07-18

## Outcome

OpenAgents Desktop `0.1.0-rc.21` is public as an immutable experimental
GitHub prerelease:

- release: https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.21
- source revision: `5e155e91d0ae08d24e657b16009cf0768ebb305c`
- publication class: `desktop_experimental_prerelease`
- published assets: 9
- signed Desktop feed promoted: no
- stable channel promoted: no

The release exists to put the accepted Full Auto implementation in testers'
hands without misrepresenting the incomplete signed-feed matrix.

## Authority and attribution

- trigger: `owner_direction`
- triggered by: OpenAgents owner through issue #8995 and the 2026-07-18
  direction to publish releases autonomously
- release actor: OpenAgents release operator
- authority profile: `openagents.owner-delegated-autonomy` revision 2
- program: `program.full_auto_release`
- grant: `grant.autonomous_rc_release_and_communication`

This grant admits experimental and verified RC publication, candidate test
requests, bounded GitHub and Forum communication, feedback intake, changelog
publication, and rollback. It does not admit stable promotion.

## Immutable artifact manifest

| Target | Artifact | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| darwin-arm64 | `OpenAgents-0.1.0-rc.21-rc-darwin-arm64.zip` | 191751252 | `ce69162cc030398b45f14c8a4743a368c5164097513a8d30d94a40e8b49cf3a4` |
| darwin-x64 | `OpenAgents-0.1.0-rc.21-rc-darwin-x64.zip` | 200095469 | `a2a15caa54ead1ec668256f92539a907e49c3bdfe881c845cc8c7b2ba88b2251` |
| linux-arm64 | `OpenAgents-0.1.0-rc.21-rc-linux-arm64.AppImage` | 208692501 | `cc0f7c754f11878fc99adcb5155e114ba9537fc113d58ac04878483026903701` |
| linux-arm64 | `OpenAgents-0.1.0-rc.21-rc-linux-arm64.deb` | 154543572 | `a0ab24c5b1de60aa087161b3745745153e7bcea928b94b18e6e29dfac7a720da` |
| linux-arm64 | `OpenAgents-0.1.0-rc.21-rc-linux-arm64.rpm` | 155574893 | `d6d9ffb558500b95edce34f1bba10cb25f3e9ef5d77bb294639510447794607a` |
| linux-x64 | `OpenAgents-0.1.0-rc.21-rc-linux-x64.AppImage` | 209113802 | `2e54b074105b5b2ea31f2758cbf177e896f3055d4c8f8a7e14aa52a33a013ff8` |
| linux-x64 | `OpenAgents-0.1.0-rc.21-rc-linux-x64.deb` | 156123258 | `0a8f53a55108015ba7bee7bd6f5ad74d2f2b5556ca08922646337bbfa600bd86` |
| linux-x64 | `OpenAgents-0.1.0-rc.21-rc-linux-x64.rpm` | 164064305 | `b6861c1570ee891d7b3f25c8202d9325356585e561836ddc6de2be519e2744be` |
| win32-x64 | `OpenAgents-0.1.0-rc.21-rc-win32-x64-portable.zip` | 226590116 | `847c9d2ba96bd7e183d7d07b817930b498f6a6d346cf209e003f76a8c944eee1` |

GitHub reported the same byte lengths and `sha256:` digests for all nine
assets before the draft was made public. The publisher refuses in-place asset
replacement and release-version reuse.

## Build and verification evidence

- The RC21 source preparation push passed the full Desktop gate: 252 test
  files, 2542 tests passed, 39 skipped, typecheck, production build, fixture
  Electron smoke, React one-click Full Auto smoke, and lifecycle teardown.
- Every platform build consumed the exact exported RC21 source through the
  target staging contract and passed the post-package ASAR gate: 37 ASAR
  entries, 30 unpacked entries, and 2 closure components byte-verified.
- Staged ledger references were:
  - darwin-arm64:
    `sha256:ee1cbab25b81951b6db14014a6cd136acdc616499286ca96ebe7c4c28b379948`
  - darwin-x64:
    `sha256:913fb3e487ce718f5854d4af60340b12bddedc9840e9421910d002636808ff8f`
  - linux-arm64:
    `sha256:c08b345f0ba394b0f215aa5064caa12a1cd41018233d8ce00ddc6219f70dff79`
  - linux-x64:
    `sha256:a483693c2abf2436830d29f7b2bedfab726d1a1b30968d9cfa45d18498954188`
  - win32-x64:
    `sha256:dbfd227fd2a88c17ed242a3bfa7a5448eccdd0aa2389876b5c6f773ac9035432`
- Apple accepted notarization submissions
  `55e60e19-8e9a-41a7-95ac-b24a2d221684` and
  `74f3bfc7-ba45-43c9-8f59-47f1b5e01d4a`. Both app bundles carry stapled
  tickets, pass deep strict code-sign verification, and are accepted by
  Gatekeeper as `Notarized Developer ID`.
- Linux DEB and RPM metadata matched their declared native architectures.
  Extracted AppImage executables were independently identified as AArch64 and
  x86-64 respectively.
- The Windows executable's PE machine was `0x8664` and the portable archive
  passed integrity and packaged-Codex exclusion checks. Its Authenticode state
  is `NotSigned`, which is disclosed rather than promoted.
- All nine final archives passed the packaged-Codex exclusion oracle. Provider
  CLIs remain discovered from the user's environment at runtime.

## Experimental limitations

- The signed Desktop ReleaseSet and update feed were not promoted.
- Windows x64 is an unsigned portable ZIP, not an Authenticode-signed NSIS
  installer.
- The local macOS DiskImages service refused DMG creation with
  `DIHLDiskImageCreate` error 6. The release uses notarized ZIPs instead.
- The Intel macOS artifact is cross-built and lacks an Intel-native
  install/update acceptance receipt.
- Linux packages are native candidate downloads, not signed-feed support
  evidence.

These conditions prohibit calling RC21 a stable release or a complete
five-target signed ReleaseSet.

## Communication and feedback receipts

- Forum candidate:
  https://openagents.com/forum/t/d15df49f-157e-48ea-914e-bf6b7c679630
- tester feedback source:
  https://github.com/OpenAgentsInc/openagents/issues/8995#issuecomment-5011754100
- autonomous release command:
  https://github.com/OpenAgentsInc/openagents/issues/8926#issuecomment-5011754165
- signed publication closure:
  https://github.com/OpenAgentsInc/openagents/issues/8993#issuecomment-5011754233
- release coordinator:
  https://github.com/OpenAgentsInc/openagents/issues/8917#issuecomment-5011754310

The candidate request names `@lathe-agent-oa` and asks for a bounded
`PASS | BLOCKED` reply with severity and observation. Combined GitHub and
Forum feedback intake inspected zero post-request replies, acknowledged zero
passes, and created zero follow-up issues. Later intake is idempotent on both
channels and will acknowledge a pass where it was posted or create one linked
GitHub follow-up issue for a blocked or unstructured tester reply.
