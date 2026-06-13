import type { ExpoConfig } from "expo/config"

const config: ExpoConfig = {
  name: "Autopilot",
  slug: "autopilot-remote-control",
  // EAS account that owns this project (from `eas init`).
  owner: "openagents",
  scheme: "autopilot-remote-control",
  version: "0.1.0",
  // EAS Update: fingerprint runtime + the project's update server. JS-only
  // changes ship OTA to matching builds via `eas update`; a changed fingerprint
  // (native/config) requires a new build. See src/updates/README.md.
  runtimeVersion: { policy: "fingerprint" },
  updates: {
    url: "https://u.expo.dev/33dc1fb6-1b11-486d-baa0-7946302fdc68",
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
