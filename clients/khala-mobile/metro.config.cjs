const { getDefaultConfig } = require("expo/metro-config")
const { withStorybook } = require("@storybook/react-native/withStorybook")
const { withNativeWind } = require("nativewind/metro")

const config = getDefaultConfig(__dirname)
const defaultResolveRequest =
  typeof config.resolver.resolveRequest === "function"
    ? config.resolver.resolveRequest
    : undefined

config.resolver.resolveRequest = (context, moduleName, platform) => {
  if (moduleName.startsWith(".") && moduleName.endsWith(".js")) {
    const withoutJs = moduleName.slice(0, -3)
    for (const candidate of [`${withoutJs}.ts`, `${withoutJs}.tsx`]) {
      try {
        return context.resolveRequest(context, candidate, platform)
      } catch {
        // Continue to Metro's normal resolver below.
      }
    }
  }

  return defaultResolveRequest === undefined
    ? context.resolveRequest(context, moduleName, platform)
    : defaultResolveRequest(context, moduleName, platform)
}

module.exports = withStorybook(withNativeWind(config, {
  input: "./global.css"
}))
