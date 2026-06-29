// VCODE-11 (#5928): projected Terminal/Log pane hardening.
//
// Covers log redaction, digest refs, explicit focus ownership, and the
// hidden-pane invariant that prevents closed terminal UI from stealing Verse
// mouselook.

import { describe, expect, test } from "bun:test"

import type { NodeStateMessage, SessionEventRow } from "../src/shared/rpc"
import { interpretKey } from "../src/ui/keyboard"
import { initialModel, Model } from "../src/ui/model"
import { GotNodeState, SelectedSession, SelectedSessionDetailView } from "../src/ui/message"
import { projectTerminalLogPane } from "../src/ui/terminal-log-projection"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === "object" && value !== null

const renderHtml = (node: unknown): string => {
  if (!isVNodeLike(node)) return ""
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, on]) => on)
    .map(([c]) => c)
    .join(" ")
  const attrStr = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes ? [["class", classes] as const] : []),
  ]
    .filter(([, v]) => v !== false && v !== undefined && v !== null)
    .map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${String(v)}"`))
    .join("")
  const tag = node.sel ?? "node"
  const children = (node.children ?? [])
    .map((c) => (typeof c === "string" ? c : renderHtml(c)))
    .join("")
  return `<${tag}${attrStr}>${node.text ?? ""}${children}</${tag}>`
}

const event = (eventIndex: number, detail: string, full?: string): SessionEventRow => ({
  eventIndex,
  phase: "composer_event",
  state: "running",
  observedAt: "2026-06-21T22:00:00.000Z",
  detail,
  ...(full === undefined ? {} : { full }),
})

const sessionRef = "session.pylon.codex.terminal"

const nodeWithEvents = (events: readonly SessionEventRow[]): NodeStateMessage => ({
  ok: true,
  schema: "openagents.pylon.control.v0.3",
  sessions: [
    {
      sessionRef,
      adapter: "codex",
      state: "running",
      accountRefHash: null,
      updatedAt: "2026-06-21T22:00:00.000Z",
    },
  ],
  events: { [sessionRef]: [...events] },
})

describe("Terminal/Log projection (#5928)", () => {
  test("redacts unsafe output and attaches digest refs", () => {
    const projection = projectTerminalLogPane({
      sessionRef,
      events: [
        event(0, "completed: bun test exit 0"),
        event(1, "env OPENAI_API_KEY=sk-secretsecretsecret"),
        event(2, "wallet sp1q1234567890abcdefghijklmnop"),
        event(3, "read /Users/private/.secrets/token.json"),
        event(4, "provider", `provider payload ${"{\"raw\":\""}${"x".repeat(120)}\"}`),
      ],
    })

    expect(projection.rows).toHaveLength(5)
    expect(projection.rows[0]?.redacted).toBe(false)
    expect(projection.rows.slice(1).every((row) => row.redacted)).toBe(true)
    expect(projection.rows.slice(1).every((row) => row.digestRef?.startsWith("digest.terminal_log."))).toBe(true)
    const serialized = JSON.stringify(projection)
    expect(serialized).not.toContain("sk-secretsecretsecret")
    expect(serialized).not.toContain("sp1q1234567890")
    expect(serialized).not.toContain("/Users/private")
    expect(serialized).not.toContain("\"raw\"")
    expect(serialized).toContain("[env]")
    expect(serialized).toContain("[wallet material]")
    expect(serialized).toContain("[local path]")
    expect(serialized).toContain("[provider payload]")
  })
})

describe("Terminal/Log pane focus contract (#5928)", () => {
  test("rendered pane owns text selection and blocks scene controls while focused", () => {
    let model = Model.make({ ...initialModel, pane: "session-detail" })
    ;[model] = update(
      model,
      GotNodeState({
        node: nodeWithEvents([
          event(0, "completed: bun test exit 0"),
          event(1, "env OPENAI_API_KEY=sk-secretsecretsecret"),
        ]),
      }),
    )
    ;[model] = update(model, SelectedSession({ sessionRef }))
    ;[model] = update(model, SelectedSessionDetailView({ view: "terminal-log" }))

    const rendered = renderHtml((view(model) as { body: unknown }).body)
    expect(rendered).toContain('data-autopilot-terminal-focus-owner="terminal-log"')
    expect(rendered).toContain('data-autopilot-terminal-scene-controls="blocked"')
    expect(rendered).toContain('data-autopilot-terminal-hidden-policy="inert"')
    expect(rendered).toContain('data-autopilot-terminal-text-selection="owned"')
    expect(rendered).toContain('data-autopilot-terminal-copy-buffer="projected"')
    expect(rendered).toContain('data-autopilot-terminal-log-redacted="true"')
    expect(rendered).toContain("digest.terminal_log.")
    expect(rendered).not.toContain("sk-secretsecretsecret")
  })

  test("hidden terminal pane is not rendered and cannot intercept Verse mouselook", () => {
    const model = Model.make({ ...initialModel, pane: "chat", verseMode: "explore" })
    const rendered = renderHtml((view(model) as { body: unknown }).body)
    expect(rendered).not.toContain("data-autopilot-terminal-focus-owner")
    expect(rendered).not.toContain("data-autopilot-terminal-text-selection")
  })

  test("editable terminal focus blocks shortcuts; blur returns shortcuts to Verse", () => {
    const model = Model.make({ ...initialModel, pane: "terminal-log" })
    expect(
      interpretKey(model, {
        key: "v",
        meta: true,
        ctrl: false,
        shift: true,
        inEditable: true,
      }).kind,
    ).toBe("none")
    expect(
      interpretKey(model, {
        key: "v",
        meta: true,
        ctrl: false,
        shift: true,
        inEditable: false,
      }).kind,
    ).toBe("toggle-verse")
  })
})
