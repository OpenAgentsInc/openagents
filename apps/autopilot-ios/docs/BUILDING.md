# Building the Autopilot iOS App

This doc is for developers and coding agents: how to get the app building, where artifacts live, and common failures.

## Prerequisites

- Xcode (with iOS SDK)
- Rust toolchain (`rustup`); iOS targets are added by the build script if missing
- Scripts: `cargo`, `xcodebuild`, `lipo`, `shasum`, `strings`, `rg`, `git`

## How the app gets its Rust code

The app does **not** compile Rust in the Xcode build. It links a **prebuilt XCFramework** (and static libs) produced by a separate script:

- **Build script:** `apps/autopilot-ios/scripts/build-rust-client-core.sh`
- **Output root:** `apps/autopilot-ios/Autopilot/RustCore/`
- **Versioned dirs:** `RustCore/v<crate-version>-<git-short>/` (e.g. `v0.1.0-1620e730baca`)
- **Xcode uses:** `RustCore/current/` — a **symlink** that must point to a version dir that contains `OpenAgentsClientCore.xcframework`

Xcode expects:

- `RustCore/current/OpenAgentsClientCore.xcframework` (with slices `ios-arm64`, `ios-arm64_x86_64-simulator`)
- Optionally `RustCore/current/libopenagents_client_core_sim.a` etc. for explicit linking (see project `LIBRARY_SEARCH_PATHS` and `OTHER_LDFLAGS`)

## Building the Rust artifact (XCFramework)

From the **repo root**:

```bash
./apps/autopilot-ios/scripts/build-rust-client-core.sh --clean
```

- **`--clean`** — Remove existing version/build dirs before building (recommended if you changed Rust or want a clean slate).
- Builds three targets: `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios`, then creates the XCFramework and a **new** versioned directory.
- On success, the script updates `RustCore/current` to point at the new version and writes `RustCore/LATEST_VERSION`.
- **Duration:** Several minutes (full dependency compile for each target).

After a successful run, open the project in Xcode and build the app as usual.

## "No XCFramework found" at `RustCore/current/OpenAgentsClientCore.xcframework`

This means either:

1. **`current` points at a version that was never fully built** (e.g. only an empty `Headers` folder), or
2. **The build script was never run** (no version dirs), or
3. **`current` was removed or broken.**

**Fix options:**

- **Option A — Rebuild and let the script set `current`:**
  Run from repo root:
  ```bash
  ./apps/autopilot-ios/scripts/build-rust-client-core.sh --clean
  ```
  Wait for it to finish; it will create a complete version dir and set `current` to it.

- **Option B — Point `current` at an existing complete version:**
  If you have a version dir that already contains `OpenAgentsClientCore.xcframework` (e.g. from a previous successful build):
  ```bash
  cd apps/autopilot-ios/Autopilot/RustCore
  rm -f current
  ln -s <version-dir> current   # e.g. ln -s v0.1.0-1620e730baca current
  echo "<version-dir>" > LATEST_VERSION
  ```
  Then the Xcode project will find the XCFramework at `RustCore/current/OpenAgentsClientCore.xcframework`.

## What’s in the artifact

- **FFI:** Client-core C symbols (e.g. `oa_client_core_normalize_email`, `oa_client_core_free_string`) — see `rust-client-core-integration.md`.
- **WGPUI:** The same static lib includes WGPUI when built with the `ios` feature; symbols `wgpui_ios_background_create`, `wgpui_ios_background_render`, `wgpui_ios_background_resize`, `wgpui_ios_background_destroy` are used by the Swift WGPUI bridge. If you add or change WGPUI iOS code in `crates/wgpui` or `crates/openagents-client-core`, rebuild the artifact so the app gets the new code.

## Building the app in Xcode

1. Ensure `RustCore/current/OpenAgentsClientCore.xcframework` exists (see above).
2. Open `apps/autopilot-ios/Autopilot/Autopilot.xcodeproj` in Xcode.
3. Select the Autopilot scheme and a simulator or device; build and run.

No Rust build phase is required in Xcode; linking uses the prebuilt XCFramework and static libs (see project’s Frameworks and Build Settings).

## References

- **Rust client core packaging and FFI:** `rust-client-core-integration.md`
- **WGPUI on iOS (for agents):** `WGPUI-IOS.md`
- **Cutover log:** `../../../docs/autopilot/wgpui-ios-cutover-log.md`
