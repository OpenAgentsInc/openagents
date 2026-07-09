const { getDefaultConfig } = require("expo/metro-config")

const metroConfig = getDefaultConfig(__dirname)
const platforms = ["ios", "android", "web", "native"]
const sourceExts = metroConfig?.resolver?.sourceExts ?? ["ts", "tsx", "js", "jsx", "json"]
const extensions = sourceExts.flatMap(ext =>
  platforms.map(platform => `.${platform}.${ext}`).concat(`.${ext}`),
)

const productionPath = "^(index\\.tsx|src/)"
const productionFixturePath = "\\.(spec|test|stories)\\.(js|mjs|cjs|ts|tsx)$"
const domainPath = "^src/(auth|config|native|security|status|sync|theme)/"
const routePath = "^src/(navigators|screens)/"
const nativeModulePath =
  "(^|/)(node_modules/)?khala-(apple-foundation-models|push-to-talk-stt)(/|$)|(^|/)modules/khala-(apple-foundation-models|push-to-talk-stt)(/|$)"

/** @type {import("dependency-cruiser").IConfiguration} */
module.exports = {
  forbidden: [
    {
      name: "no-circular",
      severity: "error",
      comment:
        "The React Navigation migration settled with zero circular imports across index.tsx, src, and tests. Tightened from warn to error in #8454 so new cycles fail architecture:check instead of accumulating as warnings.",
      from: {},
      to: { circular: true },
    },
    {
      name: "no-production-imports-from-tests",
      severity: "error",
      comment:
        "Shipping app/source files must never depend on tests or test support.",
      from: {
        path: productionPath,
        pathNot: productionFixturePath,
      },
      to: {
        path: "^tests/",
      },
    },
    {
      name: "no-non-package-json",
      severity: "error",
      comment:
        "NPM imports used by Khala Mobile must be declared in this package's dependencies/devDependencies.",
      from: {},
      to: {
        dependencyTypes: ["npm-no-pkg", "npm-unknown"],
      },
    },
    {
      name: "not-to-unresolvable",
      severity: "error",
      comment:
        "Dependency Cruiser could not resolve this import with the Expo/Metro/TypeScript extensions below. Bun's builtin test/plugin module is provided by the runtime.",
      from: {},
      to: {
        couldNotResolve: true,
        pathNot: "^bun$",
      },
    },
    {
      name: "not-to-dev-dep-from-production",
      severity: "error",
      comment:
        "Production app/source files cannot import devDependencies; move the import to tests/tooling or promote the package.",
      from: {
        path: productionPath,
        pathNot: productionFixturePath,
      },
      to: {
        dependencyTypes: ["npm-dev"],
        pathNot: ["node_modules/@types/"],
      },
    },
    {
      name: "domain-must-not-import-routes",
      severity: "error",
      comment:
        "Domain modules can be used by screens/navigators, but auth/sync/security/native/theme/etc. must not import route ownership back upward.",
      from: {
        path: domainPath,
      },
      to: {
        path: routePath,
      },
    },
    {
      name: "native-modules-through-adapter",
      severity: "error",
      comment:
        "Expo native module packages are imported only by src/native/modules.ts so unavailable/native-host states stay centralized.",
      from: {
        path: "^src/(?!native/modules\\.ts$)",
      },
      to: {
        path: nativeModulePath,
        dependencyTypesNot: ["type-only"],
      },
    },
  ],
  options: {
    doNotFollow: {
      // Treat the vendored `@effect-native/*` workspace packages as external
      // (like node_modules): the mobile app depends on them (declared in
      // package.json) but must not lint their INTERNAL source against these
      // route/domain rules. They live under apps/openagents.com/packages/ (not
      // node_modules), so name them explicitly. EN-3 (#8568).
      path: "node_modules|/effect-native-(core|tokens|render-dom|render-rn)/",
    },
    enhancedResolveOptions: {
      conditionNames: ["react-native", "import", "require", "node", "default"],
      extensions,
      exportsFields: ["exports"],
      mainFields: ["react-native", "browser", "module", "main"],
    },
    reporterOptions: {
      archi: {
        collapsePattern: "^(src|tests)/[^/]+",
      },
      dot: {
        collapsePattern: "node_modules/(@[^/]+/[^/]+|[^/]+)",
      },
      text: {
        highlightFocused: true,
      },
    },
    tsConfig: {
      fileName: "tsconfig.json",
    },
    tsPreCompilationDeps: true,
  },
}
