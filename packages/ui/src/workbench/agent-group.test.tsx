/**
 * First component-level test in `packages/ui` (#8867 T10, epic #8857 Wave 2).
 * `DesktopAgentGroup` has no interactive event surface worth a full DOM
 * (`renderToStaticMarkup` is enough to assert structure/text/data-attributes),
 * so this stays dependency-light rather than pulling in `happy-dom` for the
 * package's very first test.
 */
import { describe, expect, test } from "vite-plus/test"
import { renderToStaticMarkup } from "react-dom/server"

import { DesktopAgentGroup, type DesktopAgentActivity } from "./agent-group.tsx"

const agent = (overrides: Partial<DesktopAgentActivity> = {}): DesktopAgentActivity => ({
  agentKey: "agent-1",
  detail: "Reading files",
  name: "timeline-builder",
  role: "Delegated agent",
  status: "running",
  ...overrides,
})

describe("DesktopAgentGroup rendering states", () => {
  test("renders a single agent row with its name, role, and mono-uppercase status", () => {
    const html = renderToStaticMarkup(
      <DesktopAgentGroup agents={[agent({ status: "completed" })]} itemKey="i1" />,
    )
    expect(html).toContain('data-timeline-key="i1"')
    expect(html).toContain('data-kind="collabAgentToolCall"')
    expect(html).toContain("timeline-builder")
    expect(html).toContain("Delegated agent")
    expect(html).toContain("Done")
    expect(html).toContain("1 done")
  })

  test("every coarse status renders its default label and a distinct data-status", () => {
    for (const [status, label] of [
      ["completed", "Done"], ["failed", "Failed"], ["running", "Running"], ["waiting", "Waiting"],
    ] as const) {
      const html = renderToStaticMarkup(<DesktopAgentGroup agents={[agent({ status })]} itemKey="i" />)
      expect(html).toContain(`data-status="${status}"`)
      expect(html).toContain(label)
    }
  })

  test("statusLabel overrides the computed label (exact CollabAgentStatus wire text)", () => {
    const html = renderToStaticMarkup(
      <DesktopAgentGroup agents={[agent({ status: "waiting", statusLabel: "PENDING INIT" })]} itemKey="i" />,
    )
    expect(html).toContain("PENDING INIT")
    expect(html).not.toContain(">Waiting<")
  })

  test("multiple agents in one group summarize done/running counts", () => {
    const html = renderToStaticMarkup(<DesktopAgentGroup
      agents={[
        agent({ agentKey: "a", status: "completed" }),
        agent({ agentKey: "b", status: "running" }),
        agent({ agentKey: "c", status: "running" }),
      ]}
      itemKey="group"
    />)
    expect(html).toContain("1 done · 2 running")
  })

  test("operation tag and bounded prompt line render once for the group", () => {
    const html = renderToStaticMarkup(<DesktopAgentGroup
      agents={[agent()]}
      itemKey="i"
      operation="spawn"
      prompt="Implement the file-change card"
    />)
    expect(html).toContain('data-operation="spawn"')
    expect(html).toContain(">spawn<")
    expect(html).toContain("Implement the file-change card")
  })

  test("omitting operation/prompt renders neither element (back-compat, no visual regression)", () => {
    const html = renderToStaticMarkup(<DesktopAgentGroup agents={[agent()]} itemKey="i" />)
    expect(html).not.toContain("oa-react-agent-operation")
    expect(html).not.toContain("oa-react-agent-prompt")
  })

  test("path/nickname renders as a second heading line distinct from the primary name", () => {
    const html = renderToStaticMarkup(
      <DesktopAgentGroup agents={[agent({ path: "timeline-builder/a11y-oracle" })]} itemKey="i" />,
    )
    expect(html).toContain('class="oa-react-agent-path"')
    expect(html).toContain("timeline-builder/a11y-oracle")
  })

  test("path identical to name does not render a redundant second line", () => {
    const html = renderToStaticMarkup(
      <DesktopAgentGroup agents={[agent({ name: "child-1", path: "child-1" })]} itemKey="i" />,
    )
    expect(html).not.toContain("oa-react-agent-path")
  })

  test("activityKind renders a distinct bracket tag beside the detail line", () => {
    for (const kind of ["started", "interacted", "interrupted"] as const) {
      const html = renderToStaticMarkup(
        <DesktopAgentGroup agents={[agent({ activityKind: kind })]} itemKey="i" />,
      )
      expect(html).toContain(`data-activity="${kind}"`)
      expect(html).toContain(`>${kind}<`)
    }
  })

  test("no activityKind renders no activity tag", () => {
    const html = renderToStaticMarkup(<DesktopAgentGroup agents={[agent()]} itemKey="i" />)
    expect(html).not.toContain("oa-react-agent-activity-tag")
  })

  test("Interrupt only renders when BOTH interruptable and onInterrupt are supplied", () => {
    const withBoth = renderToStaticMarkup(<DesktopAgentGroup
      agents={[agent({ interruptable: true, onInterrupt: () => {} })]}
      itemKey="i"
    />)
    expect(withBoth).toContain("Interrupt")
    expect(withBoth).toContain('aria-label="Interrupt timeline-builder"')

    const noHandler = renderToStaticMarkup(<DesktopAgentGroup agents={[agent({ interruptable: true })]} itemKey="i" />)
    expect(noHandler).not.toContain("oa-react-agent-interrupt")

    const notInterruptable = renderToStaticMarkup(<DesktopAgentGroup
      agents={[agent({ interruptable: false, onInterrupt: () => {} })]}
      itemKey="i"
    />)
    expect(notInterruptable).not.toContain("oa-react-agent-interrupt")
  })

  test("no interrupt props at all (every pre-existing caller) never renders the control", () => {
    const html = renderToStaticMarkup(<DesktopAgentGroup agents={[agent()]} itemKey="i" />)
    expect(html).not.toContain("Interrupt")
  })

  test("nested transcript rows still render with the parent line (unchanged shape)", () => {
    const html = renderToStaticMarkup(<DesktopAgentGroup
      agents={[agent({
        depth: 1,
        parent: "timeline-builder",
        transcript: [{ label: "spawnAgent", text: "Nested child spawned for review." }],
      })]}
      itemKey="i"
    />)
    expect(html).toContain('data-depth="1"')
    expect(html).toContain("spawned by timeline-builder")
    expect(html).toContain("Nested child spawned for review.")
  })
})
