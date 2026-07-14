import { defineRule } from "@oxlint/plugins"
import { importSource } from "../ast.ts"

const SubpathOnlyPackages = new Set([
  "@openagentsinc/oa-infra",
  "@openagentsinc/pylon-core",
])

export default defineRule({
  meta: {
    type: "problem",
    docs: {
      description: "Enforce explicit package ownership boundaries by rejecting root imports for subpath-owned packages.",
    },
  },
  create(context) {
    return {
      ImportDeclaration(node) {
        const source = importSource(node)
        if (source === undefined || !SubpathOnlyPackages.has(source)) return
        context.report({
          node,
          message: `Import from an explicit ${source}/* subpath so the owning runtime boundary is visible (root AGENTS.md: Repo Layout).`,
        })
      },
    }
  },
})
