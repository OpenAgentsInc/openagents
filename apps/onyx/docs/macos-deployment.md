# macOS Deployment Guide for Onyx

This guide explains how to build, bundle, and deploy Onyx as a native macOS application with a proper app icon that can run permanently on your Mac.

## Overview

The deployment workflow involves:
1. Building the Rust binary in release mode
2. Creating a `.app` bundle using `cargo-bundle`
3. Adding app icons and metadata
4. Optionally code signing for Gatekeeper
5. Installing to `/Applications`
6. Optionally configuring auto-start via LaunchAgent

## Prerequisites

- Rust toolchain (via rustup)
- Xcode Command Line Tools: `xcode-select --install`
- cargo-bundle: `cargo install cargo-bundle --git https://github.com/zed-industries/cargo-bundle.git --branch zed-deploy`

## Directory Structure

```
apps/onyx/
├── Cargo.toml              # Bundle metadata goes here
├── resources/
│   ├── app-icon.png        # 512x512 app icon
│   ├── app-icon@2x.png     # 1024x1024 app icon (Retina)
│   ├── onyx.entitlements   # macOS entitlements
│   └── info/
│       └── Permissions.plist  # Privacy permission descriptions
└── docs/
    └── macos-deployment.md # This file
```

## Step 1: Create App Icons

Create PNG icons at these sizes:
- `resources/app-icon.png` - 512x512 pixels
- `resources/app-icon@2x.png` - 1024x1024 pixels (Retina)

`cargo-bundle` automatically converts PNG to `.icns` format during bundling.

**Quick placeholder icon generation:**
```bash
# Create resources directory
mkdir -p apps/onyx/resources

# Generate a simple placeholder icon (requires ImageMagick)
convert -size 1024x1024 xc:'#1a1a1a' -fill '#00ff88' \
  -gravity center -pointsize 400 -annotate 0 'O' \
  apps/onyx/resources/app-icon@2x.png
convert apps/onyx/resources/app-icon@2x.png -resize 512x512 \
  apps/onyx/resources/app-icon.png
```

## Step 2: Configure Cargo.toml for Bundling

Add this to `apps/onyx/Cargo.toml`:

```toml
[package.metadata.bundle]
name = "Onyx"
identifier = "com.openagents.onyx"
icon = ["resources/app-icon@2x.png", "resources/app-icon.png"]
version = "0.1.0"
copyright = "Copyright 2024 OpenAgents"
category = "public.app-category.productivity"
short_description = "Local-first Markdown note editor"
long_description = """
Onyx is a GPU-rendered Markdown note editor with live inline formatting,
vim mode, and local-first storage.
"""
osx_minimum_system_version = "10.15"
osx_url_schemes = ["onyx"]
```

## Step 3: Create Entitlements File

Create `apps/onyx/resources/onyx.entitlements`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- File access for notes vault -->
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>
    <key>com.apple.security.files.downloads.read-write</key>
    <true/>
</dict>
</plist>
```

## Step 4: Build Script

Create `scripts/bundle-mac`:

```bash
#!/bin/bash
set -euo pipefail

# Configuration
APP_NAME="Onyx"
BUNDLE_ID="com.openagents.onyx"

# Parse arguments
BUILD_TYPE="release"
SIGN_APP=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--debug) BUILD_TYPE="debug"; shift ;;
        -s|--sign) SIGN_APP=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

echo "Building Onyx ($BUILD_TYPE)..."

# Build
if [ "$BUILD_TYPE" = "release" ]; then
    cargo build --release -p onyx
else
    cargo build -p onyx
fi

# Bundle
echo "Creating app bundle..."
cargo bundle --${BUILD_TYPE} -p onyx

# Locate the bundle
if [ "$BUILD_TYPE" = "release" ]; then
    APP_PATH="target/release/bundle/osx/${APP_NAME}.app"
else
    APP_PATH="target/debug/bundle/osx/${APP_NAME}.app"
fi

if [ ! -d "$APP_PATH" ]; then
    echo "Error: Bundle not found at $APP_PATH"
    exit 1
fi

echo "Bundle created at: $APP_PATH"

# Code sign (optional, for local use ad-hoc signing works)
if [ "$SIGN_APP" = true ]; then
    echo "Code signing..."
    ENTITLEMENTS="apps/onyx/resources/onyx.entitlements"

    if [ -f "$ENTITLEMENTS" ]; then
        /usr/bin/codesign --force --deep --timestamp --options runtime \
            --entitlements "$ENTITLEMENTS" \
            --sign - "$APP_PATH"
    else
        /usr/bin/codesign --force --deep --timestamp --options runtime \
            --sign - "$APP_PATH"
    fi
    echo "Signed with ad-hoc signature"
fi

echo ""
echo "Done! To install:"
echo "  cp -r \"$APP_PATH\" /Applications/"
echo ""
echo "To run:"
echo "  open /Applications/${APP_NAME}.app"
```

Make it executable:
```bash
chmod +x scripts/bundle-mac
```

## Step 5: Build and Install

```bash
# Build release bundle
./scripts/bundle-mac

# Or with ad-hoc signing (recommended for personal use)
./scripts/bundle-mac --sign

# Install to Applications
cp -r target/release/bundle/osx/Onyx.app /Applications/

# Run
open /Applications/Onyx.app
```

## Step 6: Auto-Start on Login (Optional)

To have Onyx start automatically when you log in, create a LaunchAgent.

Create `~/Library/LaunchAgents/com.openagents.onyx.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.openagents.onyx</string>

    <key>ProgramArguments</key>
    <array>
        <string>/Applications/Onyx.app/Contents/MacOS/onyx</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <false/>

    <key>StandardOutPath</key>
    <string>/tmp/onyx.log</string>

    <key>StandardErrorPath</key>
    <string>/tmp/onyx.err</string>
</dict>
</plist>
```

Load it:
```bash
launchctl load ~/Library/LaunchAgents/com.openagents.onyx.plist
```

To unload:
```bash
launchctl unload ~/Library/LaunchAgents/com.openagents.onyx.plist
```

## Quick Install Script

For convenience, create `scripts/install-mac`:

```bash
#!/bin/bash
set -euo pipefail

echo "Building and installing Onyx..."

# Build
./scripts/bundle-mac --sign

# Remove old installation
rm -rf /Applications/Onyx.app

# Install
cp -r target/release/bundle/osx/Onyx.app /Applications/

echo "Onyx installed to /Applications/Onyx.app"
echo ""
echo "You can now:"
echo "  - Open from Spotlight (Cmd+Space, type 'Onyx')"
echo "  - Open from Finder (/Applications/Onyx.app)"
echo "  - Run from terminal: open /Applications/Onyx.app"
```

## Code Signing for Distribution

For distributing Onyx to others (not just personal use), you need:

1. **Apple Developer Account** ($99/year)
2. **Developer ID Application Certificate**
3. **Notarization** (required for macOS 10.15+)

### Getting Certificates

1. Enroll at https://developer.apple.com
2. In Xcode > Preferences > Accounts, add your Apple ID
3. Create a "Developer ID Application" certificate

### Signing with Developer ID

```bash
# Find your identity
security find-identity -v -p codesigning

# Sign with your Developer ID
IDENTITY="Developer ID Application: Your Name (TEAM_ID)"
/usr/bin/codesign --force --deep --timestamp --options runtime \
    --entitlements apps/onyx/resources/onyx.entitlements \
    --sign "$IDENTITY" \
    target/release/bundle/osx/Onyx.app
```

### Notarization

```bash
# Create a ZIP for notarization
ditto -c -k --keepParent target/release/bundle/osx/Onyx.app Onyx.zip

# Submit for notarization (requires App Store Connect API key)
xcrun notarytool submit Onyx.zip \
    --key /path/to/AuthKey.p8 \
    --key-id KEY_ID \
    --issuer ISSUER_ID \
    --wait

# Staple the ticket
xcrun stapler staple target/release/bundle/osx/Onyx.app
```

## Creating a DMG (Optional)

For distribution, create a DMG:

```bash
# Create DMG
hdiutil create -volname "Onyx" -srcfolder target/release/bundle/osx/Onyx.app \
    -ov -format UDZO Onyx.dmg

# Sign DMG (if distributing)
codesign --sign "$IDENTITY" Onyx.dmg
```

## Troubleshooting

### "App is damaged and can't be opened"
This happens when macOS quarantines an unsigned app. Fix with:
```bash
xattr -cr /Applications/Onyx.app
```

### App doesn't appear in Spotlight
Wait a few minutes for Spotlight to index, or rebuild the index:
```bash
mdimport /Applications/Onyx.app
```

### Icon not showing
Clear the icon cache:
```bash
sudo rm -rf /Library/Caches/com.apple.iconservices.store
killall Finder
killall Dock
```

### cargo-bundle not found
Install the Zed fork:
```bash
cargo install cargo-bundle --git https://github.com/zed-industries/cargo-bundle.git --branch zed-deploy
```

## CI/CD Integration

For GitHub Actions, see `.github/workflows/release-mac.yml` (to be created).

Key secrets needed:
- `MACOS_CERTIFICATE` - Base64-encoded .p12 certificate
- `MACOS_CERTIFICATE_PASSWORD` - Certificate password
- `APPLE_NOTARIZATION_KEY` - App Store Connect API key
- `APPLE_NOTARIZATION_KEY_ID` - Key ID
- `APPLE_NOTARIZATION_ISSUER_ID` - Issuer ID

## References

- [cargo-bundle documentation](https://github.com/burtonageo/cargo-bundle)
- [Apple Code Signing Guide](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/)
- [Apple Notarization Guide](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [Zed's bundle-mac script](https://github.com/zed-industries/zed/blob/main/scripts/bundle-mac)
