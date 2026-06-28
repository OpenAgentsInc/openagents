import { describe, expect, it } from "bun:test"
import { Effect } from "effect"

import {
  FORGE_UI_WORKER_VERSION,
  defaultForgeMount,
  forgeLandingCopy,
  handleForgeRequest,
  renderForgeLandingHtml,
} from "./index"

describe("Forge UI Worker", () => {
  it("renders the required landing copy from the separate Forge app", () => {
    const html = renderForgeLandingHtml()

    expect(html).toContain(forgeLandingCopy.title)
    expect(html).toContain(forgeLandingCopy.tagline)
    expect(html).toContain('data-forge-app="landing"')
    expect(html).toContain('data-shared-ui-package="@openagentsinc/ui"')
    expect(html).toContain("--forge-accent")
  })

  it("serves the landing page at the root route", async () => {
    const response = await Effect.runPromise(
      handleForgeRequest(new Request("https://forge.openagents.com/")),
    )
    const html = await response.text()

    expect(response.status).toBe(200)
    expect(response.headers.get("content-type")).toContain("text/html")
    expect(html).toContain("THE FORGE")
    expect(html).toContain("where agents git it on")
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
    })
  })

  it("keeps unknown routes closed", async () => {
    const response = await Effect.runPromise(
      handleForgeRequest(new Request("https://forge.openagents.com/api/forge")),
    )

    expect(response.status).toBe(404)
  })
})
