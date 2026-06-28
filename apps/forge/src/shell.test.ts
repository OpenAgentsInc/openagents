import { describe, expect, test } from "bun:test"

import forgeWorker from "./index"
import {
  normalizeForgeRoute,
  renderForgeShell,
} from "./shell"

const cfRequest = (
  input: string,
  init?: RequestInit,
): Request<unknown, IncomingRequestCfProperties<unknown>> =>
  new Request(input, init) as Request<
    unknown,
    IncomingRequestCfProperties<unknown>
  >

describe("Forge shell", () => {
  test("normalizes stable shell routes", () => {
    expect(normalizeForgeRoute("/")).toBe("queue")
    expect(normalizeForgeRoute("/queue")).toBe("queue")
    expect(normalizeForgeRoute("/changes")).toBe("changes")
    expect(normalizeForgeRoute("/verification")).toBe("verification")
    expect(normalizeForgeRoute("/merge")).toBe("merge")
    expect(normalizeForgeRoute("/refs")).toBe("refs")
    expect(normalizeForgeRoute("/unknown")).toBe("queue")
  })

  test("renders contract-shaped panes and route state", () => {
    const panels = [
      ["queue", "work-queue"],
      ["changes", "change-inspector"],
      ["verification", "verification-state"],
      ["merge", "merge-queue"],
      ["refs", "git-ref-views"],
    ] as const

    for (const [route, panel] of panels) {
      const html = renderForgeShell({
        route,
        generatedAt: "2026-06-28T00:00:00.000Z",
      })

      expect(html).toContain('data-forge-app-shell="true"')
      expect(html).toContain(`data-forge-active-route="${route}"`)
      expect(html).toContain(`data-forge-panel="${panel}"`)
      expect(html).toContain("forge.openagents.com")
      expect(html).toContain("contract-placeholder")
    }
  })

  test("serves health, version, and shell routes from the Worker", async () => {
    const env = { FORGE_ENV: "test" }
    const health = await forgeWorker.fetch(
      cfRequest("https://forge.openagents.com/health"),
      env,
    )
    expect(health.status).toBe(200)
    await expect(health.json()).resolves.toMatchObject({
      ok: true,
      service: "openagents-forge",
      canonicalHost: "forge.openagents.com",
    })

    const version = await forgeWorker.fetch(
      cfRequest("https://forge.openagents.com/version"),
      env,
    )
    expect(version.status).toBe(200)
    await expect(version.json()).resolves.toMatchObject({
      appBoundary: "apps/forge",
      apiDependency: "forge.public_safe_contract.pending",
    })

    const shell = await forgeWorker.fetch(
      cfRequest("https://forge.openagents.com/refs"),
      env,
    )
    expect(shell.status).toBe(200)
    expect(shell.headers.get("content-type")).toContain("text/html")
    await expect(shell.text()).resolves.toContain(
      'data-forge-active-route="refs"',
    )
  })

  test("rejects non-shell paths and unsupported methods", async () => {
    const env = { FORGE_ENV: "test" }
    const missing = await forgeWorker.fetch(
      cfRequest("https://forge.openagents.com/private/path"),
      env,
    )
    expect(missing.status).toBe(404)

    const posted = await forgeWorker.fetch(
      cfRequest("https://forge.openagents.com/", { method: "POST" }),
      env,
    )
    expect(posted.status).toBe(405)
    expect(posted.headers.get("allow")).toBe("GET, HEAD")
  })
})
