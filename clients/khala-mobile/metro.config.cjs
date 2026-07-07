const path = require("node:path")
const { getDefaultConfig } = require("expo/metro-config")
const { withStorybook } = require("@storybook/react-native/withStorybook")
const { withNativeWind } = require("nativewind/metro")

const config = getDefaultConfig(__dirname)
const defaultResolveRequest =
  typeof config.resolver.resolveRequest === "function"
    ? config.resolver.resolveRequest
    : undefined

const storybookLogBoxAliases = new Map([
  ["./UI/LogBoxNotification", ".rnstorybook/logbox/LogBoxNotification.js"],
  ["./UI/LogBoxNotification.js", ".rnstorybook/logbox/LogBoxNotification.js"],
  ["./UI/LogBoxStyle", ".rnstorybook/logbox/LogBoxStyle.js"],
  ["./UI/LogBoxStyle.js", ".rnstorybook/logbox/LogBoxStyle.js"],
  ["./LogBoxNotification", ".rnstorybook/logbox/LogBoxNotification.js"],
  ["./LogBoxNotification.js", ".rnstorybook/logbox/LogBoxNotification.js"],
  ["./LogBoxStyle", ".rnstorybook/logbox/LogBoxStyle.js"],
  ["./LogBoxStyle.js", ".rnstorybook/logbox/LogBoxStyle.js"],
])

const isLogBoxOrigin = (originModulePath) =>
  typeof originModulePath === "string" &&
  originModulePath.includes(`${path.sep}react-native${path.sep}Libraries${path.sep}LogBox${path.sep}`)

config.resolver.resolveRequest = (context, moduleName, platform) => {
  if (
    process.env.STORYBOOK_ENABLED === "true" &&
    moduleName === "./src/app" &&
    typeof context.originModulePath === "string" &&
    context.originModulePath.endsWith(`${path.sep}index.tsx`)
  ) {
    return context.resolveRequest(
      context,
      path.resolve(__dirname, ".rnstorybook/app-root.ts"),
      platform,
    )
  }

  if (process.env.STORYBOOK_ENABLED === "true" && isLogBoxOrigin(context.originModulePath)) {
    const alias = storybookLogBoxAliases.get(moduleName)
    if (alias !== undefined) {
      return context.resolveRequest(context, path.resolve(__dirname, alias), platform)
    }
  }

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
