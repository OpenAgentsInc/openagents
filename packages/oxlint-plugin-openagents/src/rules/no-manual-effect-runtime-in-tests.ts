import { defineRule } from "@oxlint/plugins"
import { identifierIs, propertyName, unwrap } from "../ast.ts"

const TestFile = /\.(?:test|spec)\.[cm]?[jt]sx?$/u
const RuntimeMethods = new Set([
  "runCallback", "runFork", "runPromise", "runPromiseExit", "runSync", "runSyncExit",
])

export default defineRule({
  meta: {
    type: "problem",
    docs: {
      description: "Enforce root INVARIANTS.md Effect Workspace Boundary: tests use the shared Effect-aware harness instead of manually owning runtimes.",
    },
  },
  create(context) {
    if (!TestFile.test(context.filename)) return {}
    return {
      CallExpression(node) {
        const callee = unwrap(node.callee)
        if (callee?.type !== "MemberExpression") return
        const method = propertyName(callee.property)
        if (method === undefined) return
        const manualEffectRun = identifierIs(callee.object, "Effect") && RuntimeMethods.has(method)
        const manualRuntime = identifierIs(callee.object, "ManagedRuntime") && method === "make"
        if (!manualEffectRun && !manualRuntime) return
        context.report({
          node: node.callee,
          message: "Tests must use the shared Effect-aware Vite Plus harness (it.effect/test layers), not a manually created or executed Effect runtime (root INVARIANTS.md: Effect Workspace Boundary).",
        })
      },
    }
  },
})
