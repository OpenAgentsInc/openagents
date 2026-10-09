# Releasing OpenAgents for Mac, Linux, and Windows

How to turn the desktop app into the `.dmg` people download: signed with
the OpenAgents Developer ID, hardened runtime, notarized by Apple, stapled,
with an **Applications** shortcut to drag the app onto. Part of
[#9965](https://github.com/OpenAgentsInc/openagents/issues/9965), step 7
([#9972](https://github.com/OpenAgentsInc/openagents/issues/9972)); the
design is [auto-pairing](../coder/design/2026-09-29-auto-pairing.md).

It is one manual script, `scripts/desktop/package-macos.sh`, run on a Mac.
There is no GitHub automation.

## What you need

| Need | Check | Notes |
| --- | --- | --- |
| Xcode command line tools | `xcrun notarytool --version` | `notarytool`, `stapler`, `codesign`, `lipo`, `hdiutil`. |
| Both Rust targets | `rustup target list --installed` | `rustup target add aarch64-apple-darwin x86_64-apple-darwin`. |
| The Developer ID Application certificate, with its private key, in the login keychain | `security find-identity -v -p codesigning` lists `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)` | Only the Apple Developer account holder can create one (developer.apple.com, Certificates, +, Developer ID Application). Expires 2031-06-16. |
| Notarization credentials | see below | An App Store Connect API key, or a saved `notarytool` profile. |

Notarization credentials, first match wins:

- `NOTARY_KEYCHAIN_PROFILE`: a profile saved once with
  `xcrun notarytool store-credentials <name> --key <AuthKey.p8> --key-id <id> --issuer <issuer>`
  (or with an Apple ID and an app-specific password).
- `ASC_API_KEY_ID`, `ASC_API_ISSUER_ID`, `ASC_API_PRIVATE_KEY_PATH`: the
  App Store Connect API key (`AuthKey_<id>.p8`) that also uploads TestFlight
  builds. `--notary-env FILE` sources these from an env file, for example the
  operator's local `appstoreconnect.env`. Keep the `.p8` out of the repo and
  out of logs; the script passes only its path.

## Version

The desktop app ships at the OpenAgents phone app's version, in lockstep
(`INVARIANTS.md`, App versions). The one source of truth is
`MARKETING_VERSION` in `bins/openagents-ios/host/project.yml`, the
TestFlight version, which the Android build also reads. The packaging
scripts read it, and `crates/openagents-desktop/Cargo.toml`'s `version`
(the updater's running version) and `bins/openagents-desktop-macos/Info.plist`
must equal it: the scripts refuse a mismatch, and
`cargo test -p openagents-desktop --test version_lockstep` fails on one.
To change the version, change `MARKETING_VERSION`, the desktop crate's
`version`, the Mac `Info.plist`, and Android's fallback `versionName` in
`build.gradle.kts` together.

The first public desktop build was 0.1.0; 1.0.0 is the first in lockstep.
The updater compares versions as semver, so installed 0.1.0 apps take 1.0.0.

## Release

```sh
scripts/desktop/package-macos.sh --notary-env ~/path/to/appstoreconnect.env
```

It writes `target/desktop-release/OpenAgents.app` and
`target/desktop-release/OpenAgents-<version>.dmg`, and prints the
submission IDs, signing authority, architectures, and the `.dmg`'s SHA-256.
A run takes the release builds plus two notarization round trips (usually
one to five minutes each).

Then publish the `.dmg` and the signed update manifest:

```sh
v=1.0.0   # MARKETING_VERSION
scripts/desktop/sign-manifest.sh --version $v --app target/desktop-release/OpenAgents.app --upload
gcloud storage cp target/desktop-release/OpenAgents-$v.dmg \
  gs://openagentsgemini-oa-updates/desktop/macos/$v/OpenAgents-$v.dmg
```

`sign-manifest.sh` zips the app, signs the manifest with the Ed25519 key
kept outside the repository, uploads the zip to `desktop/macos/<version>/`,
and then `desktop/macos/manifest.json`. The download link is
`https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/<version>/OpenAgents-<version>.dmg`;
the website's download table (`DOWNLOADS` in
`crates/openagents-web/src/pages/download.rs`) points at it.

What it does, in order:

1. **Build.** `cargo build --release --locked` of `openagents-desktop`,
   `coder`, `microcoder`, and `openagents` (the `openagents-cli` package) for
   `aarch64-apple-darwin` and `x86_64-apple-darwin`, with
   `MACOSX_DEPLOYMENT_TARGET=13.0` (the floor for `SMAppService`), joined
   into universal binaries with `lipo`. The build sets
   `OPENAGENTS_DESKTOP_RELEASE=1`, which marks it as the release: only a
   build with that flag uses the signed app's keychain items
   (`com.openagents.desktop.verse`) and runs `coder host adopt`. Any other
   build, `cargo build --release` included, is a dev build that keeps its
   own items under `com.openagents-dev.desktop.verse`, so testing a dev
   build never prompts for or touches the real ones
   ([#10096](https://github.com/OpenAgentsInc/openagents/issues/10096)).
   Binaries passed with `--bin-dir` must be built with the flag too, or
   the release would read a fresh dev world key instead of the person's.
2. **Assemble** `OpenAgents.app`:

   ```text
   OpenAgents.app/Contents/
     Info.plist                      com.openagents.desktop, the phone app's version
     MacOS/OpenAgents                the window and menu bar
     MacOS/coder                     runs `coder host serve` as a launchd agent
     MacOS/microcoder
     Helpers/openagents              the command a phone's command card runs;
                                     the host puts Helpers/ first on its
                                     terminals' PATH (not in MacOS/, where a
                                     case-insensitive volume makes it
                                     OpenAgents)
     Library/LaunchAgents/com.openagents.desktop.host.plist
     Resources/AppIcon.icns
   ```

   `Info.plist`, `com.openagents.desktop.host.plist`, and the entitlements
   files come from `bins/openagents-desktop-macos/`, the same files the quick
   local build (`bins/openagents-desktop-macos/bundle.sh`) uses; the
   version is the phone app's (see [Version](#version)) and the build
   number is the commit count, as there. The icon is `AppIcon.icns` from that folder if present, otherwise
   scaled from the iOS app icon. Missing files fall back to defaults
   written by the script.
3. **Sign**, inner code first and never with `--deep`: every other Mach-O in
   the bundle (`coder`, `microcoder`, any dylib or framework) and then the
   app, each with `--options runtime --timestamp` and the Developer ID. A
   helper's code-signing identifier is `<bundle id>.<name>`
   (`com.openagents.desktop.coder`), as `bundle.sh` signs it.
4. **Notarize the app**: `ditto` zip, `xcrun notarytool submit --wait`, then
   `xcrun stapler staple` so the app carries its ticket after it is copied
   out of the `.dmg` and opened offline.
5. **Build the `.dmg`**: the stapled app and an `Applications` symlink,
   `hdiutil create -format UDZO`, signed with the Developer ID, notarized,
   stapled.
6. **Check**: `codesign --verify --strict`, `spctl --assess --type execute`
   on the app, `spctl --assess --type open --context context:primary-signature`
   on the `.dmg`, `stapler validate` on both. Any failure stops the script;
   a rejected submission prints Apple's notarization log.

## Entitlements

The app and the embedded host get the same small set by default:

| Key | Why |
| --- | --- |
| `com.apple.security.network.client` | The host dials the iroh relay and the Nostr relay; the app talks to the host. |
| `com.apple.security.network.server` | The host accepts phones on its iroh and direct listeners. |
| `com.apple.security.cs.allow-unsigned-executable-memory` | Plugins run as Wasm under wasmtime, which compiles each module to native code at run time and makes those pages executable. Without this key the hardened runtime kills the process (exit 137) the first time a plugin runs. Every binary in the bundle links wasmtime. |

Outside the App Sandbox the network keys are not enforced; they are there so
the bundle keeps working if it is ever sandboxed. The executable-memory key is
the one hardened-runtime exception, and it is enforced: it was missing until
2026-10-01, when the release acceptance gate's `explain-error` scenario
caught a signed app that could not run any plugin (debug builds aren't
hardened, so no earlier test saw it). `allow-jit` alone does not help;
wasmtime does not use `MAP_JIT`. Add any other exception only with a reason
in the entitlements file.

**Keychain.** The login keychain (`keyring` with service
`com.openagents.desktop`) needs no entitlement for a Developer ID app that is
not sandboxed. Do not add `keychain-access-groups`: it requires a
provisioning profile, and a Developer ID app carrying it without one is
killed at launch. Keychain items are bound to the signer's designated
requirement (the OpenAgents team ID and bundle identifier), so a new
release signed with the same Developer ID keeps reading the keys the old one
wrote and paired phones reconnect. Signing a build ad hoc, or with another
team, loses that access.

`bins/openagents-desktop-macos/OpenAgents.entitlements` and
`host.entitlements`, when present, replace the defaults for the app and for
the helpers in `Contents/MacOS`.

## Options for testing

| Flag | Use |
| --- | --- |
| `--adhoc` | Sign ad hoc (`codesign -s -`), no notarization. For a Mac without the certificate; Gatekeeper refuses the result on any other Mac. Chosen automatically when no Developer ID identity is in the keychain. |
| `--no-notarize` | Developer ID signature, no Apple round trip. |
| `--native` | Build only this Mac's architecture; the `.dmg` name gets the architecture. Not for release. |
| `--app PATH` | Sign, notarize, and package an existing `.app` instead of building one. |
| `--bin-dir DIR` | Assemble the bundle from prebuilt `openagents-desktop`, `coder`, `microcoder`, and `openagents` in `DIR` instead of running Cargo. |
| `--identity ID` | Another signing identity (name or SHA-1). |
| `--out DIR`, `--volname NAME` | Output folder and the `.dmg` volume name. |

`DESKTOP_PACKAGE` and `DESKTOP_BIN` name the app's Cargo package and binary
(default `openagents-desktop`).

## Verify a release

On the release Mac the script's checks must pass. Then, on a second Mac
that has never run a development build:

1. Download the `.dmg` through a browser (so it gets the quarantine flag).
2. Open it: no warning. Drag **OpenAgents** onto **Applications**.
3. Open OpenAgents from Applications: no "cannot be opened" or "downloaded
   from the internet" dialog beyond the standard first-open confirmation
   naming OpenAgents, Inc.

To check a downloaded `.dmg` from a terminal:

```sh
spctl --assess --type open --context context:primary-signature -vv OpenAgents-<version>.dmg
xcrun stapler validate OpenAgents-<version>.dmg
```

Both should print `accepted` / `source=Notarized Developer ID` and
`The validate action worked!`.

## Release candidates

A release candidate is a signed, notarized, stapled `.dmg` for testing
that installed apps never see. It goes under
`desktop/macos/rc/<name>/` in `openagentsgemini-oa-updates`, and
`sign-manifest.sh` is not run, so `desktop/macos/manifest.json` (which
installed apps follow) does not change.

The bundle stays at `MARKETING_VERSION`: the lockstep rule (App versions)
allows no `-rc.N` in `CFBundleShortVersionString` or the desktop crate, and
the updater compares versions as semver, so an rc string would sort below
the release it precedes. The candidate is told apart by its
`CFBundleVersion` (the commit count, as every build has) and by the
artifact's name, which carries the rc:

```sh
scripts/desktop/package-macos.sh --notary-env ~/work/.secrets/appstoreconnect.env
gcloud storage cp --no-clobber target/desktop-release/OpenAgents-1.0.0.dmg \
  gs://openagentsgemini-oa-updates/desktop/macos/rc/1.0.0-rc.2/OpenAgents-1.0.0-rc.2.dmg
```

| Candidate | Bundle version | `CFBundleVersion` | Commit | Date |
| --- | --- | --- | --- | --- |
| `1.0.0-rc.2` | 1.0.0 | 28828 | `68662bd344` | 2026-10-01 |

`1.0.0-rc.2`
([#10126](https://github.com/OpenAgentsInc/openagents/issues/10126)),
`https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/macos/rc/1.0.0-rc.2/OpenAgents-1.0.0-rc.2.dmg`,
SHA-256 `8e3e6764c3299b976241b70d3f884b075742d5aa7eaf1ed03ccd3bcfa01514de`.
Universal; notarization `a8933968-3f82-4929-92af-67748c91c7ef` (app) and
`e5f72d18-2a3e-4e9b-a4c3-d5152b99e8e3` (`.dmg`), both `Accepted`. The
`.dmg` downloaded from that URL with a quarantine flag was accepted by
`spctl` (`Notarized Developer ID`) and mounted, the app copied out of it
was accepted, and `stapler validate` passed on both. The release
acceptance gate on that copy passed 29 of 30 scenarios; `delegate-grok`
failed because Grok Build answered by reading the files without running a
shell command, which the scenario then required. That was the check's
mistake, not the app's: a test delegation is a read-only question, so
reading the project with Grok Build's own tools is the right answer. The
scenario has counted any completed Grok Build tool call as work in the
project since `f5e4aaba20`
([#10236](https://github.com/OpenAgentsInc/openagents/issues/10236),
with a recorded read-only trajectory as its test fixture), so it is not a
1.0 blocker; the release run's gate checks it again.

## Release day: 1.0.0 (#11092)

Desktop ships as **1.0.0** (owner, 2026-10-09), at the version already in
the lockstep files. A 1.0.0 build was uploaded on 2026-09-30 to
`desktop/macos/1.0.0/` and `desktop/linux/1.0.0/`, and both platforms'
`manifest.json` name it, but it was never released. It moves to a
`1.0.0-rc.0` label and the real 1.0.0 takes its place. Windows has no
earlier upload outside `desktop/windows-test/`.

One thing this can't fix: an app that already took the 2026-09-30 build
as an update is at 1.0.0, so the updater (which compares versions) does
not offer it the real 1.0.0. Those installs move with the next version,
or by downloading 1.0.0 again from the download page.

Each step names the computer it runs on. `CLOUDSDK_CONFIG=...` is an
account that can write `openagentsgemini-oa-updates`.

1. **Build**, from a clean checkout of the release commit:
   - Mac (the release Mac, with the Developer ID and notarization
     credentials): `scripts/desktop/package-macos.sh --notary-env ~/work/.secrets/appstoreconnect.env`
   - Linux (any x86_64 Linux computer with Docker):
     `scripts/desktop/build-linux-release.sh --out /tmp/openagents-linux-1.0.0`,
     then copy that folder to the release Mac.
   - Windows (any Mac or Linux computer with the MinGW target and `wixl`):
     `scripts/desktop/package-windows.sh --out /tmp/openagents-windows-1.0.0`
     (unsigned: no `--pfx` and no `--require-signing`; see Authenticode
     below), then copy that folder to the release Mac.
2. **Gate** the signed app (release Mac):
   `scripts/release/acceptance.sh --app target/desktop-release/OpenAgents.app`.
   Every scenario must pass (`docs/release/acceptance.md`).
3. **Move the 2026-09-30 build aside** (release Mac). Read the plan
   first; it only prints:

   ```sh
   CLOUDSDK_CONFIG=... scripts/desktop/retire-prerelease.sh
   CLOUDSDK_CONFIG=... scripts/desktop/retire-prerelease.sh --run
   ```

   It copies `desktop/<os>/1.0.0/` and the current `manifest.json` to
   `desktop/<os>/1.0.0-rc.0/`, checks each copy, and only then deletes
   `desktop/<os>/1.0.0/`. From here until step 4's manifests are up,
   installed apps find nothing to download and check again later, so go
   straight on.
4. **Publish** 1.0.0 (release Mac, which holds the update key):

   ```sh
   CLOUDSDK_CONFIG=... scripts/desktop/sign-manifest.sh --version 1.0.0 \
     --app target/desktop-release/OpenAgents.app --upload
   CLOUDSDK_CONFIG=... gcloud storage cp target/desktop-release/OpenAgents-1.0.0.dmg \
     gs://openagentsgemini-oa-updates/desktop/macos/1.0.0/OpenAgents-1.0.0.dmg
   CLOUDSDK_CONFIG=... scripts/desktop/sign-manifest-linux.sh --version 1.0.0 \
     --dir /tmp/openagents-linux-1.0.0 --upload
   CLOUDSDK_CONFIG=... scripts/desktop/sign-manifest-windows.sh --version 1.0.0 \
     --dir /tmp/openagents-windows-1.0.0 --upload
   ```

5. **Show it on the download page**: set `DESKTOP_RELEASED` to `true` in
   `crates/openagents-web/src/pages/download.rs`, run
   `cargo test -p openagents-web`, push, and deploy the site as usual.
   Until then `/download` shows no desktop files, so nobody downloads the
   2026-09-30 build from it.
6. **Check** every link and manifest: `scripts/dev/check-downloads.sh`
   must end with `Every link answered 200.`, and each platform's manifest
   line must say `1.0.0 published <today>`.

## Record of the first runs (2026-09-29)

On the desktop app itself (`30d22ea758`, the first commit with
`crates/openagents-desktop`): universal `OpenAgents`, `coder`, and
`microcoder`, both notarization submissions `Accepted`,
`OpenAgents-0.1.0.dmg` accepted by `spctl` with a browser quarantine flag
set, the app inside it accepted, `stapler validate` passing on both, and
the helpers signed as `com.openagents.desktop.coder` and
`com.openagents.desktop.microcoder` with the hardened runtime. The second
Mac check is an owner step in the workspace `NEEDS_OWNER.md`.

Before the desktop app landed, the pipeline was run with stand-ins:

- `--app` on the deck (`scripts/bundle-openagents-deck.sh`) with
  `microcoder` added to `Contents/MacOS`: both submissions `Accepted`;
  `spctl` accepted the app and the `.dmg`, also with a browser quarantine
  flag on the `.dmg`; `stapler validate` passed on both; the helper carried
  the hardened runtime flag and the entitlements above.
- `--bin-dir` with the deck binary as `OpenAgents` and real `coder` and
  `microcoder` builds: the assembled `OpenAgents.app` (default `Info.plist`,
  host launchd plist, icon) notarized and stapled the same way, and the
  signed `coder` and `microcoder` ran from `Contents/MacOS`.
- The full build path with `DESKTOP_PACKAGE=openagents-deck`: universal
  (`x86_64 arm64`) `OpenAgents`, `coder`, and `microcoder` from Cargo,
  assembled, signed, notarized, stapled, and accepted by `spctl`; the
  x86_64 slice of `coder` ran under Rosetta.


## Releasing for Linux

The Linux release is an AppImage and a `.deb` for x86_64 (and a
`.tar.gz`), at the same version as the Mac
([#10025](https://github.com/OpenAgentsInc/openagents/issues/10025)).
Two steps, on two computers:

1. **Build**, on any x86_64 Linux computer with Docker, from a clean
   checkout of the release commit:

   ```sh
   scripts/desktop/build-linux-release.sh --out /tmp/openagents-linux-1.0.0
   ```

   It compiles the window, `coder`, and `microcoder` inside the pinned
   `rust:1.97.1-bullseye` image (glibc 2.31: Debian 11, Ubuntu 20.04, and
   newer) and packages them with `package-linux.sh` and the pinned AppImage
   type-2 runtime. `package-linux.sh` builds with
   `OPENAGENTS_DESKTOP_RELEASE=1`, as the Mac's does, so only the release
   uses the release Secret Service items
   ([#10096](https://github.com/OpenAgentsInc/openagents/issues/10096));
   with `--skip-build`, the binaries must have been built with it. Paths are fixed and remapped, and every packaged file
   carries the commit's time, so a second run of the same commit writes the
   same bytes. `BUILDINFO` records the commit, the image digest, and the
   runtime's SHA-256.
2. **Sign and publish**, on the computer that holds the update key (the
   release Mac), with the build output copied over:

   ```sh
   CLOUDSDK_CONFIG=... scripts/desktop/sign-manifest-linux.sh \
     --version 1.0.0 --dir /tmp/openagents-linux-1.0.0 --upload
   ```

   It checks `SHA256SUMS`, the AppImage's header, and the `.deb`'s
   package, version, and architecture; writes the signed manifest (the
   Mac's envelope, with `"platform": "linux"` and each artifact's
   `format`); signs `SHA256SUMS` into `SHA256SUMS.sig`; and uploads
   everything to `desktop/linux/<version>/`, then the manifest to
   `desktop/linux/manifest.json`.

The downloads are
`https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/linux/<version>/OpenAgents-<version>-x86_64.AppImage`
and `.../openagents_<version>_amd64.deb`; the website's download table
(`DOWNLOADS` in `crates/openagents-web/src/pages/download.rs`) points at
them. To check one by hand, beside `SHA256SUMS`, `SHA256SUMS.sig`, and
the public key:

```sh
openssl pkeyutl -verify -pubin -inkey openagents-desktop-update.pub.pem \
  -rawin -in SHA256SUMS -sigfile SHA256SUMS.sig
sha256sum --ignore-missing -c SHA256SUMS
```

The public key to trust is the one in `update.rs` (`TRUSTED_KEYS`, hex
`b9c688e6f33b77f63f61ce46d1b520b2c5588b79ea4cccafc30b122f211a8b84`), not
only the copy beside the files.

**Updates on Linux.** An app running from an AppImage or the `.deb` checks
`desktop/linux/manifest.json` at start and every six hours, and Settings
shows the version. An AppImage downloads and verifies a newer build in
the background; Settings then offers **Restart to update to VERSION**,
which replaces the AppImage file in place (one rename, after the copy is
checked against the signed SHA-256), restarts the host unit
`com.openagents.desktop.host.service` (whose `ExecStart` is that file), and
starts the new app. From a terminal, `OpenAgents-….AppImage --update` does
the same without the window, and `--check-update` only reports. The
`.deb` has no package repository, so Settings offers **Download VERSION**,
which opens the new package in the browser. A build directory never
checks. `OPENAGENTS_UPDATE_MANIFEST_URL` points the check at a test
manifest (for example `desktop/linux-test/manifest.json`, signed with
`--prefix desktop/linux-test`); the compiled key still has to verify it.

## Releasing for Windows

The Windows release is a per-user MSI and a `.zip` for x64, at the same
version as the Mac
([#10027](https://github.com/OpenAgentsInc/openagents/issues/10027)).
Two steps, which can run on one Mac or Linux computer:

1. **Build**, from a clean checkout of the release commit, with Rust's
   `x86_64-pc-windows-gnu` target, a MinGW-w64 linker
   (`x86_64-w64-mingw32-gcc`: Homebrew's `mingw-w64` on a Mac), and
   `wixl`, `zip`, and `osslsigncode` on `PATH` (on NixOS,
   `nix shell nixpkgs#msitools nixpkgs#zip nixpkgs#osslsigncode`):

   ```sh
   scripts/desktop/package-windows.sh --out /tmp/openagents-windows-1.0.0 \
     --pfx codesign.pfx --pfx-password-file codesign.pass --require-signing
   ```

   It cross-builds `OpenAgents.exe` (the window), `coder.exe`,
   `microcoder.exe`, and `coder-boundary.exe`, signs each with
   Authenticode (SHA-256, RFC 3161 timestamp), and writes
   `OpenAgents-<version>-x64.msi` (wixl), `OpenAgents-<version>-windows-x64.zip`,
   `SHA256SUMS`, and `BUILDINFO`. On a Windows computer,
   `scripts\desktop\package-windows.ps1` builds the same packages with the
   MSVC target, the WiX Toolset, and `signtool`. Both install in
   `%LOCALAPPDATA%\Programs\OpenAgents` with the same `UpgradeCode`.
2. **Sign and publish**, on the computer that holds the update key:

   ```sh
   CLOUDSDK_CONFIG=... scripts/desktop/sign-manifest-windows.sh \
     --version 1.0.0 --dir /tmp/openagents-windows-1.0.0 --upload
   ```

   It checks `SHA256SUMS`, the MSI's and the `.zip`'s headers, and that
   `BUILDINFO` says the packages are signed; writes the signed manifest
   (the Mac's envelope, with `"platform": "windows"` and each artifact's
   `format`, `msi` or `zip`); signs `SHA256SUMS`; and uploads everything
   to `desktop/windows/<version>/`, then the manifest to
   `desktop/windows/manifest.json`.

**Authenticode.** Optional (owner, 2026-10-09). With the OpenAgents
code-signing certificate `package-windows.sh` signs the packages; without
it the packages are unsigned and still go to `desktop/windows` as a
release. Windows SmartScreen asks people to confirm an unsigned MSI the
first time ("Windows protected your PC"; **More info**, **Run anyway**), so
the download page says so. Updates stay verified either way: the manifest
is signed with our own update key.

**Updates on Windows.** An app installed from the MSI checks
`desktop/windows/manifest.json` at start and every six hours, downloads
the new MSI in the background, and verifies its size and SHA-256 against
the signed manifest. Settings then offers **Restart to update to
VERSION**: the window closes, a hidden PowerShell waits for it, stops what
still runs from the install folder (the host, a task's engine), and runs
`msiexec /i <msi> /passive /norestart`; the MSI's major upgrade replaces
the files and opens the new app, which starts the new host. Pairings
survive: the host's keys are in Credential Manager and its state under
`%USERPROFILE%\.openagents`, never in the install folder. A copy unpacked
from the `.zip` shows **Download VERSION** instead, which opens the MSI in
the browser. A development build (`openagents-desktop.exe`) never checks.
From a terminal, `OpenAgents.exe --check-update` reports, and `--update`
installs after it exits. `OPENAGENTS_UPDATE_MANIFEST_URL` points the check
at a test manifest, such as `desktop/windows-test/manifest.json`; the
compiled key still has to verify it.

**What differs on Windows.** Chat, Coder runs from chat, and pairing are
the same shared Rust as on the Mac. The Verse page's world (the Grid) stays
off: Verse does not build for Windows yet, and the window
keeps its plain background. Coder's desktop notifications are Linux's
([#10026](https://github.com/OpenAgentsInc/openagents/issues/10026)) and the
Mac's ([#10061](https://github.com/OpenAgentsInc/openagents/issues/10061),
through the notification center, asked for on the first notice); on Windows
they are toasts
([#10062](https://github.com/OpenAgentsInc/openagents/issues/10062)) under
the AppUserModelID `OpenAgents.Desktop`, which both MSIs put on the Start
menu shortcut (`System.AppUserModel.ID`; the `.sh` adds the
`MsiShortcutProperty` row with msitools' `msibuild`, since wixl has no
`<ShortcutProperty>`) and register under
`HKCU\Software\Classes\AppUserModelId`, and which the app claims at
startup. A `.zip` copy has no registration and shows no toasts. A click on a
toast opens its chat while the app runs; with no COM activator, a click in
Action Center after the app quit opens nothing. `OpenAgents.exe
--notify-test` shows a test toast and says how it went.
Copy and paste use the Windows
clipboard API as Unicode text; the folder chooser is the common item
dialog (`IFileOpenDialog`); IME composition comes from winit, and a
character typed with AltGr (which Windows reports as Ctrl+Alt) is text,
not a shortcut. A local Coder run's sandbox (the `toolchains` access of
[#10045](https://github.com/OpenAgentsInc/openagents/issues/10045)) is
the AppContainer boundary with the network open; it grants no extra reads
on Windows (each read is a DACL entry written over a whole tree, and the
system folders on `PATH` cannot take one), so tools installed for all
users under `C:\Program Files` (Git for Windows, Python, Node, Go) are on
its `PATH`, and a toolchain in the profile (rustup) is not yet. Where no
AppContainer can be made, the run refuses rather than running unconfined.

## Linux and Windows

The same app builds for Linux and Windows
([#9977](https://github.com/OpenAgentsInc/openagents/issues/9977));
`crates/openagents-desktop/src/platform/` holds what differs.

- **Linux.** `scripts/desktop/package-linux.sh` (on Linux) builds the window,
  `coder`, and `microcoder` and writes an AppImage, a `.deb`, a `.tar.gz`, and
  `SHA256SUMS`. On NixOS, run it inside
  `nix shell nixpkgs#patchelf nixpkgs#dpkg nixpkgs#squashfsTools`; it resets
  the Nix loader to the standard one and prints the newest glibc the build
  needs. The AppImage needs `appimagetool` on `PATH`, or
  `--appimage-runtime FILE` with an AppImage type-2 runtime. On first launch
  the app writes the systemd user unit `com.openagents.desktop.host.service`
  (`coder host serve --keychain --iroh --control`), and the host keeps its
  keys in the Secret Service. On a computer with no Secret Service running
  (no GNOME Keyring, KWallet, or KeePassXC), `coder host adopt detect`
  reports `"keys": "files"` and the unit runs
  `coder host serve --keys ~/.openagents/host-keys --iroh --control`
  instead: `0600` files in a `0700` directory, as a command-line install
  keeps them. A computer set up the old way is upgraded silently on first
  launch (`crates/openagents-desktop/src/migrate.rs`).
- **Windows.** `scripts/desktop/package-windows.ps1` (on Windows) writes a
  per-user MSI (WiX v4 or later) and a `.zip`, signed with
  `-CertificateThumbprint` through `signtool`; `-RequireSigning` for a
  release. The host starts at sign-in from the `Run` entry `OpenAgents`
  (`OpenAgents.exe --start-host`, which starts `coder.exe` with no console),
  and the control channel is a named pipe only this user can open
  ([#9980](https://github.com/OpenAgentsInc/openagents/issues/9980)). The
  host runs `coder.exe host serve --keychain --iroh --control` with a
  console that has no window, so neither it nor a program it starts opens
  one. Its keys are generic credentials in Credential Manager
  (`host-key.com.openagents.desktop`, `owner-key.…`, `host-iroh-key.…`,
  kept on this computer only), and its state files admit only the user
  (an owner-only DACL where Unix has `0700` and `0600`). A task's
  processes run in a job object that ends with the task, and terminals are
  ConPTY consoles running `%ComSpec%`. Repository tasks and auto-start run
  there too ([#9983](https://github.com/OpenAgentsInc/openagents/issues/9983)):
  a task's commands run in Git for Windows' `bash`, which must be installed
  for all users (`C:\Program Files\Git`), inside an AppContainer that
  `coder-boundary.exe` (packaged beside `coder.exe`) starts. The container
  is made for the one run and removed after it; its only file access is the
  entries the boundary adds for it to the workspace (and its checkout and
  scratch), and it holds no network capability. The workspace snapshot
  opens every entry relative to its parent's handle and never follows a
  link or junction. `-SkipCoder` still packages the window alone.
