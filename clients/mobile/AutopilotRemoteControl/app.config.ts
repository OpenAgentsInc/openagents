import type { ExpoConfig } from "expo/config"

const config: ExpoConfig = {
  name: "Autopilot Remote Control",
  slug: "autopilot-remote-control",
  // EAS account that owns this project (from `eas init`).
  owner: "openagents",
  scheme: "autopilot-remote-control",
  version: "0.1.0",
  orientation: "portrait",
  userInterfaceStyle: "automatic",
  ios: {
    bundleIdentifier: "com.openagents.autopilot-mobile",
    supportsTablet: false,
    infoPlist: {
      // No non-exempt encryption (HTTPS/standard crypto only) — skips the
      // App Store Connect export-compliance prompt for TestFlight.
      ITSAppUsesNonExemptEncryption: false,
    },
  },
  android: {
    // Android package segments can't contain hyphens; mirror without it.
    package: "com.openagents.autopilotmobile",
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
