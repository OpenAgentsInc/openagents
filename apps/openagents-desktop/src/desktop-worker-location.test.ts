import { describe, expect, test } from "vite-plus/test"
import { desktopWorkerUrl } from "./desktop-worker-location.ts"

describe("desktopWorkerUrl", () => {
  test("maps packaged workers to their exact unpacked signed file", () => {
    expect(desktopWorkerUrl(
      "file:///Applications/OpenAgents.app/Contents/Resources/app.asar/dist/main.js",
      "codex-history-worker.js",
    ).href).toBe(
      "file:///Applications/OpenAgents.app/Contents/Resources/app.asar.unpacked/dist/workers/codex-history-worker.js",
    )
  })

  test("keeps development and non-file bases unchanged", () => {
    expect(desktopWorkerUrl(
      "file:///repo/apps/openagents-desktop/dist/main.js",
      "workspace-search-worker.js",
    ).href).toBe("file:///repo/apps/openagents-desktop/dist/workers/workspace-search-worker.js")
    expect(desktopWorkerUrl(
      "https://example.test/dist/main.js",
      "workspace-search-worker.js",
    ).href).toBe("https://example.test/dist/workers/workspace-search-worker.js")
  })
})
