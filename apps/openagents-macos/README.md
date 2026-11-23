# OpenAgents macOS

Clean-sheet native SwiftUI macOS client and Tailscale Host companion for **Antigravity over Tailscale** (GitHub [#13993](https://github.com/OpenAgentsInc/openagents/issues/13993)).

## Architecture

- **100% shared core with OpenAgents iOS (`apps/shared/antigravity/`):**
  - Compiled Coax C core (`client-core.coax` → `Generated/client_core.{c,h}` + `client-core-bridge.{h,c}`) for NIP-44 crypto, Tailscale admission/direct-grant/push wire codecs, Environment + Workspace Git/PR wire, and `#authority` Antigravity-only model policy.
  - Shared cross-Apple Swift engine (`AntigravityStore`, `TailscaleClient`, `LocalSocketClient`, `DeviceIdentityStore`, `TranscriptComponents`, `WorkspaceInspectorView`, `SubagentInspectorView`).
- **Dual role on macOS:**
  - **Local + Remote Antigravity client:** Talks to this Mac's `coder-host` instantly via `~/.openagents/coder/host/service.sock` (owner-only `AF_UNIX`), and connects to any remote Mac/Linux host over direct Tailscale TCP (port `47109`, zero public relays).
  - **Tailscale Host Server + Menu Bar Companion:** Advertises `_openagents-coder._tcp`, enforces `tailscale whois`, mints relay-free direct Tailscale grants, forwards sealed NIP-44 RPCs to `service.sock`, broadcasts real-time push frames (`openagents.host-tailnet-push.v1`), and manages the optional `~/Library/LaunchAgents/com.openagents.coder-host.plist` background daemon (`coder-host serve --no-relay`).

## Build & Verify

```bash
apps/openagents-macos/build.sh
apps/openagents-macos/check.sh
apps/openagents-macos/install.sh
xcodebuild test -project apps/openagents-macos/OpenAgents.xcodeproj -scheme OpenAgentsTests -sdk macosx -derivedDataPath apps/openagents-macos/build CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=-
```
