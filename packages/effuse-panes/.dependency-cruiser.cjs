/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    {
      name: "no-circular",
      severity: "error",
      comment:
        "Circular dependencies make refactors and agent navigation harder. Break the cycle (extract shared module, dependency inversion, etc.).",
      from: {},
      to: { circular: true },
    },
    {
      name: "not-to-unresolvable",
      severity: "error",
      comment: "This module depends on something that cannot be resolved.",
      from: {},
      to: { couldNotResolve: true },
    },
    {
      name: "no-non-package-json",
      severity: "error",
      comment: "This module depends on an npm package that isn't declared in package.json.",
      from: {},
      to: { dependencyTypes: ["npm-no-pkg", "npm-unknown"] },
    },
    {
      name: "not-to-spec",
      severity: "error",
      comment: "Production code must not depend on spec/test files.",
      from: { pathNot: "\\.(spec|test)\\.(js|mjs|cjs|ts|tsx)$" },
      to: { path: "\\.(spec|test)\\.(js|mjs|cjs|ts|tsx)$" },
    },
  ],
  options: {
    doNotFollow: { path: "node_modules" },
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

