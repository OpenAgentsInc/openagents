import type { ExpoConfig } from "expo/config"

const config: ExpoConfig = {
  name: "Autopilot Remote Control",
  slug: "autopilot-remote-control",
  scheme: "autopilot-remote-control",
  version: "0.1.0",
  orientation: "portrait",
  userInterfaceStyle: "automatic",
  ios: {
    bundleIdentifier: "com.openagents.autopilot.remote",
    supportsTablet: false,
  },
  android: {
    package: "com.openagents.autopilot.remote",
  },
}

export default config
