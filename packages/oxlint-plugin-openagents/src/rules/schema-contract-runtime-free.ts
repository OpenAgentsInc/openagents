import { defineRule } from "@oxlint/plugins"
import { importSource } from "../ast.ts"

const SchemaPackageSource = /packages\/(?:[^/]*(?:contract|schema)[^/]*)\/src\//u
const RuntimeImport = /^(?:node:|bun:|@effect\/platform|@openagentsinc\/runtime-platform|@anthropic-ai\/|@openai\/|openai$)/u

export default defineRule({
  meta: {
    type: "problem",
    docs: {
      description: "Enforce root INVARIANTS.md Effect Workspace Boundary: schema/contract packages are declarative and runtime-side-effect free.",
    },
  },
  create(context) {
    if (!SchemaPackageSource.test(context.filename.replaceAll("\\", "/"))) return {}
    return {
      ImportDeclaration(node) {
        if (node.importKind === "type") return
        const source = importSource(node)
        if (source === undefined || !RuntimeImport.test(source)) return
        context.report({
          node,
          message: `Schema-only contract packages cannot import runtime module ${source}. Move host composition to an app/runtime package (root INVARIANTS.md: Effect Workspace Boundary).`,
        })
      },
    }
  },
})
