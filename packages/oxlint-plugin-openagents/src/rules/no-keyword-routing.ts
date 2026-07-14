import { defineRule } from "@oxlint/plugins"
import { propertyName, unwrap } from "../ast.ts"

const RoutingFile = /(?:intent|retriev|router|routing|selector|tool-selection)/iu
const KeywordMethod = new Set(["endsWith", "includes", "match", "search", "startsWith", "test"])

export default defineRule({
  meta: {
    type: "problem",
    docs: {
      description: "Enforce AGENTS.md Semantic Routing And Retrieval: user-facing routing must use a typed semantic selector, structured planner, modeled parser, or embedding search.",
    },
  },
  create(context) {
    if (!RoutingFile.test(context.filename)) return {}
    return {
      CallExpression(node) {
        const callee = unwrap(node.callee)
        if (callee?.type !== "MemberExpression") return
        const method = propertyName(callee.property)
        if (method === undefined || !KeywordMethod.has(method)) return
        if (!node.arguments.some(argument => unwrap(argument)?.type === "Literal")) return
        context.report({
          node: node.callee,
          message: "Do not route user intent, retrieval, or tool selection with keyword/string matching. Use the central typed semantic selector or structured planner (workspace AGENTS.md: Semantic Routing And Retrieval).",
        })
      },
    }
  },
})
