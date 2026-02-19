# Onyx Auto-Update System

## Overview

Onyx includes a manual update checker that compares your installed version against the latest release on GitHub. When an update is available, it notifies you and opens the release page in your browser.

## Usage

Press **Cmd+Shift+U** (or Ctrl+Shift+U on Linux/Windows) to check for updates.

The status bar will show:
- "Checking for updates..." while checking
- "Onyx X.Y.Z is up to date" if you have the latest version
- "Update available: X.Y.Z - visit github.com/..." if a newer version exists

If an update is available, your browser will automatically open to the GitHub releases page where you can download the latest version.

## How It Works

The update checker queries the GitHub Releases API to find releases tagged with the `onyx-v` prefix:

```
https://api.github.com/repos/OpenAgentsInc/openagents/releases
```

It filters for:
1. Tags starting with `onyx-v` (e.g., `onyx-v0.1.0`, `onyx-v0.2.0`)
2. Non-prerelease versions only

The current version (from `Cargo.toml`) is compared against the latest release using semver. If a newer version exists, the user is notified.

## Multi-Product Support

The OpenAgents monorepo contains multiple products (Onyx, Pylon, etc.). Each product uses its own tag prefix:

| Product | Tag Prefix | Example |
|---------|------------|---------|
| Onyx    | `onyx-v`   | `onyx-v0.1.0` |
| Pylon   | `pylon-v`  | `pylon-v1.0.0` |

This allows independent versioning and release schedules for each product.

## Creating a Release

To create a new Onyx release:

```bash
# Update version in Cargo.toml
# crates/onyx/Cargo.toml -> version = "X.Y.Z"

# Create and push the tag
git tag onyx-vX.Y.Z
git push origin onyx-vX.Y.Z

# Create the GitHub release
gh release create onyx-vX.Y.Z \
  --title "Onyx vX.Y.Z" \
  --notes "Release notes here"
```

To attach binaries to the release:

```bash
# Build the macOS bundle
./scripts/bundle-mac --sign

# Upload to release
gh release upload onyx-vX.Y.Z \
  target/release/bundle/osx/Onyx.app.zip \
  --clobber
```

## Technical Details

### Module Location

The update checker is implemented in:
- `crates/onyx/src/update_checker.rs` - Core update checking logic
- `crates/onyx/src/app.rs` - UI integration (Cmd+Shift+U handler)

### Dependencies

```toml
# In crates/onyx/Cargo.toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
semver = "1"
```

### API Response

The GitHub API returns releases in this format:

```json
[
  {
    "tag_name": "onyx-v0.1.0",
    "html_url": "https://github.com/OpenAgentsInc/openagents/releases/tag/onyx-v0.1.0",
    "prerelease": false,
    "name": "Onyx v0.1.0"
  }
]
```

## Future Enhancements

The current implementation is MVP (manual check only). Planned improvements:

- [ ] Background polling (check every hour)
- [ ] Download DMG directly within the app
- [ ] Auto-install with user confirmation
- [ ] Settings UI for auto-update preferences
- [ ] Update notifications on app startup
