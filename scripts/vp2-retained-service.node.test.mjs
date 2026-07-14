import assert from "node:assert/strict"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { pathToFileURL } from "node:url"
import { test } from "node:test"

import { Runtime } from "../packages/runtime-platform/src/index.ts"

test("compiled retained Live Hub serves health under stock Node", async () => {
  const directory = await mkdtemp(join(tmpdir(), "openagents-vp2-live-hub-"))
  try {
    const built = await Runtime.build({
      entrypoints: [new URL("../apps/khala-live-hub/src/server.ts", import.meta.url).pathname],
      outdir: directory,
      target: "node",
      format: "esm",
    })
    assert.equal(built.success, true, built.logs.join("\n"))
    const artifact = join(directory, "server.js")
    const { startLiveHubServer } = await import(pathToFileURL(artifact).href)
    const running = startLiveHubServer({
      token: "vp2-stock-node-smoke",
      port: 0,
      pingIntervalMs: 60_000,
      log: () => {},
    })
    await running.server.ready
    try {
      const response = await fetch(new URL("/health", running.server.url))
      assert.equal(response.status, 200)
      assert.deepEqual(await response.json(), { ok: true, scopes: 0, sockets: 0 })
    } finally {
      await running.stop()
    }
  } finally {
    await rm(directory, { recursive: true, force: true })
  }
})
