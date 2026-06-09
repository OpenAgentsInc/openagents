import relayWorker, {
  NostrRelayDO,
} from "nostr-effect/relay/backends/cloudflare/worker"
import type { Env as NostrRelayEnv } from "nostr-effect/relay/backends/cloudflare/NostrRelayDO"

export { NostrRelayDO }

export interface Env extends NostrRelayEnv {
  OPENAGENTS_NOSTR_RELAY_ISSUE?: string
}

const json = (value: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(value, null, 2), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...init.headers,
    },
  })

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url)

    if (url.pathname === "/health") {
      return json({
        ok: true,
        relay: "openagents-nostr-relay-poc",
        backend: "cloudflare-durable-object",
        issue: env.OPENAGENTS_NOSTR_RELAY_ISSUE ?? null,
      })
    }

    return relayWorker.fetch(request, env)
  },
}
