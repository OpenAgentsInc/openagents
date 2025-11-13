# iOS Archive Workflow for Tauri App

## Problem

Tauri v2's iOS archiving process has a fundamental issue: the `xcode-script` command tries to connect to a WebSocket dev server (on localhost:59123) even during production archive builds. This causes the archive to fail with:

```
thread '<unnamed>' panicked at crates/tauri-cli/src/mobile/mod.rs:436:6:
failed to read CLI options: Context("failed to build WebSocket client",
Io(Os { code: 61, kind: ConnectionRefused, message: "Connection refused" }))
```

This happens because Tauri CLI expects a running dev server for coordinating the build, but archive builds (ACTION=install) should not require this.

## Solution

We created a custom build script (`build-rust.sh`) that:
1. Detects whether it's a dev build or archive build (via `ACTION` environment variable)
2. For archive builds: Uses a pre-built Rust library instead of trying to rebuild
3. For dev builds: Falls back to the standard Tauri CLI workflow

## Prerequisites

- Xcode 17.0+ (tested with 17.1.1)
- Rust toolchain with iOS targets installed
- bun package manager
- Apple Developer account (team ID: HQWSG26L43)

## Complete Workflow

### Step 1: Build the iOS Rust Library

The Rust library must be built BEFORE archiving. This is done manually with proper SDK environment variables:

```bash
cd /Users/christopherdavid/code/openagents/tauri/src-tauri

# Set iOS SDK environment variables
export SDKROOT="/Applications/Xcode.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS26.1.sdk"
export IPHONEOS_DEPLOYMENT_TARGET=17.0

# Build for iOS device (arm64)
cargo build --target aarch64-apple-ios --release --lib
```

**Important Notes:**
- The SDKROOT must point to the actual SDK (e.g., iPhoneOS26.1.sdk), not the symlink
- Use `xcrun --sdk iphoneos --show-sdk-path` to get the correct SDK path
- This builds `libopenagents_lib.a` in `target/aarch64-apple-ios/release/`
- Build time: ~50 seconds on M1 Mac

### Step 2: Archive the iOS App

Once the library is built, archive using xcodebuild:

```bash
cd /Users/christopherdavid/code/openagents/tauri/src-tauri/gen/apple

xcodebuild archive \
  -project openagents.xcodeproj \
  -scheme openagents_iOS \
  -configuration release \
  -archivePath ~/Desktop/OpenAgents.xcarchive \
  -sdk iphoneos \
  -allowProvisioningUpdates \
  CODE_SIGN_STYLE=Automatic \
  DEVELOPMENT_TEAM=HQWSG26L43
```

The custom build script will:
1. Detect `ACTION=install` (archive mode)
2. Look for the pre-built library at `src-tauri/target/aarch64-apple-ios/release/libopenagents_lib.a`
3. Copy it to the expected location: `gen/apple/Externals/arm64/release/libapp.a`
4. Continue with the rest of the Xcode build process

**Expected output:**
```
Archive build detected - checking for pre-built library
Found pre-built library at /Users/.../libopenagents_lib.a
Copied library to .../Externals/arm64/release/libapp.a
Archive build complete
```

Archive location: `~/Desktop/OpenAgents.xcarchive`

### Step 3: Upload to App Store Connect

#### Option A: Using Xcode Organizer (Recommended)

1. Open Xcode Organizer: Window > Organizer (⇧⌘O)
2. Select "Archives" tab
3. Find "OpenAgents" and select the latest archive
4. Click "Distribute App"
5. Choose "App Store Connect"
6. Select "Upload"
7. Choose "Automatically manage signing"
8. Review and click "Upload"

#### Option B: Using Command Line

1. **Export the IPA:**
```bash
xcodebuild -exportArchive \
  -archivePath ~/Desktop/OpenAgents.xcarchive \
  -exportPath ~/Desktop/OpenAgents-Export \
  -exportOptionsPlist /Users/christopherdavid/code/openagents/tauri/src-tauri/gen/apple/ExportOptions.plist
```

2. **Upload to App Store Connect:**
```bash
xcrun altool --upload-app \
  --type ios \
  --file ~/Desktop/OpenAgents-Export/OpenAgents.ipa \
  --username YOUR_APPLE_ID \
  --password YOUR_APP_SPECIFIC_PASSWORD
```

Or using the newer `notarytool`:
```bash
xcrun notarytool submit ~/Desktop/OpenAgents-Export/OpenAgents.ipa \
  --apple-id YOUR_APPLE_ID \
  --password YOUR_APP_SPECIFIC_PASSWORD \
  --team-id HQWSG26L43
```

### Step 4: TestFlight

After upload completes:

1. Go to https://appstoreconnect.apple.com
2. Select "OpenAgents" app
3. Navigate to "TestFlight" tab
4. Wait for processing (5-15 minutes typically)
5. Add internal testers (team members with Admin/Developer/App Manager roles)
6. Or submit for external testing (requires beta review)

TestFlight link: https://testflight.apple.com/join/dvQdns5B

## Build Script Details

### File: `tauri/src-tauri/gen/apple/build-rust.sh`

The custom script handles both development and archive builds:

**For Archive Builds (ACTION=install):**
- Checks for pre-built library at expected location
- Copies to Xcode's expected output location
- Fails with clear error message if library not found

**For Dev Builds:**
- Falls back to standard Tauri CLI: `bun tauri ios xcode-script`
- Requires WebSocket dev server to be running
- Used during `bun run dev` / simulator development

### Integration

The script is integrated via `project.yml`:

```yaml
preBuildScripts:
  - script: ${SRCROOT}/build-rust.sh
    name: Build Rust Code
    basedOnDependencyAnalysis: false
    outputFiles:
      - $(SRCROOT)/Externals/x86_64/${CONFIGURATION}/libapp.a
      - $(SRCROOT)/Externals/arm64/${CONFIGURATION}/libapp.a
```

## App Configuration

Current production configuration:

- **Bundle ID:** com.openagents.app
- **Version:** 0.3.0
- **Build Number:** 100
- **Display Name:** OpenAgents
- **Category:** Developer Tools
- **Deployment Target:** iOS 17.0
- **Development Team:** HQWSG26L43 (OpenAgents, Inc.)
- **Supported Devices:** iPhone and iPad (universal)

### Entitlements

Required for local network discovery (Bonjour/mDNS):

```xml
<key>com.apple.developer.networking.multicast</key>
<true/>
<key>com.apple.security.network.client</key>
<true/>
<key>com.apple.security.network.server</key>
<true/>
<key>NSLocalNetworkUsageDescription</key>
<string>OpenAgents needs local network access to discover and connect to
your desktop server for syncing conversations and settings.</string>
<key>NSBonjourServices</key>
<array>
  <string>_openagents._tcp</string>
</array>
```

## Troubleshooting

### "ERROR: Library not found"

The build script reports:
```
ERROR: Library not found at /Users/.../target/aarch64-apple-ios/release/libopenagents_lib.a
Please run 'cargo build --target aarch64-apple-ios --release --lib' first
```

**Solution:** Run Step 1 (build the Rust library) before archiving.

### "ld: library 'iconv' not found"

This happens if you try to build the Rust library without setting `SDKROOT`:

**Solution:** Make sure to export SDKROOT before running cargo:
```bash
export SDKROOT="/Applications/Xcode.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS26.1.sdk"
cargo build --target aarch64-apple-ios --release --lib
```

### "No such file or directory" when building

Make sure you're in the correct directory:
- For cargo build: `tauri/src-tauri/`
- For xcodebuild: `tauri/src-tauri/gen/apple/`

### Xcode Project Not Found

If xcodebuild says `'openagents.xcodeproj' does not exist`:

```bash
cd /Users/christopherdavid/code/openagents/tauri/src-tauri/gen/apple
# Verify you're in the right place
ls openagents.xcodeproj
```

### Code Signing Issues

If you see provisioning profile errors:

1. Open the project in Xcode
2. Select the "openagents_iOS" target
3. Go to "Signing & Capabilities" tab
4. Ensure "Automatically manage signing" is checked
5. Select team: "OpenAgents, Inc." (HQWSG26L43)

### Wrong SDK Version

If the SDK path in SDKROOT doesn't exist:

```bash
# Find available SDKs
ls /Applications/Xcode.app/Contents/Developer/Platforms/iPhoneOS.platform/Developer/SDKs/

# Or use xcrun to get current SDK
xcrun --sdk iphoneos --show-sdk-path
```

## Alternative: Standard Tauri CLI (Currently Broken for Archive)

The standard Tauri workflow would be:

```bash
cd tauri
bun run build          # Build frontend
bun tauri ios build    # Build iOS app
```

However, this doesn't work for creating archives due to the WebSocket requirement issue. It's fine for simulator testing during development.

## Version Management

When releasing a new version:

1. Update `tauri/src-tauri/tauri.conf.json`:
```json
{
  "version": "0.4.0"
}
```

2. Update `tauri/src-tauri/gen/apple/project.yml`:
```yaml
CFBundleShortVersionString: 0.4.0
CFBundleVersion: "101"  # Increment build number
```

3. Regenerate Xcode project:
```bash
cd tauri/src-tauri/gen/apple
xcodegen generate
```

4. Rebuild and archive following Steps 1-2 above.

**Important:**
- `CFBundleShortVersionString` is the user-visible version (e.g., "0.4.0")
- `CFBundleVersion` must be unique and incrementing for each submitted build
- Build number must be higher than any previously submitted build for that version

## Files Modified

- `tauri/src-tauri/gen/apple/build-rust.sh` - Custom build script (NEW)
- `tauri/src-tauri/gen/apple/project.yml` - Updated preBuildScripts to use custom script
- `tauri/src-tauri/tauri.conf.json` - Version, bundle ID, display name
- `tauri/src-tauri/gen/apple/openagents_iOS/Info.plist` - Network permissions
- `tauri/src-tauri/gen/apple/openagents_iOS/openagents_iOS.entitlements` - Multicast entitlement

## Future Improvements

1. **Automated Build Script:** Create a single script that runs both steps
2. **CI/CD Integration:** Automate this workflow in GitHub Actions
3. **Simulator Support:** Add x86_64 and aarch64-sim targets for testing
4. **Cargo Config:** Explore better Rust cross-compilation configuration
5. **Tauri Fix:** Monitor Tauri project for fixes to the WebSocket requirement

## Summary

The key insight is that Tauri's mobile archiving has an architectural limitation where it expects a dev server connection even for production builds. By pre-building the Rust library with proper iOS SDK configuration and using a custom build script to detect and handle archive mode, we bypass this requirement entirely.

This two-step process (build Rust library, then archive) is currently the only reliable way to create iOS archives for TestFlight/App Store distribution.

## References

- **Tauri Mobile Docs:** https://v2.tauri.app/develop/mobile/
- **App Store Connect:** https://appstoreconnect.apple.com
- **TestFlight:** https://testflight.apple.com
- **Xcode Cloud:** https://developer.apple.com/xcode-cloud/
- **Main Build Guide:** `/docs/tauri-ios-build.md`
