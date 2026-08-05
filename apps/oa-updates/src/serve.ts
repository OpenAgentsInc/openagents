import { Runtime } from "@openagentsinc/runtime-platform"
import { seedPylonReleases } from "./pylon-seed.ts"
import { createUpdatesServer } from "./server.ts"

// Cloud Run entrypoint for updates.openagents.com. It seeds exactly one
// surface: the signed per-platform Pylon release feed. The Expo mobile OTA
// seed (`OA_SEED_DIST` / `OA_SEED_RUNTIME` / `OA_SEED_PLATFORM` /
// `OA_SEED_BRANCH` / `OA_SEED_EXPO_CLIENT_PATH`) and the manifest code-signing
// key (`OA_SIGNING_KEY`) were retired with the mobile update path on
// 2026-08-05 (#9325) and are deliberately never read again, so a stale
// operator environment cannot resurrect a feed this service no longer serves.
if (Runtime.isMain(import.meta.url)) {
  const port = Number(process.env.PORT ?? 8080)
  const server = createUpdatesServer()

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

  Runtime.serve({ port, fetch: server.fetch })
  console.log(`oa-updates listening on http://localhost:${port}`)
}
