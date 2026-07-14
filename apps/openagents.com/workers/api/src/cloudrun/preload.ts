/**
 * CFG-9 (#8524): Node preload mapping the Cloudflare-only built-in to the
 * structural Cloud Run stub. The production build emits the adjacent `.js`
 * module, so the runtime never needs a TypeScript loader or Bun plugin.
 */
import { registerHooks } from "node:module"

const stubUrl = new URL("./cloudflare-workers-stub.js", import.meta.url).href

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (specifier === "cloudflare:workers") {
      return { url: stubUrl, format: "module", shortCircuit: true }
    }
    return nextResolve(specifier, context)
  },
})
