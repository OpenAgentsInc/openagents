import {
  publicNostrChatManifest,
} from "@openagentsinc/public-nostr-chat"

const noStore = {
  "cache-control": "no-store",
  "content-type": "application/json; charset=utf-8",
} as const

const json = (body: unknown, status = 200, headers?: Headers): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: headers ?? noStore,
  })

export const handlePublicNostrChatManifest = (
  request: Request,
  relaySelfPubkey?: string,
): Response => {
  if (request.method !== "GET") {
    return json({ error: "method-not-allowed" }, 405)
  }
  return new Response(JSON.stringify(publicNostrChatManifest(relaySelfPubkey)), {
    headers: {
      "access-control-allow-origin": "*",
      "cache-control": "public, max-age=60",
      "content-type": "application/json; charset=utf-8",
    },
  })
}
