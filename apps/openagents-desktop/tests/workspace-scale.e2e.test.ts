import { expect, test } from "bun:test"
import { execFileSync } from "node:child_process"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { performance } from "node:perf_hooks"
import { pathToFileURL } from "node:url"

import { makeWorkspaceSearchHost } from "../src/workspace-search-host.ts"
import { openWorkspaceService } from "../src/workspace-service.ts"

const appRoot = path.resolve(import.meta.dir, "..")

test("CUT-17 real worker bounds a 20k-entry repository and project close drains watcher/search ownership", async () => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-workspace-scale-"))
  const workerOut = mkdtempSync(path.join(tmpdir(), "openagents-workspace-worker-"))
  try {
    execFileSync("git", ["init", "--quiet"], { cwd: root })
    for (let directory = 0; directory < 100; directory++) {
      const directoryPath = path.join(root, `src-${String(directory).padStart(3, "0")}`)
      mkdirSync(directoryPath)
      for (let file = 0; file < 200; file++) {
        writeFileSync(
          path.join(directoryPath, `file-${String(file).padStart(3, "0")}.txt`),
          `bounded fixture ${directory}:${file}\n`,
        )
      }
    }

    const build = await Bun.build({
      entrypoints: [path.join(appRoot, "src/workspace-search-worker.ts")],
      outdir: workerOut,
      target: "node",
      format: "esm",
    })
    expect(build.success).toBe(true)

    const searchHost = makeWorkspaceSearchHost(
      root,
      "workspace.grant.scale",
      pathToFileURL(path.join(workerOut, "workspace-search-worker.js")) as unknown as URL,
    )
    let watcherCloses = 0
    const workspace = openWorkspaceService(root, {
      grantRef: "workspace.grant.scale",
      searchHostFactory: () => searchHost,
      watchFactory: () => {
        let closed = false
        return {
          close: () => {
            if (closed) return
            closed = true
            watcherCloses += 1
          },
        }
      },
    })
    const subscription = workspace.subscribe(() => undefined)
    const startedAt = performance.now()
    const first = await workspace.search({
      query: "absent-content-needle",
      mode: "content",
      limit: 100,
    }).result
    const elapsedMs = performance.now() - startedAt
    expect(first).toMatchObject({
      state: "available",
      grantRef: "workspace.grant.scale",
      matches: [],
      nextOffset: null,
      truncated: true,
      cache: { epoch: 0, freshness: "current" },
    })
    expect(JSON.stringify(first)).not.toContain(root)
    expect(elapsedMs).toBeLessThan(20_000)

    const cachedStartedAt = performance.now()
    const cached = workspace.search({
      query: "absent-content-needle",
      mode: "content",
      limit: 100,
    })
    expect(cached.taskRef).toStartWith("workspace.search.cache.")
    expect(await cached.result).toEqual(first)
    const cachedElapsedMs = performance.now() - cachedStartedAt
    expect(cachedElapsedMs).toBeLessThan(50)
    console.log("[CUT-17 workspace scale]", JSON.stringify({
      entries: 20_000,
      searchMs: Math.round(elapsedMs),
      cacheMs: Math.round(cachedElapsedMs * 100) / 100,
      truncated: first.state === "available" && first.truncated,
    }))

    const pending = workspace.search({ query: "fixture", mode: "content", limit: 100 })
    workspace.dispose()
    workspace.dispose()
    subscription.close()
    expect((await pending.result).state).toBe("unavailable")
    expect(watcherCloses).toBe(1)
    expect(searchHost.activeCount()).toBe(0)
  } finally {
    rmSync(root, { recursive: true, force: true })
    rmSync(workerOut, { recursive: true, force: true })
  }
}, 30_000)
