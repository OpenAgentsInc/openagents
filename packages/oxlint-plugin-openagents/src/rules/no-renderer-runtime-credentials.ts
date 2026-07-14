import { defineRule } from "@oxlint/plugins"
import { importSource } from "../ast.ts"

const DesktopRenderer = /apps\/openagents-desktop\/src\/renderer\//u
const RestrictedImport = /(?:^|\/)(?:anthropic|openai|provider-sdk|token-store|credential-store|secret-store)(?:$|\/)|^@anthropic-ai\/|^openai$/iu

export default defineRule({
  meta: {
    type: "problem",
    docs: {
      description: "Enforce Desktop renderer isolation from provider SDKs, token stores, and runtime credentials.",
    },
  },
  create(context) {
    if (!DesktopRenderer.test(context.filename.replaceAll("\\", "/"))) return {}
    return {
      ImportDeclaration(node) {
        if (node.importKind === "type") return
        const source = importSource(node)
        if (source === undefined || !RestrictedImport.test(source)) return
        context.report({
          node,
          message: `Desktop renderer code cannot import ${source}; provider SDK and credential authority stays behind the typed main/preload boundary (root INVARIANTS.md: Effect Workspace Boundary).`,
        })
      },
    }
  },
})
