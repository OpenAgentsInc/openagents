import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage } from "./view.js"
import { statusChip } from "./view.js"

// CS-A3 (#5363): the shared structured diff viewer. This is the UI port of the
// Pylon `diff-review.ts` ChangeSet model (apps/pylon/src/tas/diff-review.ts):
// a coding turn's file edits render as readable, structured per-file rows with
// +/- counts and (when present) hunk lines, instead of a flat transcript row.
//
// It lives in @openagentsinc/autopilot-ui so web reuses it (per the desktop
// AGENTS.md rule: add shared components here, not one-offs in the desktop). The
// component is public-safe and refs-only: paths are the public-safe file refs
// the node already emits in its session event tail, never raw absolute paths.

// Mirrors apps/pylon/src/tas/diff-review.ts `ChangeFileStatus`.
export type DiffFileStatus = "added" | "modified" | "deleted"

// One optional rendered hunk line (a +/-/context line). When a turn surfaces a
// raw patch body we can show it; when it only surfaces per-file +/- counts we
// render the file row without hunk lines. Either way the component is honest
// about what the node actually emitted.
export type DiffHunkLine = Readonly<{
  kind: "added" | "removed" | "context" | "meta"
  text: string
}>

// One file in the change set. Mirrors `ChangeSetFile` plus optional hunk lines.
export type DiffReviewFile = Readonly<{
  path: string
  status: DiffFileStatus
  added: number
  removed: number
  hunkLines?: ReadonlyArray<DiffHunkLine>
}>

// Mirrors `ChangeSetSummary`.
export type DiffReviewSummary = Readonly<{
  fileCount: number
  totalAdded: number
  totalRemoved: number
}>

// #5470: how a file's hunk body is laid out when present.
//   - "unified" (default): the existing single-column +/- pre block.
//   - "split":  a side-by-side removed|added two-column view, so a reviewer can
//     read the before/after of a change at a glance on a wide pane.
export type DiffViewMode = "unified" | "split"

export type DiffReviewInput = Readonly<{
  files: ReadonlyArray<DiffReviewFile>
  summary: DiffReviewSummary
  // Optional: when the per-file +/- counts were derived from the event tail
  // rather than a full patch body, surface that provenance honestly.
  provenance?: string
  // #5470: group files under a per-directory tree so a large change set (many
  // files across many dirs) stays legible. Display-only; off by default so the
  // existing flat list is unchanged for callers that don't opt in.
  fileTree?: boolean
  // #5470: render each file's hunk body unified (default) or side-by-side.
  viewMode?: DiffViewMode
  // #5470: files whose hunk body should start open. Any file NOT listed (when
  // it has hunk lines) renders collapsed behind a native <summary>, so a large
  // diff opens compact and the reviewer expands the files they care about — no
  // Message needed (native <details>). When omitted, every file with hunks
  // renders open (the prior behavior).
  expandedFiles?: ReadonlyArray<string>
}>

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const statusGlyph = (status: DiffFileStatus): string => {
  switch (status) {
    case "added":
      return "A"
    case "modified":
      return "M"
    case "deleted":
      return "D"
  }
}

const statusTone = (status: DiffFileStatus): "success" | "info" | "danger" => {
  switch (status) {
    case "added":
      return "success"
    case "modified":
      return "info"
    case "deleted":
      return "danger"
  }
}

const hunkLineClass = (kind: DiffHunkLine["kind"]): string => {
  switch (kind) {
    case "added":
      return "text-[#86efac]"
    case "removed":
      return "text-[#fca5a5]"
    case "meta":
      return "text-[var(--text-secondary,#8a8c93)]"
    case "context":
      return "text-[var(--text,#d7d8e5)]"
  }
}

const hunkLinePrefix = (kind: DiffHunkLine["kind"]): string => {
  switch (kind) {
    case "added":
      return "+"
    case "removed":
      return "-"
    case "meta":
      return ""
    case "context":
      return " "
  }
}

const countLabel = (file: DiffReviewFile): string => `+${file.added} −${file.removed}`

// The hunk lines as a single-column unified +/- block (the original layout).
const unifiedHunkBlock = (lines: ReadonlyArray<DiffHunkLine>): Html =>
  h.pre(
    [
      className(
        "m-0 max-h-[18rem] min-w-0 overflow-auto whitespace-pre rounded-[4px] border border-[var(--outline,#525458)] bg-[var(--bg,#0b0b0c)] px-3 py-2 font-mono text-xs leading-relaxed",
      ),
      h.DataAttribute("autopilot-diff-hunks", ""),
    ],
    lines.map((line) =>
      h.span([className(`block ${hunkLineClass(line.kind)}`)], [`${hunkLinePrefix(line.kind)}${line.text}`]),
    ),
  )

// #5470: side-by-side hunk body — removed lines on the left, added on the right,
// context mirrored on both sides. Pairs removed→added runs row-wise so a small
// edit reads as a before/after; unmatched lines fill the empty column with a
// non-breaking placeholder so the grid stays aligned. Display-only.
const splitHunkBlock = (lines: ReadonlyArray<DiffHunkLine>): Html => {
  type Side = { left: DiffHunkLine | null; right: DiffHunkLine | null }
  const rows: Array<Side> = []
  let pendingRemoved: Array<DiffHunkLine> = []
  const flushRemoved = (): void => {
    for (const removed of pendingRemoved) rows.push({ left: removed, right: null })
    pendingRemoved = []
  }
  for (const line of lines) {
    if (line.kind === "removed") {
      pendingRemoved.push(line)
      continue
    }
    if (line.kind === "added") {
      const matched = pendingRemoved.shift()
      if (matched) rows.push({ left: matched, right: line })
      else rows.push({ left: null, right: line })
      continue
    }
    // context / meta: flush any pending removed, then mirror on both sides.
    flushRemoved()
    rows.push({ left: line, right: line })
  }
  flushRemoved()
  const cell = (line: DiffHunkLine | null): Html =>
    h.span(
      [className(`block min-w-0 truncate ${line ? hunkLineClass(line.kind) : ""}`)],
      [line ? `${hunkLinePrefix(line.kind)}${line.text}` : " "],
    )
  return h.div(
    [
      className(
        "grid max-h-[18rem] grid-cols-2 gap-px overflow-auto rounded-[4px] border border-[var(--outline,#525458)] bg-[var(--bg,#0b0b0c)] px-3 py-2 font-mono text-xs leading-relaxed",
      ),
      h.DataAttribute("autopilot-diff-hunks", ""),
      h.DataAttribute("autopilot-diff-hunks-split", ""),
    ],
    rows.flatMap((row) => [cell(row.left), cell(row.right)]),
  )
}

const hunkBlock = (lines: ReadonlyArray<DiffHunkLine>, mode: DiffViewMode): Html =>
  mode === "split" ? splitHunkBlock(lines) : unifiedHunkBlock(lines)

const fileHeader = (file: DiffReviewFile): Html =>
  h.div([className("flex flex-wrap items-center gap-2")], [
    statusChip({
      label: statusGlyph(file.status),
      tone: statusTone(file.status),
      attrs: [h.DataAttribute("autopilot-diff-file-status", file.status)],
    }),
    h.code(
      [className("min-w-0 grow truncate font-mono text-sm text-[var(--primary,#fff)]"), h.Title(file.path)],
      [file.path],
    ),
    h.span(
      [
        className("shrink-0 font-mono text-xs text-[var(--text-secondary,#8a8c93)]"),
        h.DataAttribute("autopilot-diff-file-counts", countLabel(file)),
      ],
      [countLabel(file)],
    ),
  ])

// One file row. When `expandedFiles` is provided and this file is NOT in it, its
// hunk body collapses behind a native <details>/<summary> so a big diff opens
// compact; otherwise the body renders inline (the prior behavior).
const fileRow = (
  file: DiffReviewFile,
  mode: DiffViewMode,
  expandedFiles: ReadonlyArray<string> | undefined,
): Html => {
  const hasHunks = file.hunkLines != null && file.hunkLines.length > 0
  const collapsible = hasHunks && expandedFiles != null && !expandedFiles.includes(file.path)
  const containerAttrs: Array<Attribute<AutopilotUiMessage>> = [
    className(
      "grid gap-2 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-3 text-[var(--text,#d7d8e5)]",
    ),
    h.DataAttribute("autopilot-diff-file", file.path),
    h.DataAttribute("autopilot-diff-status", file.status),
  ]
  if (!hasHunks) {
    return h.article(containerAttrs, [fileHeader(file)])
  }
  const body = hunkBlock(file.hunkLines as ReadonlyArray<DiffHunkLine>, mode)
  if (collapsible) {
    return h.article(containerAttrs, [
      h.details(
        [h.DataAttribute("autopilot-diff-file-details", "")],
        [
          h.summary([className("cursor-pointer list-none")], [fileHeader(file)]),
          h.div([className("mt-2")], [body]),
        ],
      ),
    ])
  }
  return h.article(containerAttrs, [fileHeader(file), body])
}

// #5470: split a path into its directory ("." for a root-level file) and base.
const dirOf = (path: string): string => {
  const idx = path.lastIndexOf("/")
  return idx <= 0 ? "." : path.slice(0, idx)
}

// #5470: group files under a per-directory tree. Directories are sorted; each is
// a native <details> (open) so a large change set stays navigable without any
// Message wiring. Display-only.
const fileTreeView = (
  files: ReadonlyArray<DiffReviewFile>,
  mode: DiffViewMode,
  expandedFiles: ReadonlyArray<string> | undefined,
): Html => {
  const byDir = new Map<string, Array<DiffReviewFile>>()
  for (const file of files) {
    const dir = dirOf(file.path)
    const bucket = byDir.get(dir)
    if (bucket) bucket.push(file)
    else byDir.set(dir, [file])
  }
  const dirs = [...byDir.keys()].sort((a, b) => a.localeCompare(b))
  return h.div(
    [className("grid gap-2"), h.DataAttribute("autopilot-diff-tree", "")],
    dirs.map((dir) => {
      const dirFiles = byDir.get(dir) ?? []
      return h.details(
        [
          className("grid gap-2"),
          h.Open(true),
          h.DataAttribute("autopilot-diff-tree-dir", dir),
        ],
        [
          h.summary(
            [className("cursor-pointer list-none font-mono text-xs text-[var(--text-secondary,#8a8c93)]")],
            [`${dir === "." ? "(root)" : dir}/ · ${dirFiles.length} file${dirFiles.length === 1 ? "" : "s"}`],
          ),
          h.div([className("grid gap-2 pl-2")], dirFiles.map((file) => fileRow(file, mode, expandedFiles))),
        ],
      )
    }),
  )
}

export const DiffReview = (input: DiffReviewInput): Html => {
  const mode: DiffViewMode = input.viewMode ?? "unified"
  const body =
    input.files.length === 0
      ? h.p([className("m-0 text-sm text-[var(--text-secondary,#8a8c93)]")], ["No file changes in this turn"])
      : input.fileTree
        ? fileTreeView(input.files, mode, input.expandedFiles)
        : h.div(
            [className("grid gap-2")],
            input.files.map((file) => fileRow(file, mode, input.expandedFiles)),
          )
  return h.section(
    [
      className("grid gap-3"),
      h.DataAttribute("autopilot-diff-review", ""),
      h.DataAttribute("autopilot-diff-file-count", String(input.summary.fileCount)),
      h.DataAttribute("autopilot-diff-view-mode", mode),
    ],
    [
      h.div([className("flex flex-wrap items-center justify-between gap-2")], [
        h.h3([className("m-0 font-mono text-sm font-bold text-[var(--primary,#fff)]")], ["diff"]),
        statusChip({
          label: `${input.summary.fileCount} file${input.summary.fileCount === 1 ? "" : "s"} · +${
            input.summary.totalAdded
          } −${input.summary.totalRemoved}`,
          tone: "neutral",
          attrs: [h.DataAttribute("autopilot-diff-summary", "")],
        }),
      ]),
      body,
      input.provenance && input.provenance.length > 0
        ? h.p(
            [
              className("m-0 text-xs text-[var(--text-secondary,#8a8c93)]"),
              h.DataAttribute("autopilot-diff-provenance", input.provenance),
            ],
            [input.provenance],
          )
        : h.empty,
    ],
  )
}
