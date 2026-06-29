import { describe, expect, it } from "bun:test"
import { Effect } from "effect"

import {
  FORGE_UI_WORKER_VERSION,
  OPENAGENTS_FORGE_DEFAULT_BRANCH_REF,
  OPENAGENTS_FORGE_REPOSITORY_REF,
  OPENAGENTS_FORGE_TENANT_REF,
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

  it("defines shell routes for dogfood, work, changes, verification, queue, refs, and mirror", () => {
    expect(forgeShellRoutes.map(route => route.path)).toEqual([
      "/",
      "/dogfood",
      "/work",
      "/changes",
      "/verification",
      "/queue",
      "/refs",
      "/mirror",
    ])
    expect(forgeShellRoutes.map(route => route.apiPath)).toEqual([
      "/api/forge/overview",
      "/api/forge/dogfood-lanes",
      "/api/forge/work-records",
      "/api/forge/changes",
      "/api/forge/verification-receipts",
      "/api/forge/queue",
      "/api/forge/refs",
      "/api/forge/github-mirror-runs",
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
    expect(body.preview.dataMode).toBe("live-api-contract")
    expect(body.preview.apiBasePath).toBe("/api/forge")
    expect(body.preview.dogfoodLanes).toHaveLength(1)
    expect(body.preview.dogfoodLanes[0]?.issueRef).toBe("#6797")
    expect(body.routes).toContainEqual({
      id: "mirror",
      path: "/mirror",
      label: "Mirror",
      summary: "GitHub mirror receipts and attention state",
      apiPath: "/api/forge/github-mirror-runs",
    })
  })

  it("renders a direct route shell without the old logged-in Forge page", () => {
    const html = renderForgeShellHtml("changes")

    expect(html).toContain('data-forge-route="changes"')
    expect(html).toContain("Change Inspector")
    expect(html).not.toContain("loggedIn/page/forge")
  })

  it("renders the SU-7 dogfood lane with intake through mirror refs", () => {
    const html = renderForgeShellHtml("dogfood")

    expect(html).toContain('data-forge-route="dogfood"')
    expect(html).toContain('data-forge-dogfood-lane="lane.forge.su7.openagents-codex-low-risk"')
    expect(html).toContain("OpenAgentsInc/openagents")
    expect(html).toContain("refs/forge/intake/openagents/codex-low-risk")
    expect(html).toContain("receipt.forge.su7.su5-check-deploy")
    expect(html).toContain("queue.forge.su7.nextActualPromotion")
    expect(html).toContain("promotion.forge.su7.su4-blueprint-gated")
    expect(html).toContain("mirror.github.openagents.main.su7")
    expect(html).toContain("bun run --cwd apps/openagents.com check:deploy")
    expect(html).toContain("GitHub stays downstream visibility only")
  })

  it("renders the OpenAgents canonical repo in the refs view", () => {
    const html = renderForgeShellHtml("refs")

    expect(html).toContain(OPENAGENTS_FORGE_TENANT_REF)
    expect(html).toContain(OPENAGENTS_FORGE_REPOSITORY_REF)
    expect(html).toContain(OPENAGENTS_FORGE_DEFAULT_BRANCH_REF)
    expect(html).toContain("OpenAgentsInc/openagents default branch")
    expect(html).toContain("/api/forge/refs live canonical store")
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
