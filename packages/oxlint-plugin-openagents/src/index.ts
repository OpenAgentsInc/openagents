import { definePlugin } from "@oxlint/plugins"

import noInlineSchemaCompile from "./rules/no-inline-schema-compile.ts"
import noKeywordRouting from "./rules/no-keyword-routing.ts"
import noManualEffectRuntimeInTests from "./rules/no-manual-effect-runtime-in-tests.ts"
import noRendererRuntimeCredentials from "./rules/no-renderer-runtime-credentials.ts"
import schemaContractRuntimeFree from "./rules/schema-contract-runtime-free.ts"
import subpathOnlyImports from "./rules/subpath-only-imports.ts"

export default definePlugin({
  meta: { name: "openagents" },
  rules: {
    "no-inline-schema-compile": noInlineSchemaCompile,
    "no-keyword-routing": noKeywordRouting,
    "no-manual-effect-runtime-in-tests": noManualEffectRuntimeInTests,
    "no-renderer-runtime-credentials": noRendererRuntimeCredentials,
    "schema-contract-runtime-free": schemaContractRuntimeFree,
    "subpath-only-imports": subpathOnlyImports,
  },
})
