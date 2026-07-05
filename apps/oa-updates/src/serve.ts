import type { AssetStore } from "./asset-store.ts"
import { readExportedUpdate } from "./export-reader.ts"
import type { PublishExportResult } from "./publish.ts"
import type { Platform } from "./publish-builder.ts"
import { seedDesktopReleases } from "./desktop-seed.ts"
import { seedPylonReleases } from "./pylon-seed.ts"
import {
  createUpdatesServer,
  type UpdatesServer,
} from "./server.ts"

type SeedFromDistInput = {
  readonly server: UpdatesServer
  readonly distDir: string
  readonly platform: Platform
  readonly branch: string
  readonly runtimeVersion: string
  readonly baseUrl: string
  readonly readFile?: (path: string) => Promise<Uint8Array>
  readonly expoClientConfig?: Record<string, unknown>
}

export async function seedFromDist(
  input: SeedFromDistInput,
): Promise<PublishExportResult> {
  const shimStore: AssetStore = {
    put: (bytes) => input.server.putAsset(bytes),
    get: async () => null,
  }
  const result = await readExportedUpdate({
    distDir: input.distDir,
    platform: input.platform,
    branch: input.branch,
    runtimeVersion: input.runtimeVersion,
    // The expo-updates client (FileDownloader.createUpdate) requires `id` to
    // parse as a UUID and crashes with an uncaught NSInternalInconsistencyException
    // ("update ID should be a valid UUID") otherwise — must be a real UUID, not
    // a human-readable seed tag.
    id: crypto.randomUUID(),
    createdAt: new Date().toISOString(),
    baseUrl: input.baseUrl,
    store: shimStore,
    readFile: input.readFile,
    expoClientConfig: input.expoClientConfig,
  })

  input.server.registerUpdate(result.update)

  return result
}

if (import.meta.main) {
  const port = Number(process.env.PORT ?? 8080)
  const server = createUpdatesServer({
    port,
    signingKeyPem: process.env.OA_SIGNING_KEY,
  })

  if (process.env.OA_SEED_DIST) {
    if (!process.env.OA_SEED_RUNTIME) {
      throw new Error("OA_SEED_RUNTIME is required when OA_SEED_DIST is set")
    }

    // Optional: a JSON file with the resolved public app config (the same
    // shape `expo config --type public --json` produces), embedded into the
    // manifest as `extra.expoClient`. Without this, expo-constants /
    // expo-linking throw on a downloaded (non-embedded) update and
    // expo-updates silently rolls the launch back to the cached update.
    let expoClientConfig: Record<string, unknown> | undefined
    if (process.env.OA_SEED_EXPO_CLIENT_PATH) {
      const { readFile } = await import("node:fs/promises")
      expoClientConfig = JSON.parse(
        await readFile(process.env.OA_SEED_EXPO_CLIENT_PATH, "utf8"),
      ) as Record<string, unknown>
    }

    await seedFromDist({
      server,
      distDir: process.env.OA_SEED_DIST,
      platform: (process.env.OA_SEED_PLATFORM ?? "ios") as Platform,
      branch: "production",
      runtimeVersion: process.env.OA_SEED_RUNTIME,
      baseUrl: process.env.OA_PUBLIC_URL ?? `http://localhost:${port}`,
      expoClientConfig,
    })
  }

  if (process.env.OA_DESKTOP_RELEASES_DIST) {
    await seedDesktopReleases({
      server,
      distDir: process.env.OA_DESKTOP_RELEASES_DIST,
      baseUrl: process.env.OA_PUBLIC_URL ?? `http://localhost:${port}`,
    })
  }

  if (process.env.OA_PYLON_RELEASES_DIST) {
    await seedPylonReleases({
      server,
      distDir: process.env.OA_PYLON_RELEASES_DIST,
      baseUrl: process.env.OA_PUBLIC_URL ?? `http://localhost:${port}`,
      // Binaries are served from GCS (Cloud Run caps responses at 32 MiB);
      // the feed JSON stays on this service, artifactUrls point at OA_ASSET_BASE_URL.
      ...(process.env.OA_ASSET_BASE_URL ? { assetBaseUrl: process.env.OA_ASSET_BASE_URL } : {}),
    })
  }

  // Electrobun desktop OTA artifacts: register each file in OA_DESKTOP_OTA_DIR so
  // the updater can fetch /desktop/<prefix>-update.json + the tarball/patches.
  if (process.env.OA_DESKTOP_OTA_DIR) {
    const { readdir } = await import("node:fs/promises")
    const dir = process.env.OA_DESKTOP_OTA_DIR
    for (const name of await readdir(dir)) {
      server.registerDesktopOtaFile(name, `${dir}/${name}`)
    }
  }

  Bun.serve({ port, fetch: server.fetch })
  console.log(`oa-updates listening on http://localhost:${port}`)
}
