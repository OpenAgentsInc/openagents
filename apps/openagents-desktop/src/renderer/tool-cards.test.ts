/**
 * Typed tool-call card projection (EP250, #8712). Owner statement (verbatim):
 * "why don't you go improve the UI of those tool calls so it's not just JSON
 * stuff? Like I thought we had some custom components that showed things
 * properly, not these JSON blobs."
 */
import { describe, expect, test } from "vite-plus/test"

import { parseFableLocalTraceNoteText } from "../fable-local-contract.ts"
import type { DesktopNoteEntry } from "./shell.ts"
import {
  compactArgSummary,
  humanizeToolInvocation,
  projectTranscriptEntries,
  readToolArgs,
  toolCardIcon,
  toolResultSnippet,
  toolTraceFromNote,
  projectToolCardEntries,
  contextGroupSummary,
} from "./tool-cards.ts"

const note = (input: Partial<DesktopNoteEntry> & Pick<DesktopNoteEntry, "key" | "text">): DesktopNoteEntry => ({
  role: "system",
  timestamp: "18:05",
  ...input,
} as DesktopNoteEntry)

describe("parseFableLocalTraceNoteText (deterministic inverse of the serializer)", () => {
  test("round-trips started / ok / failed trace lines", () => {
    expect(parseFableLocalTraceNoteText('Read · started · {"file_path":"notes.md"}')).toEqual({
      toolName: "Read",
      phase: "started",
      summary: '{"file_path":"notes.md"}',
    })
    expect(parseFableLocalTraceNoteText("Read · ok")).toEqual({
      toolName: "Read",
      phase: "ok",
      summary: "",
    })
    expect(parseFableLocalTraceNoteText("mcp__codex__delegate · failed · revoked token")).toEqual({
      toolName: "mcp__codex__delegate",
      phase: "failed",
      summary: "revoked token",
    })
  })

  test("non-trace system text never parses as a tool trace", () => {
    expect(parseFableLocalTraceNoteText("Claude · claude-fable-5")).toBeNull()
    expect(parseFableLocalTraceNoteText("The model request failed.")).toBeNull()
    expect(parseFableLocalTraceNoteText("hello")).toBeNull()
  })
})

describe("toolTraceFromNote", () => {
  test("prefers typed meta.trace over text parsing", () => {
    const typed = note({
      key: "t1",
      text: "Read · started · legacy-text",
      meta: { trace: { toolName: "Read", phase: "started", summary: '{"file_path":"a.md"}' } },
    })
    expect(toolTraceFromNote(typed)?.summary).toBe('{"file_path":"a.md"}')
  })

  test("falls back to parsing persisted pre-typed note text; non-system notes never match", () => {
    expect(toolTraceFromNote(note({ key: "t2", text: "Bash · ok · done" }))?.phase).toBe("ok")
    expect(toolTraceFromNote(note({ key: "t3", text: "Read · started", role: "assistant" }))).toBeNull()
  })
})

describe("projectTranscriptEntries (started + completion = ONE updating card)", () => {
  test("folds a started/ok pair into a single card with the result attached", () => {
    const entries = projectTranscriptEntries([
      note({ key: "u", text: "hi", role: "user" }),
      note({ key: "s1", text: 'Read · started · {"file_path":"notes.md"}' }),
      note({ key: "s2", text: "Read · ok · bounded fixture read" }),
      note({ key: "a", text: "done", role: "assistant" }),
    ])
    expect(entries.map((entry) => entry.kind)).toEqual(["note", "tool", "note"])
    const tool = entries[1]
    if (tool?.kind !== "tool") throw new Error("expected tool entry")
    expect(tool.card).toMatchObject({
      key: "s1",
      toolName: "Read",
      status: "ok",
      argsSummary: '{"file_path":"notes.md"}',
      resultSummary: "bounded fixture read",
    })
  })

  test("a still-running invocation renders one running card", () => {
    const entries = projectTranscriptEntries([
      note({ key: "s1", text: 'Bash · started · {"command":"ls"}' }),
    ])
    expect(entries).toHaveLength(1)
    const tool = entries[0]
    if (tool?.kind !== "tool") throw new Error("expected tool entry")
    expect(tool.card.status).toBe("running")
    expect(tool.card.resultSummary).toBeNull()
  })

  test("failed completion updates the same card with the failure text", () => {
    const entries = projectTranscriptEntries([
      note({ key: "s1", text: 'mcp__codex__delegate · started · {"task":"Say hi"}' }),
      note({ key: "s2", text: "mcp__codex__delegate · failed · Codex account codex needs reconnection: credentials revoked" }),
    ])
    expect(entries).toHaveLength(1)
    const tool = entries[0]
    if (tool?.kind !== "tool") throw new Error("expected tool entry")
    expect(tool.card.status).toBe("failed")
    expect(tool.card.resultSummary).toContain("needs reconnection")
  })

  test("same-tool invocations pair FIFO by note order (no invocation id exists)", () => {
    const notes = [
      note({ key: "s1", text: 'Read · started · {"file_path":"a.md"}' }),
      note({ key: "s2", text: 'Read · started · {"file_path":"b.md"}' }),
      note({ key: "s3", text: "Read · ok · first result" }),
      note({ key: "s4", text: "Read · failed · second result" }),
    ]
    // The per-invocation pairing layer keeps ONE updating card per
    // invocation (the landed contract oracle).
    const cards = projectToolCardEntries(notes)
    expect(cards).toHaveLength(2)
    const [first, second] = cards
    if (first?.kind !== "tool" || second?.kind !== "tool") throw new Error("expected tool entries")
    expect(first.card).toMatchObject({ key: "s1", status: "ok", resultSummary: "first result" })
    expect(second.card).toMatchObject({ key: "s2", status: "failed", resultSummary: "second result" })
    // EP250 card reconciliation TIGHTENS the presentation: consecutive
    // read/glob/grep cards fold into one context-group row that carries the
    // same per-invocation members (data preserved, presentation grouped).
    const grouped = projectTranscriptEntries(notes)
    expect(grouped).toHaveLength(1)
    const group = grouped[0]
    if (group?.kind !== "context-group") throw new Error("expected context group")
    expect(group.group.cards.map((card) => card.key)).toEqual(["s1", "s2"])
    expect(group.group.failed).toBe(true)
    expect(group.group.running).toBe(false)
    expect(contextGroupSummary(group.group)).toBe("2 reads")
  })

  test("context grouping: only runs of 2+ consecutive read/glob/grep group; other tools break the run", () => {
    const entries = projectTranscriptEntries([
      note({ key: "r1", text: 'Read · started · {"file_path":"a.md"}' }),
      note({ key: "g1", text: 'Grep · started · {"pattern":"foo"}' }),
      note({ key: "b1", text: 'Bash · started · {"command":"ls"}' }),
      note({ key: "r2", text: 'Read · started · {"file_path":"b.md"}' }),
    ])
    // r1+g1 form a group ("Gathering context…" while running); Bash breaks
    // the run; the trailing single Read stays a plain tool card.
    expect(entries.map((entry) => entry.kind)).toEqual(["context-group", "tool", "tool"])
    const group = entries[0]
    if (group?.kind !== "context-group") throw new Error("expected context group")
    expect(group.group.running).toBe(true)
    expect(group.group.reads).toBe(1)
    expect(group.group.searches).toBe(1)
    expect(contextGroupSummary(group.group)).toBe("1 read, 1 search")
    const single = entries[2]
    if (single?.kind !== "tool") throw new Error("expected plain tool card")
    expect(single.card.key).toBe("r2")
  })

  test("an orphan completion renders as its own completed card", () => {
    const entries = projectTranscriptEntries([
      note({ key: "s1", text: "Read · ok · dangling result" }),
    ])
    expect(entries).toHaveLength(1)
    const tool = entries[0]
    if (tool?.kind !== "tool") throw new Error("expected tool entry")
    expect(tool.card).toMatchObject({ status: "ok", argsSummary: "", resultSummary: "dangling result" })
  })

  test("question notes project as question entries; ordinary notes stay notes", () => {
    const question = note({
      key: "q1",
      text: "Which path?",
      question: {
        turnRef: "turn.fable.x",
        questionRef: "question.1",
        status: "pending",
        questions: [{ question: "Which path?", header: "Fixture", multiSelect: false, options: [{ label: "A" }] }],
      },
    })
    const entries = projectTranscriptEntries([question, note({ key: "m", text: "Claude · claude-fable-5" })])
    expect(entries.map((entry) => entry.kind)).toEqual(["question", "note"])
  })
})

describe("humanizeToolInvocation (the humanization table)", () => {
  const table: ReadonlyArray<[toolName: string, args: string, title: string, detail: string]> = [
    ["mcp__codex__delegate", '{"task":"Say hi in Spanish"}', "Delegate to Codex", "Say hi in Spanish"],
    ["Agent", '{"description":"Audit the parser","model":"sonnet"}', "Agent", "Audit the parser · sonnet"],
    ["Agent", '{"description":"Audit the parser"}', "Agent", "Audit the parser"],
    ["Bash", '{"command":"ls -la","description":"List files"}', "Bash", "List files"],
    ["Bash", '{"command":"ls -la"}', "Bash", "ls -la"],
    ["Read", '{"file_path":"/tmp/notes.md"}', "Read", "/tmp/notes.md"],
    ["Write", '{"file_path":"/tmp/out.md","content":"x"}', "Write", "/tmp/out.md"],
    ["Edit", '{"file_path":"src/shell.ts"}', "Edit", "src/shell.ts"],
    ["Glob", '{"pattern":"**/*.ts"}', "Glob", "**/*.ts"],
    ["Grep", '{"pattern":"IconButton"}', "Grep", "IconButton"],
    ["ToolSearch", '{"query":"select:Read"}', "Tool search", "select:Read"],
    ["WebSearch", '{"query":"effect native"}', "Web search", "effect native"],
    ["WebFetch", '{"url":"https://example.com"}', "Web fetch", "https://example.com"],
  ]
  test.each(table)("%s → humanized primary line", (toolName, args, title, detail) => {
    expect(humanizeToolInvocation(toolName, args)).toEqual({ title, detail })
  })

  test("unknown tools get a bounded compact string-field summary — never a raw JSON dump", () => {
    const human = humanizeToolInvocation(
      "mystery_tool",
      '{"target":"prod","count":3,"nested":{"deep":"hidden"},"flag":true}',
    )
    expect(human.title).toBe("Mystery tool")
    expect(human.detail).toContain("target: prod")
    expect(human.detail).toContain("count: 3")
    expect(human.detail).toContain("flag: true")
    expect(human.detail).not.toContain("{")
    expect(human.detail).not.toContain("}")
    expect(human.detail).not.toContain('"')
  })

  test("truncated (bound-clipped) JSON still yields string fields, not a parse crash", () => {
    const truncated = '{"task":"Do the thing","context":"long text that got clip'
    expect(humanizeToolInvocation("mcp__codex__delegate", truncated).detail).toBe("Do the thing")
    expect(readToolArgs(truncated)["task"]).toBe("Do the thing")
  })

  test("long details are truncated with an ellipsis", () => {
    const long = "x".repeat(400)
    const human = humanizeToolInvocation("Read", `{"file_path":"${long}"}`)
    expect(human.detail.length).toBeLessThanOrEqual(160)
    expect(human.detail.endsWith("…")).toBe(true)
  })
})

describe("result snippets, compact summaries, icons", () => {
  test("toolResultSnippet takes the first non-empty line, bounded", () => {
    expect(toolResultSnippet("\n\nfirst line\nsecond line")).toBe("first line")
    expect(toolResultSnippet("y".repeat(300)).length).toBeLessThanOrEqual(200)
  })

  test("compactArgSummary stays inside its budget", () => {
    const summary = compactArgSummary({ a: "1".repeat(100), b: "2".repeat(100), c: "3".repeat(100) })
    expect(summary.length).toBeLessThanOrEqual(160)
  })

  test("icons reuse the history-workspace tool vocabulary", () => {
    expect(toolCardIcon("Bash")).toBe("Terminal")
    expect(toolCardIcon("Edit")).toBe("Code")
    expect(toolCardIcon("Read")).toBe("Folder")
    expect(toolCardIcon("mcp__codex__delegate")).toBe("Agent")
    expect(toolCardIcon("mystery")).toBe("Tools")
  })
})

describe("typed WorkbenchItem passthrough (#8859)", () => {
  const runningItem = {
    kind: "command",
    source: "codex",
    command: "pnpm test",
    status: "in_progress",
  } as const
  const completedItem = {
    kind: "command",
    source: "codex",
    command: "pnpm test",
    status: "completed",
    exitCode: 0,
    durationMs: 950,
    outputTail: "42 passed",
  } as const

  test("a started trace item rides the card and the completion item supersedes it in place", () => {
    const entries = projectToolCardEntries([
      note({
        key: "b1",
        text: "Bash · started",
        meta: { trace: { toolName: "Bash", phase: "started", summary: '{"command":"pnpm test"}', item: runningItem } },
      }),
      note({
        key: "b2",
        text: "Bash · ok · 42 passed",
        meta: { trace: { toolName: "Bash", phase: "ok", summary: "42 passed", item: completedItem } },
      }),
    ])
    expect(entries).toHaveLength(1)
    const entry = entries[0]!
    if (entry.kind !== "tool") throw new Error("expected a tool card")
    expect(entry.card.status).toBe("ok")
    expect(entry.card.item).toEqual(completedItem)
    // The string contract is untouched — summaries still populate the card.
    expect(entry.card.argsSummary).toBe('{"command":"pnpm test"}')
    expect(entry.card.resultSummary).toBe("42 passed")
  })

  test("a completion without a typed item keeps the started item; notes without items stay item-free", () => {
    const entries = projectToolCardEntries([
      note({
        key: "c1",
        text: "Bash · started",
        meta: { trace: { toolName: "Bash", phase: "started", summary: "", item: runningItem } },
      }),
      note({
        key: "c2",
        text: "Bash · failed · exit 1",
        meta: { trace: { toolName: "Bash", phase: "failed", summary: "exit 1" } },
      }),
      note({
        key: "d1",
        text: "Read · started",
        meta: { trace: { toolName: "Read", phase: "started", summary: "" } },
      }),
    ])
    const cards = entries.flatMap(entry => entry.kind === "tool" ? [entry.card] : [])
    expect(cards).toHaveLength(2)
    expect(cards[0]!.status).toBe("failed")
    expect(cards[0]!.item).toEqual(runningItem)
    expect(cards[1]!.item).toBeUndefined()
  })
})
