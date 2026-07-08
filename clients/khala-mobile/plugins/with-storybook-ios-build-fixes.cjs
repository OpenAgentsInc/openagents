const { withPodfileProperties, withXcodeProject } = require("@expo/config-plugins")

const APP_TARGET_DEVELOPMENT_TEAM = "HQWSG26L43"

module.exports = function withStorybookIosBuildFixes(config) {
  config = withPodfileProperties(config, (pluginConfig) => {
    pluginConfig.modResults["ios.buildReactNativeFromSource"] = "true"
    return pluginConfig
  })

  return withXcodeProject(config, (pluginConfig) => {
    const buildConfigurations = pluginConfig.modResults.pbxXCBuildConfigurationSection()

    for (const buildConfiguration of Object.values(buildConfigurations)) {
      if (!buildConfiguration || typeof buildConfiguration !== "object" || !buildConfiguration.buildSettings) {
        continue
      }

      const { buildSettings, name } = buildConfiguration
      const isKhalaAppTarget = buildSettings.DEVELOPMENT_TEAM === APP_TARGET_DEVELOPMENT_TEAM
      const isDebugConfiguration = typeof name === "string" && name.includes("Debug")

      if (isKhalaAppTarget && isDebugConfiguration) {
        buildSettings.ENABLE_DEBUG_DYLIB = "NO"
      }
    }

    return pluginConfig
  })
}
