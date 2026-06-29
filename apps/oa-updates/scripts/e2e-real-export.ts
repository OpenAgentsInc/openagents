// Real-bundle e2e: feed an actual `expo export` dist through readExportedUpdate
// into the server, then fetch the manifest + the real Hermes bundle over HTTP.
// Usage: bunx expo export --platform ios --output-dir /tmp/oa-export
//        bun run apps/oa-updates/scripts/e2e-real-export.ts /tmp/oa-export
import { createUpdatesServer } from "../src/server.ts"
import { readExportedUpdate } from "../src/export-reader.ts"
import type { AssetStore } from "../src/asset-store.ts"

const PORT = 4801
const RUNTIME = "d36a2a5bb2320ce37a89c965823c91887d7d6bdb"
const DIST = process.argv[2] ?? "/tmp/oa-export"

const server = createUpdatesServer({ port: PORT })
// Route published bytes into the server's own store so /assets/:hash can serve them.
const shimStore = {
  put: (bytes: Uint8Array) => server.putAsset(bytes),
  get: async () => null,
} as unknown as AssetStore

const { update } = await readExportedUpdate({
  distDir: DIST,
  platform: "ios",
  branch: "production",
  runtimeVersion: RUNTIME,
  id: "real-export-0001",
  createdAt: "2026-06-13T13:30:00.000Z",
  baseUrl: `http://localhost:${PORT}`,
  store: shimStore,
})
server.registerUpdate(update)

const http = Bun.serve({ port: PORT, fetch: server.fetch })
try {
  const res = await fetch(`http://localhost:${PORT}/autopilot/manifest`, {
    headers: { "Expo-Platform": "ios", "Expo-Runtime-Version": RUNTIME, "Expo-Channel-Name": "production" },
  })
  const m = (await res.json()) as Record<string, any>
  const b = await fetch(m.launchAsset.url)
  const bytes = new Uint8Array(await b.arrayBuffer())
  console.log(`manifest id=${m.id} runtime=${m.runtimeVersion}`)
  console.log(`launchAsset ${m.launchAsset.contentType} ${m.launchAsset.url} → ${bytes.length} bytes`)
  console.log(`assets: ${m.assets.length}`)
  const ok = bytes.length > 1000 && m.launchAsset.contentType === "application/javascript" && m.runtimeVersion === RUNTIME
  if (!ok) { console.error("REAL-EXPORT E2E FAIL"); process.exit(1) }
  console.log("\nREAL-EXPORT E2E PASS — our server served a real expo-exported Hermes bundle as an OTA manifest over HTTP.")
} finally {
  http.stop(true)
}
