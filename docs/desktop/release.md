# Releasing OpenAgents for Mac

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

## Release

```sh
scripts/desktop/package-macos.sh --notary-env ~/path/to/appstoreconnect.env
```

It writes `target/desktop-release/OpenAgents.app` and
`target/desktop-release/OpenAgents-<version>.dmg`, and prints the
submission IDs, signing authority, architectures, and the `.dmg`'s SHA-256.
A run takes the release builds plus two notarization round trips (usually
one to five minutes each).

What it does, in order:

1. **Build.** `cargo build --release --locked` of `openagents-desktop`,
   `coder`, `microcoder`, and `openagents` (the `openagents-cli` package) for
   `aarch64-apple-darwin` and `x86_64-apple-darwin`, with
   `MACOSX_DEPLOYMENT_TARGET=13.0` (the floor for `SMAppService`), joined
   into universal binaries with `lipo`.
2. **Assemble** `OpenAgents.app`:

   ```text
   OpenAgents.app/Contents/
     Info.plist                      com.openagents.desktop, version from Cargo
     MacOS/OpenAgents                the window and menu bar
     MacOS/coder                     runs `coder host serve` as a launchd agent
     MacOS/microcoder
     MacOS/openagents                the command a phone's command card runs;
                                     the host puts MacOS/ first on its
                                     terminals' PATH
     Library/LaunchAgents/com.openagents.desktop.host.plist
     Resources/AppIcon.icns
   ```

   `Info.plist`, `com.openagents.desktop.host.plist`, and the entitlements
   files come from `bins/openagents-desktop-macos/`, the same files the quick
   local build (`bins/openagents-desktop-macos/bundle.sh`) uses; the
   version is the crate's and the build number is the commit count, as
   there. The icon is `AppIcon.icns` from that folder if present, otherwise
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

Outside the App Sandbox neither key is enforced; they are there so the
bundle keeps working if it is ever sandboxed. The hardened runtime needs no
exception (`allow-jit`, `disable-library-validation`, and so on): the binaries
are plain Rust with no JIT and no third-party dylibs. Add one only with a
reason in the entitlements file.

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
  keys in the Secret Service. A computer with no Secret Service running (no
  GNOME Keyring, KWallet, or KeePassXC) cannot use `--keychain`; the host says
  so and stores nothing in a file.
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
  ConPTY consoles running `%ComSpec%`. Repository tasks and auto-start
  refuse on Windows for now: they run under the write boundary and the
  workspace snapshot, which have no Windows implementation yet
  ([#9983](https://github.com/OpenAgentsInc/openagents/issues/9983)).
  `-SkipCoder` still packages the window alone.
