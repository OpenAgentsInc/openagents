/* eslint-disable no-undef */
/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    {
      name: "no-circular",
      severity: "error",
      comment:
        "Circular dependencies make agent work and refactors substantially harder. Break the cycle (extract shared module, dependency inversion, etc.).",
      from: {},
      to: { circular: true },
    },
    {
      name: "not-to-unresolvable",
      severity: "error",
      comment:
        "This module depends on something that cannot be resolved. Fix the import path or add the dependency.",
      from: { pathNot: "\\.d\\.ts$" },
      to: { couldNotResolve: true },
    },
    {
      name: "no-non-package-json",
      severity: "error",
      comment:
        "This module depends on an npm package that isn't declared in package.json dependencies/devDependencies.",
      from: {},
      to: { dependencyTypes: ["npm-no-pkg", "npm-unknown"] },
    },
    {
      name: "not-to-dev-dep",
      severity: "error",
      comment:
        "Production code (src/) must not depend on devDependencies. Move the package to dependencies or restrict usage to dev/test paths.",
      from: {
        path: "^src",
        pathNot: ["\\.d\\.ts$", "\\.(spec|test)\\.(js|mjs|cjs|ts|tsx)$"],
      },
      to: { dependencyTypes: ["npm-dev"], pathNot: ["node_modules/@types/"] },
    },

    // Layering rules (agent-legibility guardrails):
    {
      name: "effect-must-not-depend-on-ui",
      severity: "error",
      comment:
        "Effect/service layer must not depend on UI. Move shared logic to src/lib (or a package), then depend upward from UI.",
      from: { path: "^src/effect" },
      to: { path: "^src/(components|effuse-pages|routes|storybook|effuse-app|effuse-deck)" },
    },
    {
      name: "worker-host-must-not-depend-on-ui",
      severity: "error",
      comment:
        "Worker host code must not depend on UI modules. Keep host/runtime boundaries clean for determinism and deploy safety.",
      from: { path: "^src/effuse-host", pathNot: "^src/effuse-host/storybook\\.ts$" },
      to: { path: "^src/(components|effuse-pages|routes|storybook)" },
    },
    {
      name: "no-storybook-from-non-storybook",
      severity: "error",
      comment:
        "Only storybook integration surfaces may depend on src/storybook/*. (If you need storybook in another place, route through an explicit integration module.)",
      from: {
        pathNot: [
          "^src/storybook",
          "^src/effuse-app/routes\\.ts$",
          "^src/effuse-pages/storybook\\.ts$",
          "^src/effuse-host/storybook\\.ts$",
          "^src/effuse-deck/render\\.ts$",
        ],
      },
      to: { path: "^src/storybook" },
    },
  ],
  options: {
    // `apps/web` should not lint internals of workspace packages (they each have their own structure lints).
    // Without this, depcruise can follow file: deps outside node_modules and report false positives.
    doNotFollow: { path: ["node_modules", "^\\.\\./\\.\\./packages/"] },
    tsPreCompilationDeps: true,
    tsConfig: { fileName: "tsconfig.json" },
    enhancedResolveOptions: {
      exportsFields: ["exports"],
      conditionNames: ["import", "require", "node", "default"],
    },
    reporterOptions: {
      text: { highlightFocused: true },
    },
  },
}
