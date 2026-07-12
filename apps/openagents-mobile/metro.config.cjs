const { getDefaultConfig } = require("expo/metro-config")
const { createRequire } = require("node:module")

const config = getDefaultConfig(__dirname)
const defaultResolveRequest = config.resolver.resolveRequest
const defaultRewriteRequestUrl = config.server.rewriteRequestUrl
const expoRequire = createRequire(require.resolve("expo/metro-config"))

// This app uses a plain React Native debug host rather than expo-dev-client.
// Keep dynamic native-module imports in the main bundle so Expo's split-bundle
// loader does not attempt to register an HMR client that this host never sets up.
config.transformer.asyncRequireModulePath = expoRequire.resolve(
  "metro-runtime/src/modules/asyncRequire.js",
)
config.server.rewriteRequestUrl = (url) => {
  const rewritten = typeof defaultRewriteRequestUrl === "function"
    ? defaultRewriteRequestUrl(url)
    : url
  const absolute = rewritten.startsWith("/")
    ? new URL(rewritten, "http://localhost")
    : new URL(rewritten)
  if (absolute.pathname.endsWith(".bundle")) {
    absolute.searchParams.set("lazy", "false")
  }
  return rewritten.startsWith("/")
    ? `${absolute.pathname}${absolute.search}`
    : absolute.toString()
}

// Shared NodeNext packages use explicit `.js` specifiers so their emitted ESM
// is valid. Metro consumes their TypeScript source directly, so map only those
// relative specifiers back to the matching source file before normal resolution.
config.resolver.resolveRequest = (context, moduleName, platform) => {
  if (moduleName.startsWith(".") && moduleName.endsWith(".js")) {
    const withoutJs = moduleName.slice(0, -3)
    for (const candidate of [`${withoutJs}.ts`, `${withoutJs}.tsx`]) {
      try {
        return context.resolveRequest(context, candidate, platform)
      } catch {
        // Continue to Metro's normal resolver.
      }
    }
  }

  return typeof defaultResolveRequest === "function"
    ? defaultResolveRequest(context, moduleName, platform)
    : context.resolveRequest(context, moduleName, platform)
}

module.exports = config
