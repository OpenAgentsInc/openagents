import { describe, expect, it } from "bun:test"
import { Effect } from "effect"

import {
  FORGE_UI_WORKER_VERSION,
  defaultForgeMount,
  forgeLandingCopy,
  forgeShellContract,
  forgeShellPreviewState,
  forgeShellRoutes,
  handleForgeRequest,
  renderForgeLandingHtml,
  renderForgeShellHtml,
  resolveForgeShellRoute,
} from "./index"

describe("Forge UI Worker", () => {
  it("renders the required copy inside the separate Forge shell", () => {
    const html = renderForgeLandingHtml()

    expect(html).toContain(forgeLandingCopy.title)
    expect(html).toContain(forgeLandingCopy.tagline)
    expect(html).toContain('data-forge-app="shell"')
    expect(html).toContain('data-forge-route="overview"')
    expect(html).toContain('data-shared-ui-package="@openagentsinc/ui"')
    expect(html).toContain("--forge-energy")
  })

  it("defines shell routes for work, changes, verification, queue, and refs", () => {
    expect(forgeShellRoutes.map(route => route.path)).toEqual([
      "/",
      "/work",
      "/changes",
      "/verification",
      "/queue",
      "/refs",
    ])
    expect(forgeShellRoutes.map(route => route.apiPath)).toEqual([
      "/api/forge/overview",
      "/api/forge/work-records",
      "/api/forge/changes",
      "/api/forge/verification-receipts",
      "/api/forge/queue",
      "/api/forge/refs",
    ])
    expect(resolveForgeShellRoute("/work/")?.id).toBe("work")
  })

  it("renders every shell route with a selected route marker", async () => {
    for (const route of forgeShellRoutes) {
      const response = await Effect.runPromise(
        handleForgeRequest(
          new Request(`https://forge.openagents.com${route.path}`),
        ),
      )
      const html = await response.text()

      expect(response.status).toBe(200)
      expect(response.headers.get("content-type")).toContain("text/html")
      expect(html).toContain(`data-forge-route="${route.id}"`)
      expect(html).toContain(route.label)
      expect(html).toContain(route.apiPath)
    }
  })

  it("serves the shell contract metadata without exposing control-plane routes", async () => {
    const response = await Effect.runPromise(
      handleForgeRequest(new Request("https://forge.openagents.com/shell.json")),
    )
    const body = (await response.json()) as ReturnType<typeof forgeShellContract>

    expect(response.status).toBe(200)
    expect(body.service).toBe("openagents-forge")
    expect(body.version).toBe(FORGE_UI_WORKER_VERSION)
    expect(body.mount).toEqual(defaultForgeMount)
    expect(body.routes).toEqual(forgeShellRoutes)
    expect(body.preview).toEqual(forgeShellPreviewState)
    expect(body.preview.dataMode).toBe("stubbed-public-contract")
    expect(body.preview.apiBasePath).toBe("/api/forge")
  })

  it("renders a direct route shell without the old logged-in Forge page", () => {
    const html = renderForgeShellHtml("changes")

    expect(html).toContain('data-forge-route="changes"')
    expect(html).toContain("Change Inspector")
    expect(html).not.toContain("loggedIn/page/forge")
  })

  it("reports health without claiming any coordination authority", async () => {
    const response = await Effect.runPromise(
      handleForgeRequest(new Request("https://forge.openagents.com/health")),
    )
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(body).toEqual({
      ok: true,
      service: "openagents-forge",
      version: FORGE_UI_WORKER_VERSION,
      mount: defaultForgeMount,
      shellRoutes: forgeShellRoutes.map(({ id, path, apiPath }) => ({
        id,
        path,
        apiPath,
      })),
    })
  })

  it("keeps unknown and future control-plane routes closed in the UI app", async () => {
    const response = await Effect.runPromise(
      handleForgeRequest(new Request("https://forge.openagents.com/api/forge")),
    )

    expect(response.status).toBe(404)
  })

  it("keeps the exported contract aligned with live route metadata", () => {
    expect(forgeShellContract()).toEqual({
      service: "openagents-forge",
      version: FORGE_UI_WORKER_VERSION,
      mount: defaultForgeMount,
      routes: forgeShellRoutes,
      preview: forgeShellPreviewState,
    })
  })
})
