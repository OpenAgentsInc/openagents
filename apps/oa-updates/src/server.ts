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
import {
  sortDesktopFeed,
  type DesktopUpdateManifest,
} from "./desktop-release.ts"
import {
  buildPylonFeed,
  normalizePylonPlatform,
  type PylonPlatform,
  type PylonReleaseManifest,
} from "./pylon-release.ts"

type CreateUpdatesServerOptions = {
  port?: number
  signingKeyPem?: string
  keyid?: string
}

export type UpdatesServer = {
  fetch: (request: Request) => Promise<Response>
  registerUpdate: (update: Update) => void
  registerDesktopUpdate: (
    channel: string,
    manifest: DesktopUpdateManifest,
  ) => void
  registerPylonUpdate: (manifest: PylonReleaseManifest) => void
  // Serve a large asset (e.g. a Pylon binary) straight from disk by hash,
  // streamed — so the seed never loads hundreds of MB into memory at boot.
  registerDiskAsset: (hash: string, path: string, contentType?: string) => void
  // Serve an Electrobun desktop OTA artifact (<prefix>-update.json / .tar.zst /
  // .dmg / .patch) at /desktop/<filename>, streamed from disk.
  registerDesktopOtaFile: (filename: string, path: string, contentType?: string) => void
  putAsset: (
    bytes: Uint8Array,
    contentType?: string,
  ) => Promise<{ hash: string; url: string }>
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

// Expo Updates Protocol requires manifest/directive responses as multipart/mixed
// with a part named "manifest" or "directive". A bare application/json body is
// parsed by expo-updates as a manifest and crashes on the missing required `id`
// (Manifest.swift requiredValue) — this is what crashed build 13 on launch.
const OTA_BOUNDARY = "oa-updates-boundary"

const multipartMixedResponse = (
  parts: { name: string; body: string; partHeaders?: Record<string, string> }[],
  responseHeaders: Record<string, string> = {},
): Response => {
  const crlf = "\r\n"
  let body = ""
  for (const part of parts) {
    body += `--${OTA_BOUNDARY}${crlf}`
    body += `content-disposition: form-data; name="${part.name}"${crlf}`
    body += `content-type: application/json${crlf}`
    for (const [k, v] of Object.entries(part.partHeaders ?? {})) {
      body += `${k}: ${v}${crlf}`
    }
    body += crlf
    body += part.body + crlf
  }
  body += `--${OTA_BOUNDARY}--${crlf}`
  return new Response(body, {
    headers: {
      ...responseHeaders,
      "content-type": `multipart/mixed; boundary=${OTA_BOUNDARY}`,
    },
  })
}

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
  const desktopFeeds = new Map<string, DesktopUpdateManifest[]>()
  // key: `${channel}/${platform}` -> releases (latest first)
  const pylonFeeds = new Map<string, PylonReleaseManifest[]>()
  // hash -> on-disk file served by streaming (large binaries never held in memory)
  const diskAssets = new Map<string, { path: string; contentType: string }>()
  // filename -> on-disk Electrobun desktop OTA artifact, served at /desktop/<filename>
  const desktopOtaFiles = new Map<string, { path: string; contentType: string }>()
  const assetContentTypes = new Map<string, string>()
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

              // Signature travels as a part header on the manifest part.
              return multipartMixedResponse(
                [
                  {
                    name: "manifest",
                    body: signedResponse.body,
                    partHeaders: { "expo-signature": signedResponse.headers["expo-signature"] },
                  },
                ],
                result.responseHeaders,
              )
            }

            return multipartMixedResponse(
              [{ name: "manifest", body: JSON.stringify(result.manifest) }],
              result.responseHeaders,
            )
          }

          return multipartMixedResponse(
            [{ name: "directive", body: JSON.stringify(result.directive) }],
            result.responseHeaders,
          )
        }

        const assetMatch = url.pathname.match(/^\/assets\/([^/]+)$/)

        if (assetMatch !== null) {
          const hash = assetMatch[1]

          // Disk-backed assets (Pylon binaries) stream straight from the file —
          // bounded memory regardless of size.
          const disk = diskAssets.get(hash)
          if (disk !== undefined) {
            return new Response(Bun.file(disk.path), {
              headers: {
                "cache-control": "public, max-age=31536000, immutable",
                "content-type": disk.contentType,
              },
            })
          }

          const bytes = await assetStore.get(hash)

          if (bytes === null) {
            return new Response("Not found", { status: 404 })
          }

          return new Response(bytes, {
            headers: {
              "cache-control": "public, max-age=31536000, immutable",
              "content-type":
                assetContentTypes.get(hash) ??
                assetContentType(updates.values(), hash),
            },
          })
        }

        const desktopFeedMatch = url.pathname.match(
          /^\/desktop\/([^/]+)\/feed\.json$/,
        )

        if (desktopFeedMatch !== null) {
          const channel = desktopFeedMatch[1]
          return jsonResponse(sortDesktopFeed(desktopFeeds.get(channel) ?? []), {
            "cache-control": "no-store",
          })
        }

        // Electrobun desktop OTA artifact: /desktop/<filename> (the updater fetches
        // <prefix>-update.json and <prefix>-…tar.zst / .patch from release.baseUrl).
        const desktopOtaMatch = url.pathname.match(/^\/desktop\/([^/]+)$/)
        if (desktopOtaMatch !== null) {
          const file = desktopOtaFiles.get(desktopOtaMatch[1])
          if (file !== undefined) {
            // update.json must not be cached (it's the freshness signal); the
            // immutable artifacts can cache hard.
            const noCache = desktopOtaMatch[1].endsWith("update.json")
            return new Response(Bun.file(file.path), {
              headers: {
                "cache-control": noCache
                  ? "no-store"
                  : "public, max-age=31536000, immutable",
                "content-type": file.contentType,
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

        // Discovery: list this owner's registered nodes (the app auto-connects
        // to the tailnet-first reachable one — no QR/paste).
        const nodesGet = url.pathname.match(/^\/([^/]+)\/nodes$/)
        if (nodesGet !== null) {
          // Prune before listing so a stale/dead node (no heartbeat within
          // ~6× the 20s interval) never gets handed to the phone, which picks
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

    registerUpdate(update) {
      updates.set(update.id, update)
      channelToBranch.set(update.branch, update.branch)
    },

    registerDesktopUpdate(channel, manifest) {
      const normalizedChannel = channel.trim()
      const current = desktopFeeds.get(normalizedChannel) ?? []
      desktopFeeds.set(normalizedChannel, [
        manifest,
        ...current.filter((candidate) => candidate.version !== manifest.version),
      ])
    },

    registerDiskAsset(hash, path, contentType) {
      diskAssets.set(hash, { path, contentType: contentType ?? "application/octet-stream" })
    },

    registerDesktopOtaFile(filename, path, contentType) {
      desktopOtaFiles.set(filename, {
        path,
        contentType:
          contentType ??
          (filename.endsWith(".json")
            ? "application/json"
            : "application/octet-stream"),
      })
    },

    registerPylonUpdate(manifest) {
      const key = `${manifest.channel}/${manifest.platform}`
      const current = pylonFeeds.get(key) ?? []
      pylonFeeds.set(key, [
        manifest,
        ...current.filter((candidate) => candidate.version !== manifest.version),
      ])
    },

    async putAsset(bytes, contentType) {
      const stored = await assetStore.put(bytes)
      if (contentType !== undefined) {
        assetContentTypes.set(stored.hash, contentType)
      }

      return stored
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
