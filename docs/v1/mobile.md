# OpenAgents v1.0.0 — Mobile Specification (iOS — 100% Cloud Mode, Zero Tailscale)

**Target Artifact:** `OpenAgents.app` (`apps/openagents-ios/`, iOS 17+, public TestFlight `https://testflight.apple.com/join/dvQdns5B`)  
**Architecture Shift:** Complete removal of Tailscale / Tailnet / MagicDNS / 100.x / Bonjour LAN pairing; unconditional OpenAgents Cloud mode across the entire iOS app.

---

## 1. Core Architectural Contract

OpenAgents for iOS v1.0.0 is a **pure OpenAgents Cloud mobile client** rendered on the shared **pure-black (`#000000`) Metal Coax GPU UI** (`GpuRootViewController` + `gpu-page.coax` + `ComponentsHost.m`) with native iOS platform bridges strictly for system auth, Keychain, Spark Wallet, and voice dictation.

### Hard Zero-Tailscale Invariant
- `apps/openagents-ios/` MUST contain **zero** references to Tailscale, Tailnet, MagicDNS, `100.64.x` / `100.x` IP pairing, Bonjour local peer discovery, QR code Mac pairing, `AppTailnet.swift`, `TailscaleClient.swift`, `tailnet-codec.h`, or `NSLocalNetworkUsageDescription`.
- Verification gate: `git grep -iE "(tailscale|tailnet|magicdns|100\.64\.)" apps/openagents-ios/` returns **0 active matches**.
- There is **no** "Local vs Cloud" mode switch (`preferCloudMode` toggle is eliminated): Cloud mode is the sole runtime mode.
- Shared macOS files outside `apps/openagents-ios/` (such as `apps/shared/native/AppTailnet.swift`) remain untouched so Desktop is never broken.

---

## 2. Feature Scope (iOS v1.0.0)

### 2.1 Authentication & Identity (`AccountView`, `MobileAccountController`)
- **WorkOS Authentication (Apple App Store Guideline 4.8 compliant):**
  - Sign in with Apple (`ASWebAuthenticationSession`, `openagents://auth/callback`, `prefersEphemeralWebBrowserSession = false`)
  - Sign in with Google, Sign in with GitHub, Enterprise SSO (domain discovery + multi-org picker & signed-in org switcher)
  - Magic Auth (6-digit email code) and Email + Password
- **First-Party Sovereignty (`L2` / `L2-fresh`):**
  - System Passkey registration & step-up (`AuthenticationServices`)
  - Account Nostr identity (public key copy, Face ID / device-auth protected Backup / Rotate / Restore)
  - Device-bound `oa_at_` / `oa_rt_` tokens with `OA-Device-Proof` stored in Keychain (`AfterFirstUnlockThisDeviceOnly`)
- **Signed-Out State:** Clean sign-in screen launching directly into `AccountView` — zero "Pair a Mac over Tailscale" onboarding or dead LAN banners.

### 2.2 Synced Cloud Chat (`MobileCloudChat`, `GpuRootViewController`, `gpu-page.coax`)
- **Conversations Sidebar:** Backed directly by `GET /api/chat/v2/conversations`, `POST /api/chat/v2/conversations`, and `DELETE /api/chat/v2/conversations/:id` with live SSE synchronization (`/api/chat/v2/stream` & `/api/chat/v2/conversations/:id/stream`) across Web, Desktop, and iOS.
- **Streaming Cloud Turns:**
  - Direct streaming via `POST /api/v1/inference` with Tier 1 Cloud Tools (`/api/tools/cloud/v1`) and Gemini `thought_signature` preservation.
  - Durable turns via `POST /api/chat/submit/v2alpha1` when targeting an enrolled computer or Cloud Sandbox, with live SSE event streaming, compact expandable tool rows, copyable replies/code, and Stop/Cancel.
- **No Third-Party Branding:** Zero Antigravity or retired branding in any iOS UI string or screen (`da6ab05135`).

### 2.3 Cloud Environments & Projects (`EnvironmentsView` / Cloud sheet replacing Tailscale `ComputersView`)
- **Enrolled Computers (`GET /api/chat/environments/v1`):** Lists every Mac or Linux workstation enrolled under the user's OpenAgents account, online/offline heartbeat status, and bound git repositories — reached exclusively through OpenAgents Cloud with zero VPN/Tailscale configuration.
- **Cloud Projects (`GET /api/projects/v1`):** Browse and select account projects (with linked GitHub repos) so chats automatically pick the right context and enrolled computer.
- **Execution Target Picker:** Choose `Auto` (`POST /api/chat/placement/v1`), `Cloud`, `Cloud Sandbox` (GCE `oa-boat`), or any specific online enrolled computer.

### 2.4 Spark Bitcoin/Lightning Wallet (`WalletView`)
- Self-custodial Spark Bitcoin/Lightning wallet with isolated wallet Keychain custody (separate from account Nostr identity), live balance read-back, Lightning invoice pay/receive, and `L2-fresh` biometric/passkey backup.
- Account credit status displays both **Paid Credit (sats)** and **Promo Credit (USD)** from `/api/auth/account`.

### 2.5 Native Input & Polish
- Voice Dictation, pure-black Metal Coax GPU surface, dynamic type / VoiceOver accessibility labels, native keyboard dismissal, and safe API origin configuration (`https://openagents.com` default; configurable staging override for pre-cutover testing).

---

## 3. Build, Verification & TestFlight Release Gates

Before shipping iOS v1.0.0 to public TestFlight:

1. **Automated Simulator & Coax Gates (all must pass with 0 failures):**
   - `git grep -iE "(tailscale|tailnet|magicdns|100\.64\.)" apps/openagents-ios/` → **0 matches**
   - `./coax/bin/coax test apps/openagents-ios/gpu-page.coax` (and all `.coax` units under `apps/openagents-ios/`)
   - `apps/openagents-ios/check.sh`
   - `apps/openagents-ios/build.sh` (clean Xcode simulator build via regenerated `OpenAgents.xcodeproj`)
   - `xcodebuild test -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgents -only-testing:OpenAgentsTests -destination 'platform=iOS Simulator,id=<SIM_ID>'`
2. **Physical iPhone Sign-Off (Owner — TestFlight before opening public link on Monday):**
   - Fresh install from Owner TestFlight group (build > `1431`, Cloud-only).
   - Sign in with Apple and/or Magic Auth against production `https://openagents.com` (with `OA_WORKOS_ENABLED=1`).
   - Verify synced chats from Web appear automatically; send a live Cloud turn (with web search / GitHub tool) and confirm streaming completes cleanly.
   - Open Cloud Environments & Projects sheet, confirm enrolled Mac appears online (with Tailscale **OFF** on the iPhone!), and dispatch a durable turn to the enrolled Mac or a Cloud Sandbox.
   - Verify Spark Wallet balance + Face ID / Passkey step-up backup, VoiceOver, and background/resume reconnect.

---

## 4. Android Status (Explicitly Post-v1.0.0)

- Android WorkOS Phase 4 authentication (`apps/shared/app-login-flow.coax` `631/631` tests, NDK `aarch64-linux-android35-clang -Werror` clean, App Link PKCE + Keystore AES-256-GCM) is already verified and merged (`#14101`).
- Full Android mobile app packaging and Play Store rollout are scheduled **post-v1.0.0** and do not block Monday's launch.
