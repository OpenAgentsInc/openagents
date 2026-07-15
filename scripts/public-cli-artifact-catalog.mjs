export const publicCliArtifacts = [
  { root: "packages/agent-readiness", name: "@openagentsinc/agent-readiness", entries: { index: "src/index.ts", cli: "src/cli.ts" }, bin: { "agent-readiness": "dist/cli.mjs" } },
  { root: "packages/product-spec", name: "@openagentsinc/product-spec", entries: { index: "src/index.ts", cli: "src/cli.ts" }, bin: { "product-spec": "dist/cli.mjs" } },
  { root: "packages/assurance-spec", name: "@openagentsinc/assurance-spec", entries: { index: "src/index.ts", browser: "src/browser.ts", cli: "src/cli.ts" }, bin: { "assurance-spec": "dist/cli.mjs" }, assets: ["skills", "starter-kit"] },
  { root: "apps/qa-runner", name: "@openagentsinc/qa-runner", entries: { index: "src/index.ts", byo: "src/byo.ts" }, bin: { qa: "dist/byo.mjs" }, eagerDts: true },
  {
    root: "apps/pylon/packages/runtime",
    name: "@openagentsinc/pylon-runtime",
    entries: { index: "src/index.ts", cli: "src/cli.ts" },
    bin: { "pylon-runtime": "dist/cli.mjs" },
    externalInternalDts: true,
    typeDependencies: [
      { name: "@openagentsinc/blueprint-contracts", root: "packages/blueprint-contracts" },
      { name: "@openagentsinc/provider-account-schema", root: "packages/provider-account-schema" },
    ],
  },
  { root: "apps/pylon", name: "@openagentsinc/pylon", entries: { index: "src/index.ts" }, bin: { pylon: "dist/index.mjs", "pylon-foundation-bridge": "bin/foundation-bridge" }, assets: ["bin/foundation-bridge", "swift", "docs", "README.md", "LICENSE"] },
]

export const sourceEntriesFor = (record, manifest) => {
  const entries = { ...record.entries }
  if (record.entriesFromExports) {
    for (const [subpath, target] of Object.entries(manifest.exports ?? {})) {
      if (typeof target !== "string" || !/^\.\/src\/.*\.[cm]?[jt]sx?$/.test(target)) continue
      const relative = target.replace(/^\.\/src\//, "").replace(/\.[cm]?[jt]sx?$/, "")
      const key = subpath === "." ? "index" : relative
      entries[key] = target.replace(/^\.\//, "")
    }
  }
  return entries
}
