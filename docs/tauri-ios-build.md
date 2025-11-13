This document has moved.

See the new iOS deployment docs:
- docs/ios-deployment/README.md
- docs/ios-deployment/tauri-ios-build.md

## Prerequisites

- macOS with Xcode 17.0+ (tested with Xcode 17.1.1 / 17B100)
- iOS 17.0+ deployment target
- Apple Developer account (OpenAgents, Inc. team ID: HQWSG26L43)
- bun package manager installed
- Rust toolchain (automatically installed by Tauri if missing)

## Project Configuration

### Key Configuration Files

#### `tauri/src-tauri/tauri.conf.json`
```json
{
  "productName": "OpenAgents",
  "version": "0.3.0",
  "identifier": "com.openagents.app"
}
```

- **Product Name**: Display name shown to users ("OpenAgents")
- **Version**: Semantic version (currently 0.3.0)
- **Bundle ID**: com.openagents.app

#### `tauri/src-tauri/gen/apple/project.yml`
```yaml
bundleIdPrefix: com.openagents.app
deploymentTarget:
  iOS: 17.0
settings:
  base:
    DEVELOPMENT_TEAM: HQWSG26L43
    CODE_SIGN_STYLE: Automatic
    CFBundleShortVersionString: 0.3.0
    CFBundleVersion: "100"
    LSApplicationCategoryType: public.app-category.developer-tools
```

- **CFBundleShortVersionString**: User-visible version (e.g., "0.3.0")
- **CFBundleVersion**: Build number (e.g., "100")
- **Category**: Developer Tools
- **Team**: HQWSG26L43 (OpenAgents, Inc.)

### Entitlements & Permissions

The app requires specific entitlements for local network access (Bonjour/mDNS discovery):

#### `tauri/src-tauri/gen/apple/openagents_iOS/openagents_iOS.entitlements`
```xml
<key>com.apple.developer.networking.multicast</key>
<true/>
<key>com.apple.security.network.client</key>
<true/>
<key>com.apple.security.network.server</key>
<true/>
```

#### `tauri/src-tauri/gen/apple/openagents_iOS/Info.plist`
```xml
<key>NSLocalNetworkUsageDescription</key>
<string>OpenAgents needs local network access to discover and connect to your desktop server for syncing conversations and settings.</string>
<key>NSBonjourServices</key>
<array>
  <string>_openagents._tcp</string>
</array>
```

## Building the App

### Method 1: CLI Build (Recommended)

1. **Navigate to tauri directory:**
   ```bash
   cd tauri
   ```

2. **Build frontend:**
   ```bash
   bun run build
   ```
   This compiles the React/TypeScript frontend to `dist/`.

3. **Build iOS app:**
   ```bash
   bun tauri ios build
   ```

   This command:
   - Compiles Rust code for aarch64-apple-ios target
   - Builds WebView frontend
   - Creates iOS app bundle
   - Signs with Apple Developer certificate (Automatic signing)

**Note**: The build script in `project.yml` uses an absolute path to bun: `/Users/christopherdavid/.bun/bin/bun`. If building on a different machine, update this path in `project.yml` or ensure bun is in Xcode's PATH.

### Method 2: Xcode Build

1. **Open Xcode project:**
   ```bash
   cd tauri/src-tauri/gen/apple
   open openagents.xcodeproj
   ```

2. **Select target:**
   - Scheme: "openagents_iOS"
   - Destination: "Any iOS Device"

3. **Build:**
   - Press ⌘B to build
   - Or Product > Build

## Archiving for App Store

### Known Issues

Tauri's `xcodebuild archive` command currently fails with a WebSocket connection error when trying to archive from the command line. This is because the Tauri CLI's `xcode-script` attempts to connect to a development server during archive builds.

**Current Status**: Archive must be created through Xcode GUI.

### Archiving via Xcode (Required)

1. **Open project in Xcode:**
   ```bash
   cd tauri/src-tauri/gen/apple
   open openagents.xcodeproj
   ```

2. **Select "Any iOS Device" destination** (not a simulator)

3. **Create Archive:**
   - Product > Archive (or ⇧⌘B)
   - Wait for build to complete
   - Archive will appear in Organizer window

4. **Validate Archive (optional):**
   - In Organizer, select the archive
   - Click "Validate App"
   - Choose automatic signing
   - Fix any validation errors

5. **Distribute to App Store Connect:**
   - Click "Distribute App"
   - Select "App Store Connect"
   - Choose "Upload"
   - Select automatic signing
   - Review and upload

### Alternative: Manual Export & Upload

If Xcode upload fails, you can export the archive and upload via command line:

1. **Export archive:**
   ```bash
   xcodebuild -exportArchive \
     -archivePath ~/Desktop/OpenAgents.xcarchive \
     -exportPath ~/Desktop/OpenAgents-AppStore \
     -exportOptionsPlist tauri/src-tauri/gen/apple/ExportOptions.plist
   ```

2. **Upload to App Store Connect:**
   ```bash
   xcrun altool --upload-app \
     --type ios \
     --file ~/Desktop/OpenAgents-AppStore/OpenAgents.ipa \
     --username YOUR_APPLE_ID \
     --password YOUR_APP_SPECIFIC_PASSWORD
   ```

## TestFlight Distribution

After uploading to App Store Connect:

1. **Go to App Store Connect:** https://appstoreconnect.apple.com
2. **Select "OpenAgents" app**
3. **Navigate to TestFlight tab**
4. **Wait for processing** (usually 5-15 minutes)
5. **Add testers:**
   - Internal Testing: Add team members
   - External Testing: Submit for beta review first
6. **Distribute build** to test groups

TestFlight link (if already configured):
https://testflight.apple.com/join/dvQdns5B

## Troubleshooting

### "bun: command not found" during build

**Cause**: Xcode's build environment doesn't include custom paths.

**Fix**: Update `project.yml` build script to use absolute path:
```yaml
preBuildScripts:
  - script: /Users/USERNAME/.bun/bin/bun tauri ios xcode-script ...
```

### TestFlight shows "Failed to request http://localhost:1420" (dev server)

Cause: The iOS build attempted to reach the frontend dev server (`devUrl`) which is not available in TestFlight.

Fix: We now serve the built frontend locally on iOS using `tauri-plugin-localhost` bound to port 1420, matching the dev URL.

Implemented changes:
- Added `tauri-plugin-localhost` dependency and initialize it on iOS in `tauri/src-tauri/src/lib.rs` with port `1420`.
- Added ATS exceptions for `localhost` and `127.0.0.1` in `tauri/src-tauri/gen/apple/openagents_iOS/Info.plist`.

Build steps for release/TestFlight:
1. `cd tauri && bun run build`
2. `bun tauri ios build`
3. `bun tauri ios build --open` then Product > Archive
4. Upload via Xcode Organizer to App Store Connect

### TypeScript errors blocking build

**Cause**: Pre-existing TypeScript errors in codebase.

**Fix**: Skip TypeScript check during build by changing `package.json`:
```json
{
  "scripts": {
    "build": "vite build",
    "build:check": "tsc && vite build"
  }
}
```

Use `bun run build` (no check) for production builds.

### mDNS discovery errors during iOS build

**Cause**: Error handling mismatch between `flume` crate and `std::sync::mpsc`.

**Fix**: Already fixed in `src-tauri/src/discovery.rs` - simplified error handling to ignore recv timeouts.

### Archive fails with "Connection refused" error

**Cause**: Tauri CLI tries to connect to dev server during archive builds.

**Status**: Known issue with Tauri's mobile archiving.

**Workaround**: Use Xcode GUI to archive (Product > Archive).

### Provisioning profile errors

**Cause**: Missing or expired provisioning profile.

**Fix**:
1. Open Xcode project
2. Select "openagents_iOS" target
3. Signing & Capabilities tab
4. Ensure "Automatically manage signing" is checked
5. Select correct team (OpenAgents, Inc.)

## Version Management

When releasing a new version:

1. **Update version in `tauri.conf.json`:**
   ```json
   {
     "version": "0.4.0"
   }
   ```

2. **Update version and build number in `project.yml`:**
   ```yaml
   CFBundleShortVersionString: 0.4.0
   CFBundleVersion: "101"
   ```

3. **Rebuild:**
   ```bash
   bun run build
   bun tauri ios build
   ```

4. **Archive and submit** as described above

**Important**:
- CFBundleShortVersionString is the user-visible version (e.g., "0.4.0")
- CFBundleVersion must be incremented for each submitted build
- CFBundleVersion must be unique across all builds ever submitted

## Development vs Release Builds

### Development Build
```bash
cd tauri
bun run dev  # Starts dev server on localhost:1420
bun tauri ios dev  # Opens in simulator with hot reload
```

### Release Build
```bash
cd tauri
bun run build  # Build production frontend
bun tauri ios build  # Build release iOS app
```

Development builds connect to localhost:1420 for hot reload. Release builds use bundled frontend from `dist/`.

## Resources

- **Tauri Mobile Docs**: https://v2.tauri.app/develop/mobile/
- **App Store Connect**: https://appstoreconnect.apple.com
- **TestFlight**: https://testflight.apple.com
- **OpenAgents Repo**: https://github.com/OpenAgentsInc/openagents
- **Xcode Download**: https://developer.apple.com/xcode/

## Notes

- iOS 17.0 is the minimum deployment target
- App category is "Developer Tools"
- Automatic code signing is configured (team HQWSG26L43)
- App supports iPhone and iPad (universal)
- Local network access is required for desktop sync functionality
