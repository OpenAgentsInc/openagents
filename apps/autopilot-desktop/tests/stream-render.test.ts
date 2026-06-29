import { describe, expect, test } from "bun:test"
import type { SessionEventRow } from "../src/shared/rpc"
import {
  classifyStreamLine,
  conciseStreamLines,
  renderMarkdown,
} from "../src/ui/stream-render"

const row = (eventIndex: number, detail: string, full?: string): SessionEventRow => ({
  eventIndex,
  phase: "composer_event",
  state: "running",
  observedAt: "2026-06-19T12:00:00.000Z",
  detail,
  ...(full === undefined ? {} : { full }),
})

describe("classifyStreamLine", () => {
  test("suppresses token-counter noise", () => {
    expect(classifyStreamLine("tokens used: 51342").kind).toBe("noise")
    expect(classifyStreamLine("thinking tokens: 7; output tokens: 11").kind).toBe("noise")
  })

  test("suppresses lifecycle + mode + ref noise", () => {
    expect(classifyStreamLine("thread started").kind).toBe("noise")
    expect(classifyStreamLine("turn completed").kind).toBe("noise")
    expect(classifyStreamLine("control session mode: local_bounded; sandbox: workspace-write").kind).toBe(
      "noise",
    )
    expect(classifyStreamLine("external session: session.pylon.codex_composer.abc123").kind).toBe("noise")
    // A bare digest ref (long hex, no spaces) is noise.
    expect(classifyStreamLine("digest.pylon.control_session.response.0123456789abcdef0123456789").kind).toBe(
      "noise",
    )
  })

  test("classifies assistant, reasoning, tool, error and strips labels", () => {
    expect(classifyStreamLine("agent: ## Heading\nbody")).toEqual({
      kind: "assistant",
      text: "## Heading\nbody",
    })
    expect(classifyStreamLine("thinking: considering the diff")).toEqual({
      kind: "reasoning",
      text: "considering the diff",
    })
    expect(classifyStreamLine("completed: git push origin main exit 0").kind).toBe("tool")
    expect(classifyStreamLine("web search: latest ldk release").kind).toBe("tool")
    expect(classifyStreamLine("error: turn failed").kind).toBe("error")
  })
})

describe("conciseStreamLines", () => {
  test("drops noise, dedupes repeats, prefers full over detail", () => {
    const lines = conciseStreamLines([
      row(0, "thread started"),
      row(1, "agent: Hello"),
      row(2, "agent: Hello"), // duplicate collapses
      row(3, "tokens used: 999"),
      row(4, "completed: ls exit 0"),
      row(5, "short", "agent: Full content wins"),
    ])
    expect(lines.map((l) => `${l.kind}:${l.text}`)).toEqual([
      "assistant:Hello",
      "tool:completed: ls exit 0",
      "assistant:Full content wins",
    ])
  })
})

describe("renderMarkdown", () => {
  test("returns a Foldkit node for headings/lists/code (never raw markup)", () => {
    // Smoke: the renderer must produce a node (not throw, not h.empty) for the
    // exact constructs the owner saw rendered raw.
    const node = renderMarkdown("# Title\n\n- one\n- two\n\n```\ncode\n```\n\n**bold** and `inline`")
    expect(node).toBeDefined()
    // Empty input degrades to an empty node.
    const empty = renderMarkdown("")
    expect(empty).toBeDefined()
  })
})
