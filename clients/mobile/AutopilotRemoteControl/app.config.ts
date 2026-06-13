import type { ExpoConfig } from "expo/config"

const config: ExpoConfig = {
  name: "Autopilot Remote Control",
  slug: "autopilot-remote-control",
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
}

export default config
