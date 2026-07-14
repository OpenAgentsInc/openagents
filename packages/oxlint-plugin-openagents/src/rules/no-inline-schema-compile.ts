import { defineRule } from "@oxlint/plugins"
import { identifierIs, propertyName, unwrap } from "../ast.ts"

const CompilerMethod = /^(?:asserts|decode|encode|is)(?:Unknown)?(?:Effect|Exit|Option|Promise|Result|Sync)?$/u

export default defineRule({
  meta: {
    type: "problem",
    docs: {
      description: "Prevent Effect Schema compiler allocation inside hot function paths; compile once at module scope.",
    },
  },
  createOnce(context) {
    let functionDepth = 0
    const enter = () => { functionDepth += 1 }
    const exit = () => { functionDepth -= 1 }
    return {
      before() { functionDepth = 0 },
      FunctionDeclaration: enter,
      "FunctionDeclaration:exit": exit,
      FunctionExpression: enter,
      "FunctionExpression:exit": exit,
      ArrowFunctionExpression: enter,
      "ArrowFunctionExpression:exit": exit,
      CallExpression(node) {
        if (functionDepth === 0) return
        const callee = unwrap(node.callee)
        if (callee?.type !== "MemberExpression" || !identifierIs(callee.object, "Schema")) return
        const method = propertyName(callee.property)
        if (method === undefined || !CompilerMethod.test(method)) return
        context.report({
          node: node.callee,
          message: `Hoist Schema.${method}(...) to a module-level const so hot paths do not repeatedly compile the same schema (root INVARIANTS.md: Effect Workspace Boundary).`,
        })
      },
    }
  },
})
