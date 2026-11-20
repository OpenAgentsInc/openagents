# OpenAgents Desktop Consolidation Audit

**Date:** 2026-11-19  
**Scope:** Consolidate `apps/coder-desktop/` into `apps/openagents-desktop/` (taking the **OpenAgents** product name, slug, bundle ID `com.openagents.desktop`, `OpenAgents.app`, and `apps/openagents-desktop/` path), deprecate/delete legacy desktop shells in `apps/openagents-desktop/`, and re-evaluate `apps/openagents-ios/` and `apps/shared/` under the consolidated architecture.

---

## 1. Executive Summary

### High-Level Recommendation

**Proceed with a phased consolidation** that promotes `apps/coder-desktop/` to become the canonical `apps/openagents-desktop/` (`OpenAgents.app`, `com.openagents.desktop`) on macOS, while **decoupling four non-obvious cross-repo dependencies before any deletion** of legacy `apps/openagents-desktop/` trees.

`apps/openagents-desktop/` is not a single application today—it is **five overlapping generations of desktop shells** living in one directory:
1. **Zeron / Metal-Vulkan Chat Shell** (top-level `build.sh`, `zeron/`, `zeron-face.c`, `face.c`, `Account.swift`, `dictation-*`, `linux/`).
2. **Native GPU Carrier Shell** (`native/gpu/`, compiling `apps/openagents-ios/Generated/gallery.c` + `apps/shared/native/*.swift` into `build/GPU/OpenAgents.app`).
3. **Native AppKit/GTK4 Shell** (`native/build.sh`, `native/build-linux.sh`, `native/program.coax`, `native/linux-services.coax`, `native/linux.c`).
4. **Legacy WASM Home Shell** (`home/`, `extras/`).
5. **Router/Test Harness Artifacts** (`router.coax`, `router-labels.tsv`, `route-face.c`, `chat_screen.coax`, `goldens/`, `tests/`).

Meanwhile, `apps/coder-desktop/` is the actively maintained, heavily evaluated (4-tier eval suite in `evals/coder-desktop/`), Coax-authored GPU desktop application (`page.coax` + `apps/shared/host/apple/ui-host.m` + Swift host + Microcoder worker + Foreman + Worktrees + Ed25519 signed `Updater.swift`). Promoting it into `apps/openagents-desktop/` eliminates four dead or redundant shells and unifies desktop product identity.

### Key Surprises & Blockers

1. **Everglade Workshop Hard-Depends on `apps/openagents-desktop/zeron/` + `linux/` + `lab-sign.coax`:**  
   `apps/everglade-desktop/build.sh:155-184` (and `apps/everglade-desktop/Sources/Workshop.swift:14`) embeds OpenAgents chat directly inside Everglade Workshop by compiling `apps/openagents-desktop/zeron/linux.coax`, `apps/openagents-desktop/lab-sign.coax`, `apps/openagents-desktop/zeron-face.c`, `apps/openagents-desktop/linux/chat.c`, `apps/openagents-desktop/linux/chat.h`, and `apps/openagents-desktop/linux/dictation-none.c`. Deleting `zeron/` or `linux/` without relocating or replacing Everglade's chat bridge will break `apps/everglade-desktop/build.sh` and `apps/shared/host/apple/check.sh`.
2. **`apps/shared/native/identity-check.coax` Hard-Depends on `apps/openagents-desktop/native/gpu/`:**  
   `apps/shared/native/identity-check.coax:7-15` hardcodes object outputs `apps/openagents-desktop/build/gpu/{carrier,gallery,host}.o` and header `apps/openagents-desktop/native/gpu/Bridge.h` to compile and run the standalone macOS XCTest for shared native identity/account logic. Deleting `native/gpu/` breaks this test harness unless the carrier/bridge is relocated into `apps/shared/native/` (or retired alongside the `apps/shared/native/` Swift bridge).
3. **`home/` + `extras/` vs. `apps/openagents-ios/` Reality:**  
   `apps/openagents-desktop/home/` and `apps/openagents-desktop/extras/` are legacy WASM/WebView host shells still referenced by `coax/native/generate.sh:11-12` and `coax/native/prepare.coax:7`. In reality, **`apps/openagents-ios/` (`com.openagents.app`) has already moved off WASM** to the compiled Coax GPU component architecture (`app.coax` -> `Generated/gallery.c` + `apps/shared/host/apple/ui-host.m` + `apps/shared/native/*.swift`). `home/` and `extras/` are obsolete once `coax/native/generate.sh` and `prepare.coax` are cleaned up.
4. **`Updater.swift` Already Speaks `openagents.desktop.update.v1`:**  
   `apps/coder-desktop/Updater.swift:4,187-195` already uses `schemaVersion = "openagents.desktop.update.v1"`, signing key ID `"openagents-desktop-2026-v1"`, and `pinnedTeamID = "HQWSG26L43"` (matching `proto/registry.json:32`). However, it still pins `pinnedBundleID = "com.openagents.coder"` (`Updater.swift:188`) and defaults its manifest URL to `coder-desktop-update.json` (`Updater.swift:190`). Changing `pinnedBundleID` requires a coordinated update-channel/manifest cutover so existing `Coder.app` installs either migrate cleanly or receive a final bridge release.
5. **`apps/coder-desktop` Already Borrows `OpenAgents.icns`:**  
   `apps/coder-desktop/build.sh:30` and `apps/coder-desktop/install.sh:35` already copy `apps/openagents-desktop/native/OpenAgents.icns` into `Coder.app/Contents/Resources/Coder.icns`. Moving `OpenAgents.icns` to the root of the consolidated `apps/openagents-desktop/` (or `resources/OpenAgents.icns`) is mandatory before deleting `native/`.
6. **CI & Host Checks Currently Gate Legacy `openagents-desktop`, Not `coder-desktop`:**  
   `apps/shared/host/apple/check.sh:26-39` builds `everglade-mac`, `everglade-iphone`, `openagents-mac` (`apps/openagents-desktop/build.sh` + `tests/account-check.sh` + `tests/sign-check.sh`), and `openagents-iphone`—it does **not** build `apps/coder-desktop`. Conversely, `coax/remote/check.sh:77,215-219,333-355,415,645` gates `evals/coder-desktop`, `desktop-units` (`zeron/linux.coax` + `lab-sign.coax`), and `native-style` (`apps/openagents-desktop/native/*.swift`). Both scripts must be updated in lockstep with the cutover.

---

## 2. Complete Inventory & Disposition of `apps/openagents-desktop/`

Every directory and top-level file in `apps/openagents-desktop/` is catalogued below with its role, exact repo-wide callers, and recommended disposition:

- **Fold** = Merge into the new `apps/openagents-desktop/` (incoming `coder-desktop`).
- **Relocate / Preserve** = Move to a neutral shared/consumer location (e.g., `apps/shared/` or `apps/everglade-desktop/`) or keep temporarily behind a clear deprecation boundary.
- **Deprecate & Delete** = Remove once callers/CI gates are updated.

### Top-Level Files in `apps/openagents-desktop/`

| Path | What It Does | Exact Repo-Wide Callers / References | Disposition |
|---|---|---|---|
| `.gitignore` | Ignores local `build/` outputs. | Local to `apps/openagents-desktop/`. | **Fold** (merge with `apps/coder-desktop/.gitignore`). |
| `Info.plist` | Production macOS bundle metadata: `com.openagents.desktop`, `OpenAgents`, v`1.0.0` (build `28829`), `OpenAgents.icns`, `NSMicrophoneUsageDescription` (`apps/openagents-desktop/Info.plist:5-29`). | `apps/openagents-desktop/build.sh`, `apps/openagents-desktop/native/gpu/build.coax:27`, `apps/openagents-desktop/native/build.sh`. | **Fold** — Merge `com.openagents.desktop`, `OpenAgents`, `OpenAgents.icns`, `CFBundleVersion >= 28830` (monotonic bump above `28829`!), and `NSMicrophoneUsageDescription` (if dictation is kept) with `apps/coder-desktop/Info.plist:5-76` (`UTExportedTypeDeclarations` for `.coax`, `CFBundleDocumentTypes`). |
| `README.md` | Documents legacy Zeron/Coax desktop build, push signing, and `com.openagents.desktop` preservation (`apps/openagents-desktop/README.md:24`). | Referenced in docs (`docs/desktop/coax-desktop.md`). | **Fold** (replace with consolidated OpenAgents Desktop README derived from `apps/coder-desktop/README.md`). |
| `sign.sh` | Two-step signing helper (`sign_prepare`, `sign_bundle`) supporting ad-hoc dev signing and provisioning-profile Push Notifications signing (`OA_MAC_SIGN_IDENTITY`, `OA_MAC_PROFILE`, `--options runtime`, `OA_PUSH_SANDBOX`, `apps/openagents-desktop/sign.sh:21-40`). | Sourced by `apps/openagents-desktop/build.sh` and tested by `apps/openagents-desktop/tests/sign-check.sh` (invoked by `apps/shared/host/apple/check.sh:34`). | **Fold** — Adopt into consolidated `build.sh`/`release.sh` and extend for Developer ID + notarization + stapling. |
| `Account.swift` | macOS Swift account bridge (`MacAccount`, `MacPush`, `UNUserNotificationCenterDelegate`, `registerForRemoteNotifications`, `AppAuth.pushTarget`, Security sheet; `apps/openagents-desktop/Account.swift:251-311`). | Compiled by `apps/openagents-desktop/build.sh`; tested by `apps/openagents-desktop/tests/account-check.swift` and `apps/shared/host/apple/check.sh:33`. | **Fold** — Wire into incoming OpenAgents Desktop when OpenAgents account login + APNs security notices are enabled. |
| `account-mac.h` | C bridge header exposing `Account.swift` entry points to C host. | Included by `apps/openagents-desktop/face.c` and `build.sh`. | **Fold** (if keeping C-to-Swift bridge) or **Deprecate & Delete** (incoming `coder-desktop` is Swift-hosted via `AppDelegate.swift` and can call `MacAccount` directly in Swift). |
| `build.sh` | Builds legacy Zeron macOS `OpenAgents.app` (compiles `zeron/linux.coax`, `lab-sign.coax`, `dictation-build.coax`, `face.c`, `zeron-face.c`, `Account.swift`, `apps/shared/host/apple/ui-host.m`). | Called by `apps/shared/host/apple/check.sh:32` and `apps/openagents-desktop/check.sh`. | **Deprecate & Delete** (replace with `apps/coder-desktop/build.sh` adapted for `OpenAgents.app` + `sign.sh`). |
| `check.sh` | Runs legacy desktop checks (`zeron/linux.coax`, `lab-sign.coax`, `router.coax`, `dictation-build.coax`, `home/`, `native/`, `tests/`). | Manual / local verification; parts mirrored in `coax/remote/check.sh`. | **Deprecate & Delete** (replace with consolidated check script wrapping `page.coax` tests, `worktree-test.sh`, `foreman-control-test.sh`, `sign-check.sh`, and `evals/coder-desktop/run.sh`). |
| `lab-sign.coax` | Standalone Coax unit implementing `coaxial_lab_nip98` NIP-98 signer for lab/auth requests. | **Active external callers:** `apps/everglade-desktop/build.sh:155-184`, `apps/openagents-desktop/linux/build.sh:23,40-42`, `coax/remote/check.sh` (`desktop-units`). | **Relocate / Preserve** — Move to `apps/shared/auth/lab-sign.coax` (or `coax/lib/nostr/`) so Everglade and future desktop/iOS builds do not depend on legacy desktop files. |
| `zeron-face.c` | C host glue bridging `zeron/linux.coax` state/actions to the GPU UI host. | **Active external caller:** `apps/everglade-desktop/build.sh:155-184`; also `apps/openagents-desktop/build.sh` and `linux/build.sh:47`. | **Relocate / Preserve** (with `zeron/` for Everglade until Everglade migrates off Zeron chat), then **Deprecate & Delete**. |
| `face.c`, `face.h` | Legacy macOS Zeron C face / window bridge. | `apps/openagents-desktop/build.sh`; referenced in comment at `apps/openagents-web/chat/chat-hud.coax:3`. | **Deprecate & Delete**. |
| `dictation-build.coax` | Coax dictation controller build unit wiring Psionic Parakeet Metal (`adopted_by` entry in Psionic manifest). | `apps/openagents-desktop/build.sh`, `apps/openagents-desktop/check.sh`, Psionic target manifest (`adopted_by: ["apps/openagents-cli/parakeet.coax", "apps/openagents-desktop/dictation-build.coax"]`). | **Fold** (if Parakeet dictation is carried into OpenAgents Desktop) or **Deprecate & Delete** (if dictation lives only in `apps/openagents-cli` initially; update Psionic manifest either way). |
| `dictation-face.c`, `dictation-face.h`, `dictation-mac.h`, `dictation-mac.m` | C / Objective-C macOS microphone capture + Parakeet Metal bridge. | `apps/openagents-desktop/build.sh`, `apps/openagents-desktop/linux/build.sh:47` (`dictation-face.c`). | **Fold** (if dictation is kept; `dictation-mac.m` provides `AVAudioEngine` capture) or **Deprecate & Delete**. |
| `chat_screen.coax` | Legacy standalone chat screen Coax module. | `apps/openagents-desktop/check.sh` / legacy Zeron shell. | **Deprecate & Delete**. |
| `router.coax`, `router-labels.tsv`, `route-face.c` | Legacy desktop local intent router experiment + C face. | `apps/openagents-desktop/check.sh`. | **Deprecate & Delete** (incoming `coder-desktop` uses Microcoder worker + `coax/lib/agent/router/`). |

### Directories in `apps/openagents-desktop/`

| Directory | What It Does | Exact Repo-Wide Callers / References | Disposition |
|---|---|---|---|
| `zeron/` | Legacy Zeron chat/composer/rooms/modes/settings/dictation Coax app (`linux.coax`, `claude.coax`, `modes.coax`, `room.coax`, `screen.coax`, `settings.coax`, `store.coax`, `dictation-linux.coax`). | **Critical external callers:** `apps/everglade-desktop/build.sh:155-184` (`zeron/linux.coax`), `apps/everglade-desktop/Sources/Workshop.swift:14`, `coax/remote/check.sh` (`desktop-units` tests `zeron/linux.coax`), `apps/openagents-desktop/linux/build.sh:22,29`. | **Relocate / Preserve (Short-Term), then Deprecate & Delete** — Move the minimal Everglade chat subset (`zeron/` + `zeron-face.c` + `linux/chat.{c,h}` + `linux/dictation-none.c`) into `apps/everglade-desktop/chat/` (or `apps/shared/legacy-chat/`) so `apps/openagents-desktop/` is cleanly unblocked. |
| `linux/` | Wayland + Vulkan Linux build of the legacy Zeron shell (`build.sh`, `main.c`, `chat.c`, `chat.h`, `stubs.c`, `dictation-host.c`, `dictation-none.c`, `README.md`). Installs `~/.openagents/bin/openagents-desktop` with `StartupWMClass=com.openagents.desktop` (`linux/build.sh:65-67`). | **Critical external callers:** `apps/everglade-desktop/build.sh:155-184` (`linux/chat.c`, `linux/chat.h`, `linux/dictation-none.c`), `apps/everglade-desktop/Sources/Workshop.swift:14`. | **Split:** Relocate `chat.c`, `chat.h`, `dictation-none.c` with Everglade's chat bridge; **Deprecate & Delete** the legacy Zeron Linux app once a Linux host for incoming OpenAgents Desktop is decided (see Section 3). |
| `native/` | Contains three distinct sub-items: (a) `OpenAgents.icns` + `entitlements.plist`; (b) legacy AppKit/GTK4 shell (`build.sh`, `build-linux.sh`, `program.coax`, `linux-services.coax`, `linux.c`); (c) `native/gpu/` (Metal GPU carrier compiling `apps/openagents-ios/Generated/gallery.c` + `apps/shared/native/*.swift` into `build/GPU/OpenAgents.app`). | (a) `apps/coder-desktop/build.sh:30` & `install.sh:35` (`OpenAgents.icns`); (b) `coax/remote/check.sh:645` (`native-style` regex on `apps/openagents-desktop/native/.*\.swift`); (c) `apps/shared/native/identity-check.coax:7-15` (`build/gpu/{carrier,gallery,host}.o`, `native/gpu/Bridge.h`). | **Split:** **Fold** `OpenAgents.icns` and review `entitlements.plist` into consolidated `apps/openagents-desktop/`; **Relocate** `native/gpu/` carrier/bridge into `apps/shared/native/` (if keeping `identity-check.coax`); **Deprecate & Delete** AppKit/GTK4 `program.coax`, `linux-services.coax`, `linux.c`, `build.sh`, `build-linux.sh`. |
| `home/` | Legacy WASM/WebView home shell (`app.coax`, `face.c`, `host.js`, `shell.coax`, `sync.coax`, `usage.coax`, `durable-checks.coax`, `README.md`). | `coax/native/generate.sh:11` (`home/app.coax`, `home/face.c`, `home/host.js`), `coax/native/prepare.coax:7`. | **Deprecate & Delete** (after removing references in `coax/native/generate.sh` and `coax/native/prepare.coax`). |
| `extras/` | Legacy WASM extras/wallet panel (`app.coax`, `index.html`, `host.js`). | `coax/native/generate.sh:12`, `coax/native/README.md:120`. | **Deprecate & Delete** (after removing from `coax/native/generate.sh`). |
| `tests/` | Legacy test suite (`account-check.swift`, `account-check.sh`, `sign-check.sh`, `zeron-defaults.c`, `zeron_source_extract.py`). | `apps/shared/host/apple/check.sh:33-34` runs `tests/account-check.sh` and `tests/sign-check.sh`. | **Split:** **Fold** `sign-check.sh` and `account-check.swift`/`.sh` into consolidated `apps/openagents-desktop/tests/`; **Deprecate & Delete** `zeron-defaults.c` and `zeron_source_extract.py`. |
| `goldens/` | Legacy visual/layout golden snapshots for Zeron shell. | Legacy `check.sh` only. | **Deprecate & Delete**. |

---

## 3. Audit of `apps/coder-desktop/` (The Incoming OpenAgents Desktop)

### Current Architecture & Capabilities

`apps/coder-desktop/` is a modern, self-contained macOS desktop application built on the repository's target GPU UI stack:

- **Coax UI Core (`page.coax`, `page/*.coax`, `settings.coax`):** Pure, effect-free Coax UI using the shared `components/app/` kit and 3-region responsive layout (`page/layout/layout.coax:19` mirroring `apps/shared/app/view/shell.coax`). Covers transcript rendering, Markdown, activity groups, composer, attachments, `@`-mentions, file pane, editor, command palette, review, follow-ups, goals, model switching, and persistence—backed by 15 Coax unit test suites (`page/checks/*-checks.coax`).
- **Shared GPU Host (`host.c`, `host.h`, `Bridge.h`, `apps/shared/host/apple/ui-host.m`):** Compiles `page.coax` to C and renders via Metal through the exact same `ui-host.m` shared by `apps/openagents-ios/` and `apps/everglade-desktop/`.
- **Swift macOS Host (`AppDelegate.swift`, `Window+Attach.swift`, `Store.swift`, `Worktree.swift`, `Foreman.swift`, `ForemanOperator.swift`, `DaemonClient.swift`, `Updater.swift`):**
  - Runs local Microcoder workers (`coder/tui/mcworker.coax`) over `mcworker-wire.coax`.
  - Manages per-chat git worktrees under `~/.openagents/worktrees` (`Worktree.swift`).
  - Connects to the shared local daemon socket `~/.openagents/coder/host/service.sock` (`DaemonClient.swift:3`), already designed so desktop and OpenAgents Mobile share host sessions.
  - Ships a Foreman automation controller (`Foreman.swift`, `ForemanOperator.swift`, `foreman-ctl`, `foreman-control-test.sh`).
  - Ships an Ed25519-verified, Apple-codesign-checked self-updater (`Updater.swift`).
- **4-Tier Evaluation Suite (`evals/coder-desktop/run.sh`, `tier1/`–`tier4/`):** Headless Coax replay tests, unit checks, and isolated UI automation (`evals/coder-desktop/tier3/run_ui.py:63` using bundle ID `com.openagents.coder.evals-t3`).

### Exact Delta & Gaps to Become `OpenAgents.app` (`com.openagents.desktop`)

| Area | Current State in `apps/coder-desktop/` | Required Change for `OpenAgents.app` (`com.openagents.desktop`) |
|---|---|---|
| **Bundle ID, Name, Version & Info.plist** | `com.openagents.coder`, `Coder`, `CFBundleShortVersionString = 0.1.0`, `CFBundleVersion = 1`, `Coder.icns`, exports `.coax` UTI (`apps/coder-desktop/Info.plist:5-76`). | Set `CFBundleIdentifier = com.openagents.desktop`, `CFBundleName = OpenAgents`, `CFBundleDisplayName = OpenAgents`, `CFBundleExecutable = OpenAgents`, `CFBundleIconFile = OpenAgents`, `CFBundleShortVersionString >= 1.0.0`, and **`CFBundleVersion >= 28830`** (strictly greater than legacy `apps/openagents-desktop/Info.plist`'s `28829` so `Updater.swift` and LaunchServices treat it as newer). Preserve `.coax` `UTExportedTypeDeclarations` and `CFBundleDocumentTypes`. |
| **App Icon (`OpenAgents.icns`)** | `build.sh:30` and `install.sh:35` copy `apps/openagents-desktop/native/OpenAgents.icns` to `$res/Coder.icns`. | Move `OpenAgents.icns` into the consolidated `apps/openagents-desktop/` (e.g., `resources/OpenAgents.icns`) and bundle it as `Contents/Resources/OpenAgents.icns`. |
| **Signing, Hardened Runtime & Notarization (`sign.sh`)** | `apps/coder-desktop/build.sh` only runs ad-hoc `codesign --force --sign -`. No provisioning profile or notarization flow in `build.sh`. | Fold `apps/openagents-desktop/sign.sh` (`sign_prepare`/`sign_bundle` with `--options runtime`, `OA_MAC_SIGN_IDENTITY`, `OA_MAC_PROFILE`, `OA_PUSH_SANDBOX`) and add a release notarization step (`xcrun notarytool submit --wait` + `xcrun stapler staple`) so Developer ID builds pass Gatekeeper and `Updater.swift`'s `SecStaticCodeCheckValidity` (`pinnedTeamID = HQWSG26L43`). |
| **Self-Updater (`Updater.swift`)** | `Updater.swift:187-195` already uses `schemaVersion = "openagents.desktop.update.v1"`, `pinnedTeamID = "HQWSG26L43"`, and key ID `"openagents-desktop-2026-v1"`, **but** pins `pinnedBundleID = "com.openagents.coder"` (`line 188`) and `defaultManifestURLString` to `coder-desktop-update.json` (`line 190`). Also toast string in `page/chats.coax:94` says `"Update ready: restart Coder to use it"`. | Change `pinnedBundleID` to `"com.openagents.desktop"`, point `defaultManifestURLString` to `openagents-desktop-update.json` (or canonical OpenAgents desktop feed), and update UI copy (`page/chats.coax:94`, `AppDelegate.swift`, menus) from "Coder" to "OpenAgents". **Migration note:** Existing `Coder.app` (`com.openagents.coder`) users cannot auto-update across a bundle ID change because `Updater.swift` rejects mismatched bundle IDs; publish a one-time notice or installer script in `install.sh` that replaces `/Applications/Coder.app` with `/Applications/OpenAgents.app`. |
| **`UserDefaults` Domain Migration** | `Foreman.swift:53`, `Worktree.swift:19` (`chatWorktrees`), `AppDelegate.swift` (`openRouterSub`, window/provider prefs), and `README.md:74` read/write `UserDefaults.standard` under `com.openagents.coder`. | Switching bundle ID moves `UserDefaults.standard` to `~/Library/Preferences/com.openagents.desktop.plist`. Add a one-time startup migration in `AppDelegate.swift` (guarded by a `migratedFromCoderDefaultsV1` flag) that reads `UserDefaults(suiteName: "com.openagents.coder")` and copies non-nil keys (`chatWorktrees`, Foreman tier, provider flags, etc.) into `UserDefaults.standard` if not already set. |
| **Local State & Socket Paths** | `DaemonClient.swift:3` uses `~/.openagents/coder/host/service.sock`; `Worktree.swift:19` uses `~/.openagents/worktrees`; `Store.swift` stores chats under `~/.openagents/...`. | **Keep `~/.openagents/worktrees` and `~/.openagents/coder/host/service.sock` stable in Phase 1** to avoid breaking `apps/openagents-ios` mobile host pairing and active worktrees. Rename internal socket/storage paths only as a separate, versioned protocol migration. |
| **Parakeet Dictation Decision** | `apps/coder-desktop/` has no dictation UI or `AVAudioEngine`/Parakeet integration today. Legacy `apps/openagents-desktop/` has `dictation-build.coax`, `dictation-face.{c,h}`, `dictation-mac.{h,m}`, and `NSMicrophoneUsageDescription`. | **Decision point:** Either (A) **Defer dictation** to a post-consolidation milestone (keep `dictation-mac.m`/`dictation-build.coax` out of Phase 1 to keep the cutover minimal, and remove `apps/openagents-desktop/dictation-build.coax` from Psionic's `adopted_by` list), or (B) **Fold dictation in Phase 2** by wiring `dictation-mac.m` + `apps/openagents-cli/parakeet.coax` into `page/compose.coax` and retaining `NSMicrophoneUsageDescription` in `Info.plist`. Recommendation: **Option A (Defer to Phase 2)** so consolidation ships cleanly without blocking on audio/Metal build coupling. |
| **OpenAgents Account, Passkey & Device-Link** | `apps/coder-desktop/` currently authenticates providers locally (API keys / Claude / Codex / OpenRouter) and does not integrate `apps/openagents-desktop/Account.swift`, `apps/shared/host/apple/AppAuth.swift`, or `apps/shared/native/KitPasskey.swift` / `AppDeviceLink.swift`. | Fold `Account.swift` + `AppAuth.swift` + `DeviceKey.swift` (and `tests/account-check.swift`) into consolidated `apps/openagents-desktop/` so OpenAgents account sign-in, NIP-98 auth, passkey (`webcredentials`), and device-link work natively in `OpenAgents.app`. |
| **Universal Links & APNs Push Alignment** | `apps/openagents-web/well-known/apple-app-site-association.json:1-8` lists `HQWSG26L43.com.openagents.desktop` under `webcredentials`, but **`applinks` only lists `HQWSG26L43.com.openagents.app`** (not desktop). `coax/web/api/push.c:13,82` already defaults `OA_APNS_TOPIC_MAC` to `com.openagents.desktop`. | 1. Preserve `com.openagents.desktop` so `webcredentials` (passkeys) and `OA_APNS_TOPIC_MAC` (`push.c:82`) continue working with zero backend changes. 2. Add `HQWSG26L43.com.openagents.desktop` to `applinks` in `apple-app-site-association.json` if macOS universal link handoff (e.g., login/device-link callbacks) is desired. 3. Wire `MacPush` (`Account.swift:285-311`) into `AppDelegate.swift` when signing with a push-enabled provisioning profile. |
| **Linux Story** | `apps/coder-desktop/` is macOS-first (`build.sh` compiles `apps/shared/host/apple/ui-host.m` + Swift AppKit host). However, `evals/coder-desktop/` already compiles `page.coax` headlessly with `-Iapps/shared/host/linux` (`evals/` grep). Legacy `apps/openagents-desktop/linux/` builds a Wayland+Vulkan binary using `apps/shared/host/linux/{ui-host,portal}.c`. | Because `page.coax` is pure Coax targeting the shared component ABI, porting OpenAgents Desktop to Linux requires a thin C host (`linux/main.c` + `apps/shared/host/linux/{ui-host,portal}.c` + `mcworker` process spawn) rather than rewriting UI. **Recommendation:** Do not block macOS consolidation on Linux; archive/deprecate `apps/openagents-desktop/linux/`'s Zeron build after relocating Everglade's `chat.{c,h}` files, and track a clean `page.coax` Wayland/Vulkan Linux host as a follow-up milestone. |
| **Eval & Install Infrastructure** | `evals/coder-desktop/run.sh:286`, `evals/coder-desktop/tier3/run_ui.py:63` (`com.openagents.coder.evals-t3`), `coax/remote/check.sh:77,215-219`, `install.sh` (`/Applications/Coder.app`). | Update `evals/coder-desktop/` paths to `apps/openagents-desktop/` (optionally renaming `evals/coder-desktop/` -> `evals/openagents-desktop/` in a follow-up PR to keep git blame clean), update `BUNDLE_ID` to `com.openagents.desktop.evals-t3`, and update `install.sh` to install `/Applications/OpenAgents.app`. |

---

## 4. Re-Evaluation of `apps/openagents-ios/` and `apps/shared/`

### `apps/openagents-ios/` Reality Check

- **Current Identity & Architecture:** `apps/openagents-ios/` (`com.openagents.app`, `OpenAgents.xcodeproj`, `project.yml`) is **not** a wrapper around `apps/openagents-desktop/home/`. It is a live, compiled Coax GPU iOS application (`app.coax` -> `generate.sh` -> `Generated/gallery.c` + `Generated/Components/`) rendered through `apps/shared/host/apple/ui-host.m` (Metal) and driven by Swift host files (`App.swift`, `AppCore.swift`, `ComponentsHost.m`, plus `apps/shared/native/*.swift` and `apps/shared/host/apple/{AppAuth,DeviceKey}.swift`).
- **Impact of Desktop Consolidation on iOS:**
  - **Zero breakage to `apps/openagents-ios/`** from deleting `apps/openagents-desktop/{home,extras,zeron,linux,native}`—*provided* `apps/shared/host/apple/` and `apps/shared/native/` remain intact.
  - **Unlocks True Desktop/iOS Convergence:** Now both `apps/openagents-desktop/` (formerly `coder-desktop`) and `apps/openagents-ios/` share the exact same rendering host (`apps/shared/host/apple/ui-host.m`), the same component kit (`components/app/`), the same daemon socket contract (`~/.openagents/coder/host/service.sock`, `DaemonClient.swift:3`), and the same Apple auth primitives (`apps/shared/host/apple/{AppAuth,DeviceKey}.swift`).
  - **Eliminates `native/gpu/` Duplication:** `apps/openagents-desktop/native/gpu/build.coax:10-29` existed as an experimental macOS runner for `apps/openagents-ios/Generated/gallery.c`. With `apps/coder-desktop/` becoming `OpenAgents.app`, `native/gpu/` is no longer needed as a desktop product candidate—its only remaining role is compiling `.o` files for `apps/shared/native/identity-check.coax:7-15`.

### `apps/shared/` Re-Evaluation

`apps/shared/` currently mixes active cross-platform infrastructure with legacy desktop-only artifacts. Post-consolidation, it should be cleanly partitioned:

1. **`apps/shared/host/apple/` (Keep & Promote — Tier-1 Shared Infrastructure):**
   - `ui-host.m`: Shared Metal/AppKit/UIKit GPU UI host used by `openagents-desktop` (incoming `coder-desktop`), `openagents-ios`, and `everglade-desktop`/`everglade-mobile`.
   - `AppAuth.swift`, `DeviceKey.swift`: Shared Apple Keychain/Secure Enclave/NIP-98 auth layer; should be adopted by the consolidated `apps/openagents-desktop/`.
   - `check.sh:26-39`: **Must be updated** to replace legacy `openagents-mac` (`apps/openagents-desktop/build.sh`) with the consolidated OpenAgents Desktop build + tests.
2. **`apps/shared/host/linux/` (Keep — Shared Wayland/Vulkan Host):**
   - Used by headless `evals/coder-desktop/` (`-Iapps/shared/host/linux`) and future Linux OpenAgents Desktop.
3. **`apps/shared/app/` (Keep & Converge):**
   - Shared Coax view/state modules (`view/shell.coax`, `subagent/view.coax`, `cases/`, `foreman/`, `login/`, `settings/`, `workspace/`). Already referenced by both `apps/coder-desktop/page/layout/layout.coax:19` and `apps/openagents-ios/project.yml:33`.
4. **`apps/shared/native/` (Audit & Decouple from `apps/openagents-desktop/native/gpu/`):**
   - Contains 16 Swift bridge files (`AppCoreC.swift`, `AppAccount.swift`, `KitPasskey.swift`, `AppDeviceLink.swift`, etc.) used by `apps/openagents-ios/` and `apps/openagents-desktop/native/gpu/build.coax:20-21`.
   - **Action Required:** Fix `apps/shared/native/identity-check.coax:7-15` so it no longer reaches into `apps/openagents-desktop/build/gpu/` or `apps/openagents-desktop/native/gpu/Bridge.h`. Move the minimal test carrier/bridge (`carrier.c`, `Bridge.h`) into `apps/shared/native/tests/` and compile into `apps/shared/native/build/`.
5. **Legacy Top-Level `apps/shared/*.coax` (`openagents-app.coax`, `chat-task-app.coax`, `projects-app.coax`, etc.):**
   - Audit callers after `home/` and `zeron/` deletion; prune any files only imported by deleted legacy shells.

---

## 5. Phased Consolidation & Cutover Plan

Execute in **four small, verifiable PRs** so `coax/remote/check.sh`, `apps/shared/host/apple/check.sh`, `everglade-desktop`, and `evals/coder-desktop` stay green at every commit.

### Phase 0: Decouple External Callers (Pre-Requisite PR)
1. **Everglade Chat Decoupling:** Copy/move `apps/openagents-desktop/zeron/`, `zeron-face.c`, `linux/chat.c`, `linux/chat.h`, `linux/dictation-none.c` into `apps/everglade-desktop/chat/` (or `apps/shared/legacy-chat/`) and relocate `lab-sign.coax` to `apps/shared/auth/lab-sign.coax`. Update `apps/everglade-desktop/build.sh:155-184`, `apps/everglade-desktop/Sources/Workshop.swift:14`, and `coax/remote/check.sh` (`desktop-units`).
2. **Identity Check Decoupling:** Move `apps/openagents-desktop/native/gpu/{carrier.c,Bridge.h}` (or a minimal test harness) into `apps/shared/native/tests/` and update `apps/shared/native/identity-check.coax:7-15` so it builds into `apps/shared/native/build/`.
3. **`coax/native/` Cleanup:** Remove obsolete `apps/openagents-desktop/home/` and `apps/openagents-desktop/extras/` entries from `coax/native/generate.sh:11-12` and `coax/native/prepare.coax:7`.
4. **Verify:** Run `apps/shared/host/apple/check.sh`, `apps/everglade-desktop/build.sh`, and `coax/web/tool.sh apps/shared/native/identity-check.coax`.

### Phase 1: Rebrand & Harden `apps/coder-desktop/` In-Place (Identity PR)
1. Copy `apps/openagents-desktop/native/OpenAgents.icns` and `apps/openagents-desktop/sign.sh` (plus `tests/sign-check.sh`) into `apps/coder-desktop/`.
2. Update `apps/coder-desktop/Info.plist`: `com.openagents.desktop`, `OpenAgents`, `OpenAgents.icns`, `CFBundleShortVersionString = 1.0.0`, `CFBundleVersion = 28830` (monotonic above `28829`).
3. Update `apps/coder-desktop/Updater.swift:188,190`: `pinnedBundleID = "com.openagents.desktop"`, update manifest URL to `openagents-desktop-update.json`, update user-facing "Coder" strings (`page/chats.coax:94`, menus, window titles) to "OpenAgents".
4. Add one-time `UserDefaults` migration in `AppDelegate.swift` from `com.openagents.coder` -> `com.openagents.desktop`.
5. Update `apps/coder-desktop/build.sh` and `install.sh` to build `OpenAgents.app`, invoke `sign.sh`, and install `/Applications/OpenAgents.app`.
6. Update `evals/coder-desktop/tier3/run_ui.py:63` (`com.openagents.desktop.evals-t3`).
7. **Verify:** Run `evals/coder-desktop/run.sh`, `apps/coder-desktop/worktree-test.sh`, `apps/coder-desktop/foreman-control-test.sh`, and `sign-check.sh`.

### Phase 2: Atomic Directory Swap & Legacy Deletion (Cutover PR)
1. Preserve `Account.swift`, `tests/account-check.swift`, `tests/account-check.sh` (either folded immediately or staged in `account/` for Phase 3).
2. Delete the remaining legacy contents of `apps/openagents-desktop/` (`home/`, `extras/`, `native/`, `linux/`, `zeron/`, `goldens/`, `face.*`, `router*`, `chat_screen.coax`, legacy `build.sh`/`check.sh`).
3. `git mv apps/coder-desktop/* apps/openagents-desktop/` (preserving git rename history).
4. Update all repo-wide path references:
   - `coax/remote/check.sh:77,215-219,333-355,415,645` (update `coder-desktop` paths, remove legacy `native-style` rule for `apps/openagents-desktop/native/.*\.swift`).
   - `apps/shared/host/apple/check.sh:32-34` (point `openagents-mac` at the new `apps/openagents-desktop/build.sh` + `sign-check.sh`).
   - `evals/coder-desktop/run.sh:286` (and tier1–tier4 scripts) to point at `apps/openagents-desktop/page.coax` and `OpenAgents.app`.
   - `.gitignore:9`, Psionic manifest (`adopted_by`), and active docs/READMEs.
5. **Verify:** Full `evals/coder-desktop/run.sh`, `apps/shared/host/apple/check.sh`, and `git grep 'apps/coder-desktop'` audit.

### Phase 3: Post-Consolidation Feature Parity (Follow-Up PRs)
1. Wire `Account.swift` + `AppAuth.swift` + `DeviceKey.swift` + `MacPush` (`UNUserNotificationCenter`) into `apps/openagents-desktop/AppDelegate.swift`.
2. Update `apps/openagents-web/well-known/apple-app-site-association.json` to add `HQWSG26L43.com.openagents.desktop` to `applinks` if universal links on macOS are required.
3. Add Developer ID notarization (`notarytool` + `stapler`) to `sign.sh` / release pipeline and publish `openagents-desktop-update.json`.
4. Decide & implement Phase 2 Parakeet dictation (if desired) and `page.coax` Wayland/Vulkan Linux host.

---

## 6. Risk Matrix & Verification Gates

| Risk | Severity | Mitigation & Verification Gate |
|---|---|---|
| Breaking `apps/everglade-desktop/build.sh` by deleting `zeron/` or `linux/` | **High** | Complete Phase 0 relocation first; gate PR on `apps/shared/host/apple/check.sh` (`everglade-mac`). |
| Breaking `apps/shared/native/identity-check.coax` by deleting `native/gpu/` | **High** | Relocate test carrier/bridge into `apps/shared/native/tests/` in Phase 0; run `coax/web/tool.sh apps/shared/native/identity-check.coax`. |
| Silent preference loss for existing `Coder.app` users | **Medium** | One-time `UserDefaults(suiteName: "com.openagents.coder")` migration in `AppDelegate.swift` with unit test. |
| Broken self-updater or version downgrade rejection | **High** | Bump `CFBundleVersion >= 28830` (above legacy `28829`), update `Updater.swift` `pinnedBundleID` + manifest URL, and run `sign-check.sh` + Updater unit tests. |
| Breaking `openagents-ios` host socket pairing | **Medium** | Keep `~/.openagents/coder/host/service.sock` and `~/.openagents/worktrees` unchanged during consolidation. |
| Stale CI triggers in `coax/remote/check.sh` | **Medium** | Update path regexes in `coax/remote/check.sh` in the same commit as `git mv` so `page.coax` changes still trigger `evals/coder-desktop`. |
