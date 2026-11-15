# Arcade account/settings — iOS 1088

The Account hub, profile editor, relays, privacy, notification settings,
keys/passkeys, protected backups, device sessions and resources use the shared
Coax UI and grayscale Arcade component scene. The drawer is a sliding 70-point
icon rail with 50-point controls; Recent chats retains the wider history pane.
The [source mapping](../../arcade/07-account-settings.md) and
[protocol](../../protocol/account-settings-v1.md) define the implemented scope.

Verification so far:

- Shared settings: 170 Coax/transitive checks pass. Native scene: 85 pass;
  workspace geometry/drag: 16 pass. The settings admission authority kills
  51/51 mutants; the changed profile-signing wrapper kills 11/11, with no
  survivors or equivalent exclusions.
- Real API/PostgreSQL, ASan/UBSan: 14 account/settings checks pass, covering
  anonymous/L1/stale-L2 refusal, revision CAS, unchanged data after refusal,
  actual derived backup identity, cache headers, public kind-zero content and
  lifecycle registration. The wrapper initially failed on the new gate's
  unregistered structured 401 response; its appended route/goldens and exact
  two-response replay now pass. Existing route indices remain unchanged.
- Frozen settings, backup and native action profiles match product codecs.
  Wire, component and layer gates pass without adding an allowlist entry.
- SDK checks cover hex/nsec input, invalid/zero/public-key refusal, Keychain
  preservation, private-input isolation, masked backup/clearing and failed
  proof refusing export. Navigation checks cover history/search, retained
  drafts, projects and world shortcuts. Existing Projects/Usage program checks
  also pass.
- The expanded native app's combined JSON-reader call chain exceeded its old
  1 MiB Wasm stack in history discovery. Its explicit 2 MiB stack fixes that
  installed-app failure within the unchanged 32 MiB carrier memory.

Simulator account data and exported screenshots are disposable fixture data.
Actual Apple passkey/Face ID ceremonies, real push receipt, physical-device
accessibility and power measurements remain in root NEEDS_OWNER.md.
The release retains the previous world producers and all wallet key records.
