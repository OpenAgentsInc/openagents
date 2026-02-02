# Onyx macOS Code Signing & Notarization

This guide explains how to set up code signing and notarization for distributing Onyx as a signed macOS application.

## Prerequisites

1. **Apple Developer Account** ($99/year) - https://developer.apple.com
2. **Xcode** installed (for `codesign`, `notarytool`, `stapler`)
3. **Developer ID Application certificate**

## Step 1: Create Developer ID Certificate

1. Go to https://developer.apple.com/account/resources/certificates/list
2. Click "+" to create a new certificate
3. Select **"Developer ID Application"** (NOT "Mac App Distribution")
4. Follow the prompts to create a Certificate Signing Request (CSR) from Keychain Access
5. Download the certificate and double-click to install in Keychain

## Step 2: Export Certificate as .p12

1. Open **Keychain Access**
2. Find your "Developer ID Application: Your Name" certificate
3. Right-click → **Export**
4. Save as `.p12` format
5. Set a strong password (you'll need this later)

## Step 3: Base64 Encode the Certificate

```bash
base64 -i ~/path/to/certificate.p12 | tr -d '\n' > certificate.txt
```

The contents of `certificate.txt` is your `MACOS_CERTIFICATE` value.

## Step 4: Create App Store Connect API Key

For notarization, you need an API key:

1. Go to https://appstoreconnect.apple.com/access/integrations/api
2. Click "+" to generate a new key
3. Give it a name like "Onyx Notarization"
4. Select **"Developer"** access (minimum required for notarization)
5. Download the `.p8` file (you can only download it once!)
6. Note the **Key ID** shown in the table
7. Note your **Issuer ID** at the top of the page

## Environment Variables

Set these environment variables for the build script:

### Code Signing

```bash
# Your signing identity (find with: security find-identity -v -p codesigning)
export MACOS_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"

# Base64-encoded .p12 certificate
export MACOS_CERTIFICATE="MIIKkQIBAz..."

# Password for the .p12 file
export MACOS_CERTIFICATE_PASSWORD="your-certificate-password"
```

### Notarization

```bash
# Contents of the .p8 API key file
export APPLE_NOTARIZATION_KEY="-----BEGIN PRIVATE KEY-----
MIGTAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBHkwdwIBAQQg...
-----END PRIVATE KEY-----"

# Key ID from App Store Connect (e.g., ABC123XYZ)
export APPLE_NOTARIZATION_KEY_ID="ABC123XYZ"

# Issuer ID from App Store Connect (UUID format)
export APPLE_NOTARIZATION_ISSUER_ID="12345678-1234-1234-1234-123456789012"
```

## Usage

### Local Development (ad-hoc signed)

No certificates needed, just sign with ad-hoc:

```bash
./scripts/bundle-mac --sign --install
```

### Release Build with DMG

```bash
./scripts/bundle-mac --sign --dmg
```

Output: `target/aarch64-apple-darwin/release/Onyx-aarch64.dmg`

### Full Release with Notarization

```bash
./scripts/bundle-mac --sign --dmg --notarize
```

This will:
1. Build the release binary
2. Create the .app bundle
3. Sign with Developer ID
4. Create DMG
5. Sign the DMG
6. Submit to Apple for notarization
7. Wait for approval (~2-5 minutes)
8. Staple the notarization ticket

## Finding Your Signing Identity

List available identities:

```bash
security find-identity -v -p codesigning
```

Look for "Developer ID Application: ..." entries.

## Troubleshooting

### "The signature is invalid"

Make sure you're using a **Developer ID Application** certificate, not:
- Mac App Distribution (App Store only)
- Apple Development (development only)
- 3rd Party Mac Developer (App Store only)

### Notarization Fails

Check the log:
```bash
xcrun notarytool log <submission-id> \
  --key "$APPLE_NOTARIZATION_KEY_FILE" \
  --key-id "$APPLE_NOTARIZATION_KEY_ID" \
  --issuer "$APPLE_NOTARIZATION_ISSUER_ID"
```

Common issues:
- Missing hardened runtime (`--options runtime`)
- Unsigned nested binaries
- Invalid entitlements

### "Developer cannot be verified"

The app hasn't been notarized, or the ticket hasn't been stapled. Run:

```bash
xcrun stapler staple /path/to/Onyx.dmg
```

Or for already-installed apps:
```bash
xattr -cr /Applications/Onyx.app
```

## GitHub Actions

For CI/CD, store secrets in GitHub:

```yaml
env:
  MACOS_SIGNING_IDENTITY: ${{ secrets.MACOS_SIGNING_IDENTITY }}
  MACOS_CERTIFICATE: ${{ secrets.MACOS_CERTIFICATE }}
  MACOS_CERTIFICATE_PASSWORD: ${{ secrets.MACOS_CERTIFICATE_PASSWORD }}
  APPLE_NOTARIZATION_KEY: ${{ secrets.APPLE_NOTARIZATION_KEY }}
  APPLE_NOTARIZATION_KEY_ID: ${{ secrets.APPLE_NOTARIZATION_KEY_ID }}
  APPLE_NOTARIZATION_ISSUER_ID: ${{ secrets.APPLE_NOTARIZATION_ISSUER_ID }}
```

## Distribution

After notarization, the DMG can be:

1. **Direct download** - Host on your website/CDN
2. **GitHub Releases** - Attach to release
3. **Homebrew Cask** - Create a cask formula

Users can verify notarization:
```bash
spctl --assess --verbose /Applications/Onyx.app
# Should output: "accepted" and "source=Notarized Developer ID"
```
