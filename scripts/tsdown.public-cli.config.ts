import { defineConfig } from "tsdown"

const rawEntries = process.env.OPENAGENTS_PUBLIC_CLI_ENTRIES
if (rawEntries === undefined) throw new Error("OPENAGENTS_PUBLIC_CLI_ENTRIES is required")
const externalInternalDts = process.env.OPENAGENTS_PUBLIC_CLI_EXTERNAL_INTERNAL_DTS === "1"

export default defineConfig({
  cwd: process.env.OPENAGENTS_PUBLIC_CLI_ROOT,
  root: process.env.OPENAGENTS_PUBLIC_CLI_ROOT,
  entry: JSON.parse(rawEntries) as Record<string, string>,
  outDir: "dist",
  clean: true,
  dts: process.env.OPENAGENTS_PUBLIC_CLI_EAGER_DTS === "1" ? { eager: true } : true,
  format: "esm",
  platform: "node",
  target: "node24",
  sourcemap: true,
  deps: {
    alwaysBundle: [/^@openagentsinc\//],
    onlyBundle: false,
    dts: externalInternalDts
      ? { neverBundle: [/^@openagentsinc\//] }
      : { alwaysBundle: [/^@openagentsinc\//] },
  },
})
