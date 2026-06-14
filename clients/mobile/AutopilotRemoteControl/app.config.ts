import type { ExpoConfig } from "expo/config"

const config: ExpoConfig = {
  name: "Autopilot",
  slug: "autopilot-remote-control",
  // EAS account that owns this project (from `eas init`).
  owner: "openagents",
  scheme: "autopilot-remote-control",
  version: "0.1.0",
  // CL-30: expo-notifications config plugin (local notifications on session
  // state changes). Native module — ships in the next local build.
  plugins: ["expo-notifications"],
  // Expo Updates runtime + our own OpenAgents Updates server. JS-only changes
  // ship OTA to matching builds via apps/oa-updates/scripts/publish-ota.sh; a
  // changed fingerprint (native/config) requires a new local build.
  runtimeVersion: { policy: "fingerprint" },
  updates: {
    // Off Expo's CDN: our own OpenAgents Updates server (Cloud Run behind the
    // updates.openagents.com CNAME). expo-updates GETs this manifest endpoint
    // with Expo-* headers; our server serves a fingerprint-matched manifest.
    url: "https://updates.openagents.com/autopilot/manifest",
    // #4949 code signing: the client verifies every manifest is signed by our
    // server's private key (the matching public cert is embedded here). The
    // server signs with OA_SIGNING_KEY, keyid "main", alg rsa-v1_5-sha256.
    // Takes effect on the next native build (a config change).
    codeSigningCertificate: "./certs/codesign.pem",
    codeSigningMetadata: { keyid: "main", alg: "rsa-v1_5-sha256" },
  },
  orientation: "portrait",
  userInterfaceStyle: "automatic",
  // App icon: the Control power-symbol mark (white glyph on black), sourced
  // from the Control iOS app's 1024×1024 AppIcon. Expo derives all sizes.
  icon: "./assets/icon.png",
  ios: {
    bundleIdentifier: "com.openagents.autopilot-mobile",
    supportsTablet: false,
    infoPlist: {
      // No non-exempt encryption (HTTPS/standard crypto only) — skips the
      // App Store Connect export-compliance prompt for TestFlight.
      ITSAppUsesNonExemptEncryption: false,
      // Allow cleartext HTTP to a Pylon node over tailnet/LAN (e.g.
      // http://100.x.x.x:4716). iOS ATS blocks plain HTTP by default in release
      // builds, which is why connecting "saw nothing". Dev/internal app → allow
      // arbitrary loads; tighten to a scoped exception once the node is HTTPS.
      NSAppTransportSecurity: {
        NSAllowsArbitraryLoads: true,
        NSAllowsLocalNetworking: true,
      },
      // iOS prompts for local-network access on first LAN/bonjour reach.
      NSLocalNetworkUsageDescription:
        "Autopilot connects to your Pylon node over your local network or tailnet.",
    },
  },
  android: {
    // Android package segments can't contain hyphens; mirror without it.
    package: "com.openagents.autopilotmobile",
    adaptiveIcon: {
      foregroundImage: "./assets/adaptive-icon.png",
      backgroundColor: "#000000",
    },
  },
  extra: {
    // EAS project link (from `eas init`). Required because this is a dynamic
    // app config — EAS can't auto-write it, so it lives here explicitly.
    eas: {
      projectId: "33dc1fb6-1b11-486d-baa0-7946302fdc68",
    },
  },
}

export default config
