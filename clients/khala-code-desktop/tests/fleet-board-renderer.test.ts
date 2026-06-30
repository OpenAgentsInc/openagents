import { describe, expect, test } from "bun:test"

import { buildKhalaFleetBoardProjection } from "../src/ui/fleet-board-projection"
import { renderKhalaFleetBoardHtml } from "../src/ui/fleet-board-renderer"
import type { KhalaCodeDesktopFleetStatus } from "../src/shared/rpc"

const status = (): KhalaCodeDesktopFleetStatus => ({
  ok: true,
  observedAt: "2026-06-30T20:00:00.000Z",
  pylon: {
    status: "online",
    pylonRef: "pylon.local.render",
    message: "online",
  },
  availableCodexAssignments: 2,
  maxCodexAssignments: 4,
  accounts: [
    {
      accountRef: "codex",
      provider: "codex",
      readiness: "ready",
      quotaState: "available",
      accountKey: null,
      email: "operator@example.com",
    },
  ],
  activeAssignments: [
    {
      assignmentRef: "assignment.public.render",
      issueRef: "github.issue.openagents.7768",
      updatedAt: "2026-06-30T20:01:00.000Z",
    },
  ],
  processes: [],
})

describe("Khala Code Fleet board renderer", () => {
  test("renders a board graph and accessible run timeline", () => {
    const projection = buildKhalaFleetBoardProjection({ status: status() })
    const rendered = renderKhalaFleetBoardHtml(projection, { reducedMotion: true })

    expect(rendered.html).toContain('class="khala-fleet-board"')
    expect(rendered.html).toContain(
      'data-fleet-board="openagents.khala_code.fleet_board_projection.v0"',
    )
    expect(rendered.graph.svg).toContain('viewBox="0 0 1480 430"')
    expect(rendered.graph.svg).toContain('data-node-id="codex-workers"')
    expect(rendered.graph.svg).toContain('data-node-id="run-timeline"')
    expect(rendered.graph.svg).toContain('data-reduced-motion="true"')
    expect(rendered.timelineHtml).toContain("Run timeline")
    expect(rendered.timelineHtml).toContain("Assignment active")
    expect(rendered.timelineHtml).toContain("assignment.public.render")
    expect(rendered.html).toContain("2/4 slots free")
  })

  test("escapes timeline content and keeps private fields out of rendered HTML", () => {
    const projection = buildKhalaFleetBoardProjection({
      status: {
        ...status(),
        pylon: {
          status: "online",
          pylonRef: "pylon.local.render",
          message: "Bearer sk-local /Users/operator/.codex/auth.json",
        },
        accounts: [
          {
            accountRef: "<script>alert(1)</script>",
            provider: "codex",
            readiness: "ready",
            quotaState: null,
            accountKey: null,
            email: "operator@example.com",
          },
        ],
        activeAssignments: [
          {
            assignmentRef: "assignment.public.<render>",
            issueRef: "github.issue.openagents.7768",
            updatedAt: "2026-06-30T20:01:00.000Z",
          },
        ],
      },
    })
    const rendered = renderKhalaFleetBoardHtml(projection)

    expect(rendered.html).not.toMatch(
      /operator@example\.com|Bearer|sk-local|\/Users\/|auth\.json|<script>|assignment\.public\.<render>/,
    )
    expect(rendered.html).toContain("account.khala_fleet.codex.1")
    expect(rendered.html).toContain("assignment.khala_fleet.pending.1")
  })

  test("the Fleet panel mounts the board from live fleet status", async () => {
    const panel = await Bun.file(new URL("../src/ui/fleet-status.ts", import.meta.url)).text()
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(panel).toContain("buildKhalaFleetBoardProjection")
    expect(panel).toContain("renderKhalaFleetBoardHtml")
    expect(panel).toContain('matchMedia("(prefers-reduced-motion: reduce)")')
    expect(panel).toContain("appendFleetBoard(container, data)")
    expect(css).toContain(".khala-fleet-board")
    expect(css).toContain(".khala-fleet-timeline-list")
  })
})
