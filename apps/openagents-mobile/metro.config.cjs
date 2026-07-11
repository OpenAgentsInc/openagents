const { getDefaultConfig } = require("expo/metro-config")

const config = getDefaultConfig(__dirname)
const defaultResolveRequest = config.resolver.resolveRequest

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
