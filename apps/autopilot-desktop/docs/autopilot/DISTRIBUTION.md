# Autopilot Desktop Distribution (Linux and macOS)

This document covers everything needed to build, sign, and distribute the Autopilot Desktop
Tauri app for Linux and macOS. Windows is intentionally out of scope.

## Scope and locations

- App root: `apps/autopilot-desktop/`
- Tauri config: `apps/autopilot-desktop/src-tauri/tauri.conf.json`
- Bundle outputs: `apps/autopilot-desktop/src-tauri/target/release/bundle/`

## Prerequisites

- Rust toolchain installed (`rustup`, `cargo`).
- Bun installed (this app uses Bun for scripts).
- Frontend deps installed: `bun install`.
- macOS: Xcode Command Line Tools installed (`xcode-select --install`).
- Linux build deps (Debian/Ubuntu):
  - `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`

Note: GUI apps on macOS and Linux do not inherit your shell `$PATH` (bash/zsh dotfiles).
If the app relies on CLI tools, use `fix-path-env-rs` to set PATH at runtime.

## Versioning

Tauri uses `version` from `apps/autopilot-desktop/src-tauri/tauri.conf.json`.
Keep `apps/autopilot-desktop/package.json` in sync with the same version.

For App Store builds, you can also set a separate `bundleVersion` under
`bundle.macOS.bundleVersion` if you need a different bundle version scheme.

## Logos and icons

Tauri uses app icons defined in `apps/autopilot-desktop/src-tauri/tauri.conf.json`
under `bundle.icon`, and the files live in `apps/autopilot-desktop/src-tauri/icons/`.

Current formats in this repo:
- PNGs: `icons/32x32.png`, `icons/64x64.png`, `icons/128x128.png`, `icons/128x128@2x.png`
- macOS: `icons/icon.icns`
- Windows: `icons/icon.ico` (kept for completeness, unused for Linux/macOS)

Recommended workflow:

1. Start from a single high-res square PNG (1024x1024 or larger).
2. Generate platform icons with the Tauri CLI:
   ```bash
   bun run tauri icon /path/to/source-icon.png
   ```
3. Commit the generated files in `src-tauri/icons/` and keep `bundle.icon`
   in `tauri.conf.json` pointing at those files.

Notes:
- macOS bundling requires `icon.icns`.
- Linux bundles read the PNGs for desktop metadata and app launchers.
- App Store metadata (screenshots, marketing images) is configured in App Store
  Connect and is separate from Tauri icon files.

## Build and bundle (local)

From `apps/autopilot-desktop/`:

```bash
bun install
bun run tauri build
```

This builds and bundles for the default targets on the current OS.

To split build and bundle steps:

```bash
bun run tauri build -- --no-bundle
bun run tauri bundle -- --bundles app,dmg
```

## macOS distribution

### App bundle (.app)

```bash
bun run tauri build -- --bundles app
```

Useful macOS bundle config options in `src-tauri/tauri.conf.json`:

```json
{
  "bundle": {
    "macOS": {
      "minimumSystemVersion": "12.0",
      "entitlements": "./Entitlements.plist",
      "frameworks": ["CoreAudio", "./libs/libcustom.dylib"],
      "files": {
        "SharedSupport/docs.md": "./docs/index.md"
      }
    }
  }
}
```

If you need custom `Info.plist` keys, create `src-tauri/Info.plist`.
Tauri merges this file into the generated Info.plist at build time.

### DMG (.dmg)

```bash
bun run tauri build -- --bundles dmg
```

Optional DMG customization:

```json
{
  "bundle": {
    "macOS": {
      "dmg": {
        "background": "./images/",
        "windowSize": { "width": 800, "height": 600 },
        "windowPosition": { "x": 400, "y": 400 },
        "appPosition": { "x": 180, "y": 220 },
        "applicationFolderPosition": { "x": 480, "y": 220 }
      }
    }
  }
}
```

### Code signing (required)

macOS distribution requires an Apple Developer account and an Apple-signed certificate.

Certificate types:
- **Developer ID Application**: distribute outside the App Store (requires notarization).
- **Apple Distribution**: App Store distribution.

OpenAgents team:
- Team name: **OpenAgents, Inc.**
- Team ID: **HQWSG26L43**
- Default signing identity (Developer ID): `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`

Local signing:

1. Create a CSR, then create and download a certificate in Apple Developer.
2. Install the `.cer` in your Keychain.
3. Get the signing identity:
   ```bash
   security find-identity -v -p codesigning
   ```
4. Set the signing identity:
   - `bundle.macOS.signingIdentity` in `tauri.conf.json` (defaults to OpenAgents Developer ID), or
   - `APPLE_SIGNING_IDENTITY` environment variable (overrides config).

CI signing (p12 export):

1. Export the certificate as `.p12`.
2. Base64 encode it:
   ```bash
   openssl base64 -in /path/to/certificate.p12 -out certificate-base64.txt
   ```
3. Set env vars:
   - `APPLE_CERTIFICATE` (base64 contents)
   - `APPLE_CERTIFICATE_PASSWORD` (p12 password)
   - `KEYCHAIN_PASSWORD` (for temporary keychain)

Ad-hoc signing (no Apple identity, dev only):

```json
{ "bundle": { "macOS": { "signingIdentity": "-" } } }
```

### Notarization (required for Developer ID)

Use one of the following auth methods:

App Store Connect API:
- `APPLE_API_ISSUER`
- `APPLE_API_KEY`
- `APPLE_API_KEY_PATH` (path to the downloaded private key)
 - `APPLE_TEAM_ID` (OpenAgents: `HQWSG26L43`)

Apple ID:
- `APPLE_ID`
- `APPLE_PASSWORD` (app-specific password)
- `APPLE_TEAM_ID` (OpenAgents: `HQWSG26L43`)

### App Store distribution

App Store builds require extra configuration:

- Set `bundle.category` in `tauri.conf.json`.
- Create a provisioning profile and embed it:
  ```json
  {
    "bundle": {
      "macOS": {
        "files": {
          "embedded.provisionprofile": "path/to/profile-name.provisionprofile"
        }
      }
    }
  }
  ```
- Add `Info.plist` with encryption flag:
  ```xml
  <key>ITSAppUsesNonExemptEncryption</key>
  <false/>
  ```
- Add entitlements with App Sandbox and IDs:
  ```xml
  <key>com.apple.security.app-sandbox</key>
  <true/>
  <key>com.apple.application-identifier</key>
  <string>$TEAM_ID.$IDENTIFIER</string>
  <key>com.apple.developer.team-identifier</key>
  <string>$TEAM_ID</string>
  ```

Keep App Store specific settings in a separate config file (recommended):

```json
{
  "bundle": {
    "macOS": {
      "entitlements": "./Entitlements.plist",
      "files": {
        "embedded.provisionprofile": "path/to/profile-name.provisionprofile"
      }
    }
  }
}
```

Build a universal app bundle:

```bash
bun run tauri build -- --bundles app --target universal-apple-darwin --config src-tauri/tauri.appstore.conf.json
```

Create a signed `.pkg`:

```bash
xcrun productbuild --sign "<Mac Installer Distribution Identity>" \
  --component "target/universal-apple-darwin/release/bundle/macos/$APPNAME.app" \
  /Applications "$APPNAME.pkg"
```

Upload to App Store Connect:

```bash
xcrun altool --upload-app --type macos --file "$APPNAME.pkg" \
  --apiKey $APPLE_API_KEY_ID --apiIssuer $APPLE_API_ISSUER
```

`altool` expects the App Store Connect private key file at one of:
`./private_keys`, `~/private_keys`, `~/.private_keys`, or
`~/.appstoreconnect/private_keys`, named `AuthKey_<APPLE_API_KEY_ID>.p8`.

## Linux distribution

### Base build

```bash
bun run tauri build
```

Default bundle outputs:

- AppImage: `src-tauri/target/release/bundle/appimage/`
- Debian: `src-tauri/target/release/bundle/deb/`
- RPM: `src-tauri/target/release/bundle/rpm/`

Build on the oldest Linux base you intend to support (glibc compatibility).
Using a container or CI for Linux builds is strongly recommended.

### AppImage

Optional AppImage settings:

```json
{
  "bundle": {
    "linux": {
      "appimage": {
        "bundleMediaFramework": true,
        "files": {
          "/usr/share/README.md": "../README.md",
          "/usr/assets": "../assets/"
        }
      }
    }
  }
}
```

AppImage signing (optional, GPG):

- Set env vars when building:
  - `SIGN=1`
  - `SIGN_KEY` (optional GPG key id)
  - `APPIMAGETOOL_SIGN_PASSPHRASE`
  - `APPIMAGETOOL_FORCE_SIGN=1` (optional, fail on signing errors)
- View signature:
  ```bash
  ./src-tauri/target/release/bundle/appimage/$APPNAME_$VERSION_amd64.AppImage --appimage-signature
  ```
- Validate with AppImage validate tool:
  ```bash
  chmod +x validate-$PLATFORM.AppImage
  ./validate-$PLATFORM.AppImage $TAURI_OUTPUT.AppImage
  ```

### Debian (.deb)

Tauri generates a stock Debian package with the usual GTK/WebKit deps.
You can add dependencies and files via `bundle.linux.deb`:

```json
{
  "bundle": {
    "linux": {
      "deb": {
        "files": {
          "/usr/share/README.md": "../README.md",
          "/usr/share/assets": "../assets/"
        }
      }
    }
  }
}
```

ARM builds require either native ARM hardware or a full cross-compile setup
(`rustup target add`, cross linker, dpkg multi-arch, and ARM webkit/openssl
dev packages). Use CI or a dedicated build host if you need ARM artifacts.

### RPM (.rpm)

You can configure RPM scripts, deps, and metadata under `bundle.linux.rpm`.
Example additions:

```json
{
  "bundle": {
    "linux": {
      "rpm": {
        "conflicts": ["oldLib.rpm"],
        "depends": ["newLib.rpm"],
        "obsoletes": ["veryoldLib.rpm"],
        "provides": ["coolLib.rpm"],
        "desktopTemplate": "/path/to/desktop-template.desktop"
      }
    }
  }
}
```

RPM signing (optional, GPG):

```bash
gpg --gen-key
export TAURI_SIGNING_RPM_KEY=$(cat /path/to/private.key)
export TAURI_SIGNING_RPM_KEY_PASSPHRASE=your_passphrase
bun run tauri build
```

Verify signature locally:

```bash
gpg --export -a 'Tauri-App' > RPM-GPG-KEY-Tauri-App
sudo rpm --import RPM-GPG-KEY-Tauri-App
rpm -v --checksig path/to/app.rpm
```

### Snapcraft (.snap)

Snapcraft uses a `snapcraft.yaml` and typically builds from the `.deb` bundle:

1. Install `snapd` and `snapcraft`.
2. Register your app name on snapcraft.io.
3. Create `snapcraft.yaml` (see Snapcraft guide for a Tauri example).
4. Build: `sudo snapcraft`
5. Release: `snapcraft upload --release=stable your.snap`

### Flatpak (Flathub)

Flatpak distribution is supported but the upstream guide is marked draft.
At a minimum:

1. Install `flatpak` and `flatpak-builder`.
2. Install the runtime: `flatpak install flathub org.gnome.Platform//46 org.gnome.Sdk//46`
3. Build a `.deb` with Tauri.
4. Create an AppStream meta info file and a Flatpak manifest.
5. Build/test with `flatpak-builder`.
6. Submit to Flathub via PR.

### AUR (Arch User Repository)

1. Create an AUR account and repo.
2. Add a `PKGBUILD` that pulls your `.deb` artifacts and lists Tauri deps.
3. Generate `.SRCINFO`:
   ```bash
   makepkg --printsrcinfo > .SRCINFO
   ```
4. Test: `makepkg`
5. Commit and push to AUR.

## CI build and release (GitHub Actions)

Use `tauri-apps/tauri-action` to build and upload releases. Because the app
is not repo-root, set `projectPath: apps/autopilot-desktop`.

Minimal example (Linux + macOS only):

```yaml
jobs:
  publish-tauri:
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: 'macos-latest'
            args: '--target aarch64-apple-darwin'
          - platform: 'macos-latest'
            args: '--target x86_64-apple-darwin'
          - platform: 'ubuntu-22.04'
            args: ''
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4
      - name: install dependencies (ubuntu only)
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
      - name: setup node
        uses: actions/setup-node@v4
        with:
          node-version: lts/*
      - name: setup bun
        uses: oven-sh/setup-bun@v1
      - name: install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}
      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './apps/autopilot-desktop/src-tauri -> target'
      - name: install frontend dependencies
        run: bun install
        working-directory: apps/autopilot-desktop
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          projectPath: apps/autopilot-desktop
          args: ${{ matrix.args }}
```

For macOS signing/notarization in CI, add the `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD`, and notarization env vars
as secrets, then follow the macOS signing steps above.
