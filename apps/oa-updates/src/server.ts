import { Runtime } from "@openagentsinc/runtime-platform"
import { createNodeRegistry, type NodeRegistration } from "./node-registry.ts"
import {
  buildPylonFeed,
  normalizePylonPlatform,
  type PylonPlatform,
  type PylonReleaseManifest,
} from "./pylon-release.ts"

export type UpdatesServer = {
  fetch: (request: Request) => Promise<Response>
  registerPylonUpdate: (manifest: PylonReleaseManifest) => void
  // Serve a large asset (e.g. a Pylon binary) straight from disk by hash,
  // streamed — so the seed never loads hundreds of MB into memory at boot.
  registerDiskAsset: (hash: string, path: string, contentType?: string) => void
}

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

// This service serves the signed Pylon release feed and the Pylon node
// discovery registry. The Expo mobile OTA surface it used to also carry —
// `/<owner>/manifest`, the multipart/mixed Expo Updates Protocol responses,
// manifest code signing, and the in-memory published-asset store — was retired
// with the mobile update path on 2026-08-05 (#9325). Those routes are ordinary
// 404s now, exactly like the Electron desktop routes deleted before them.
export function createUpdatesServer(): UpdatesServer {
  // key: `${channel}/${platform}` -> releases (latest first)
  const pylonFeeds = new Map<string, PylonReleaseManifest[]>()
  // hash -> on-disk file served by streaming (large binaries never held in memory)
  const diskAssets = new Map<string, { path: string; contentType: string }>()
  const nodeRegistry = createNodeRegistry()

  return {
    async fetch(request) {
      const url = new URL(request.url)

      if (request.method === "GET") {
        const assetMatch = url.pathname.match(/^\/assets\/([^/]+)$/)

        if (assetMatch !== null) {
          const hash = assetMatch[1]

          // Disk-backed assets (Pylon binaries) stream straight from the file —
          // bounded memory regardless of size. Nothing else is served here now
          // that mobile OTA publication is retired.
          const disk = diskAssets.get(hash)
          if (disk !== undefined) {
            return new Response(Runtime.file(disk.path).stream(), {
              headers: {
                "cache-control": "public, max-age=31536000, immutable",
                "content-type": disk.contentType,
              },
            })
          }

          return new Response("Not found", { status: 404 })
        }

        // Pylon OTA feed: /pylon/<channel>/<platform>/feed.json — per-platform,
        // signed releases (yanked dropped, latest first). The self-updater
        // verifies each release's signature against the pinned key + sha256.
        const pylonFeedMatch = url.pathname.match(
          /^\/pylon\/([^/]+)\/([^/]+)\/feed\.json$/,
        )

        if (pylonFeedMatch !== null) {
          const channel = pylonFeedMatch[1]
          let platform: PylonPlatform
          try {
            platform = normalizePylonPlatform(pylonFeedMatch[2])
          } catch {
            return new Response("Unknown platform", { status: 404 })
          }
          const releases = pylonFeeds.get(`${channel}/${platform}`) ?? []
          return jsonResponse(buildPylonFeed(channel, platform, releases), {
            "cache-control": "no-store",
          })
        }

        // Discovery: list this owner's registered Pylon nodes.
        const nodesGet = url.pathname.match(/^\/([^/]+)\/nodes$/)
        if (nodesGet !== null) {
          // Prune before listing so a stale/dead node (no heartbeat within
          // ~6× the 20s interval) never gets handed to a client, which picks
          // the first reachable node. Keeps the in-memory list self-cleaning.
          nodeRegistry.pruneStale(Date.now(), 120_000)
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

    registerDiskAsset(hash, path, contentType) {
      diskAssets.set(hash, { path, contentType: contentType ?? "application/octet-stream" })
    },

    registerPylonUpdate(manifest) {
      const key = `${manifest.channel}/${manifest.platform}`
      const current = pylonFeeds.get(key) ?? []
      pylonFeeds.set(key, [
        manifest,
        ...current.filter((candidate) => candidate.version !== manifest.version),
      ])
    },
  }
}
