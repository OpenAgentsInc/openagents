import {
  createInMemoryAssetStore,
  type AssetStore,
} from "./asset-store.ts"
import {
  parseManifestRequest,
  resolveManifest,
  type Update,
} from "./manifest-resolver.ts"
import { buildSignedManifestResponse } from "./signed-response.ts"
import { createNodeRegistry, type NodeRegistration } from "./node-registry.ts"

type CreateUpdatesServerOptions = {
  port?: number
  signingKeyPem?: string
  keyid?: string
}

export type UpdatesServer = {
  fetch: (request: Request) => Promise<Response>
  registerUpdate: (update: Update) => void
  putAsset: (bytes: Uint8Array) => Promise<{ hash: string; url: string }>
}

const defaultPort = 3000

const headersFromRequest = (request: Request): Record<string, string> =>
  Object.fromEntries(request.headers.entries())

const jsonResponse = (
  body: unknown,
  headers: Record<string, string> = {},
): Response =>
  Response.json(body, {
    headers: {
      ...headers,
      "content-type": "application/json",
    },
  })

const assetContentType = (
  updates: Iterable<Update>,
  hash: string,
): string => {
  for (const update of updates) {
    if (update.launchAsset.hash === hash) {
      return update.launchAsset.contentType
    }

    const asset = update.assets.find((candidate) => candidate.hash === hash)

    if (asset !== undefined) {
      return asset.contentType
    }
  }

  return "application/octet-stream"
}

export function createUpdatesServer(
  options: CreateUpdatesServerOptions = {},
): UpdatesServer {
  const port = options.port ?? defaultPort
  const updates = new Map<string, Update>()
  const channelToBranch = new Map<string, string>()
  const assetStore: AssetStore = createInMemoryAssetStore(
    `http://localhost:${port}`,
  )
  const nodeRegistry = createNodeRegistry()

  return {
    async fetch(request) {
      const url = new URL(request.url)

      if (request.method === "GET") {
        const manifestMatch = url.pathname.match(/^\/([^/]+)\/manifest$/)

        if (manifestMatch !== null) {
          const requestHeaders = headersFromRequest(request)
          const parsedRequest = parseManifestRequest(requestHeaders)
          const result = resolveManifest({
            updates: [...updates.values()],
            channelToBranch: Object.fromEntries(channelToBranch.entries()),
            request: parsedRequest,
          })

          if (result.kind === "manifest") {
            if (options.signingKeyPem !== undefined) {
              const signedResponse = buildSignedManifestResponse({
                manifest: result.manifest,
                privateKeyPem: options.signingKeyPem,
                keyid: options.keyid,
              })

              return new Response(signedResponse.body, {
                headers: {
                  ...result.responseHeaders,
                  ...signedResponse.headers,
                },
              })
            }

            return jsonResponse(result.manifest, result.responseHeaders)
          }

          return jsonResponse(result.directive, result.responseHeaders)
        }

        const assetMatch = url.pathname.match(/^\/assets\/([^/]+)$/)

        if (assetMatch !== null) {
          const hash = assetMatch[1]
          const bytes = await assetStore.get(hash)

          if (bytes === null) {
            return new Response("Not found", { status: 404 })
          }

          return new Response(bytes, {
            headers: {
              "cache-control": "public, max-age=31536000, immutable",
              "content-type": assetContentType(updates.values(), hash),
            },
          })
        }

        // Discovery: list this owner's registered nodes (the app auto-connects
        // to the tailnet-first reachable one — no QR/paste).
        const nodesGet = url.pathname.match(/^\/([^/]+)\/nodes$/)
        if (nodesGet !== null) {
          return jsonResponse({ nodes: nodeRegistry.listForOwner(nodesGet[1]) })
        }
      }

      if (request.method === "POST") {
        // Discovery: a node self-registers its reachable address(es) + token.
        const nodesPost = url.pathname.match(/^\/([^/]+)\/nodes$/)
        if (nodesPost !== null) {
          const reg = (await request.json()) as NodeRegistration
          nodeRegistry.register(nodesPost[1], reg)
          return jsonResponse({ ok: true })
        }
      }

      return new Response("Not found", { status: 404 })
    },

    registerUpdate(update) {
      updates.set(update.id, update)
      channelToBranch.set(update.branch, update.branch)
    },

    putAsset(bytes) {
      return assetStore.put(bytes)
    },
  }
}

if (import.meta.main) {
  const port = Number(Bun.env.PORT ?? defaultPort)
  const server = createUpdatesServer({ port })

  Bun.serve({
    port,
    fetch: server.fetch,
  })
}
