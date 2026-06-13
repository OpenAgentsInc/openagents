// End-to-end local proof of the OpenAgents Updates server: stand up the server,
// publish an update (content-addressed assets), then over REAL HTTP fetch the
// Expo-Updates manifest and an asset — proving the protocol works on our own
// infra with no Expo cloud. Run: `bun run apps/oa-updates/scripts/e2e-local.ts`
import { buildUpdateFromExport } from "../src/publish-builder.ts"
import { createUpdatesServer } from "../src/server.ts"

const PORT = 4799
const RUNTIME = "d36a2a5bb2320ce37a89c965823c91887d7d6bdb" // build #4's fingerprint
const enc = new TextEncoder()

function assert(cond: boolean, msg: string): void {
  if (!cond) {
    console.error(`FAIL: ${msg}`)
    process.exit(1)
  }
  console.log(`ok: ${msg}`)
}

const server = createUpdatesServer({ port: PORT })

// Publish: a JS launch bundle + one asset, content-addressed into the server store.
const bundleBytes = enc.encode("var __OA_UPDATE__ = 'hello from OTA';")
const assetBytes = enc.encode("PNGDATA")
const launch = await server.putAsset(bundleBytes)
const asset = await server.putAsset(assetBytes)

const update = buildUpdateFromExport({
  id: "update-e2e-0001",
  platform: "ios",
  branch: "production",
  runtimeVersion: RUNTIME,
  createdAt: "2026-06-13T13:00:00.000Z",
  baseUrl: `http://localhost:${PORT}`,
  launchBundle: { key: "bundle", hash: launch.hash },
  assets: [{ key: "icon", hash: asset.hash, contentType: "image/png", fileExtension: ".png" }],
})
server.registerUpdate(update)

const http = Bun.serve({ port: PORT, fetch: server.fetch })
try {
  // 1) manifest for matching runtime/channel
  const res = await fetch(`http://localhost:${PORT}/autopilot/manifest`, {
    headers: {
      "Expo-Platform": "ios",
      "Expo-Runtime-Version": RUNTIME,
      "Expo-Channel-Name": "production",
      "Expo-Protocol-Version": "1",
    },
  })
  assert(res.status === 200, "manifest responds 200")
  assert(res.headers.get("expo-protocol-version") === "1", "expo-protocol-version: 1 header present")
  const manifest = (await res.json()) as Record<string, any>
  assert(manifest.id === "update-e2e-0001", "manifest id matches published update")
  assert(manifest.runtimeVersion === RUNTIME, "manifest runtimeVersion matches build fingerprint")
  assert(manifest.launchAsset?.url === `http://localhost:${PORT}/assets/${launch.hash}`, "launchAsset url is content-addressed to our server")
  assert(manifest.assets?.[0]?.url === `http://localhost:${PORT}/assets/${asset.hash}`, "asset url is content-addressed")

  // 2) the launch bundle is fetchable and byte-identical
  const bundleRes = await fetch(manifest.launchAsset.url)
  assert(bundleRes.status === 200, "launch bundle fetch 200")
  const got = new Uint8Array(await bundleRes.arrayBuffer())
  assert(got.length === bundleBytes.length && got.every((b, i) => b === bundleBytes[i]), "served bundle bytes are byte-identical to published")

  // 3) a mismatched runtimeVersion gets noUpdateAvailable (the fingerprint gate)
  const noRes = await fetch(`http://localhost:${PORT}/autopilot/manifest`, {
    headers: { "Expo-Platform": "ios", "Expo-Runtime-Version": "different-fingerprint", "Expo-Channel-Name": "production" },
  })
  const directive = (await noRes.json()) as Record<string, any>
  assert(directive.type === "noUpdateAvailable", "mismatched runtimeVersion → noUpdateAvailable")

  console.log("\nE2E PASS — OpenAgents Updates served a valid Expo-Updates manifest + asset over HTTP on our own infra.")
} finally {
  http.stop(true)
}
