import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage } from "./view"
import { statusChip } from "./view"

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

export type DiffReviewInput = Readonly<{
  files: ReadonlyArray<DiffReviewFile>
  summary: DiffReviewSummary
  // Optional: when the per-file +/- counts were derived from the event tail
  // rather than a full patch body, surface that provenance honestly.
  provenance?: string
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

const fileRow = (file: DiffReviewFile): Html =>
  h.article(
    [
      className(
        "grid gap-2 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-3 text-[var(--text,#d7d8e5)]",
      ),
      h.DataAttribute("autopilot-diff-file", file.path),
      h.DataAttribute("autopilot-diff-status", file.status),
    ],
    [
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
      ]),
      file.hunkLines && file.hunkLines.length > 0
        ? h.pre(
            [
              className(
                "m-0 max-h-[18rem] min-w-0 overflow-auto whitespace-pre rounded-[4px] border border-[var(--outline,#525458)] bg-[var(--bg,#0b0b0c)] px-3 py-2 font-mono text-xs leading-relaxed",
              ),
              h.DataAttribute("autopilot-diff-hunks", ""),
            ],
            file.hunkLines.map((line) =>
              h.span(
                [className(`block ${hunkLineClass(line.kind)}`)],
                [`${hunkLinePrefix(line.kind)}${line.text}`],
              ),
            ),
          )
        : h.empty,
    ],
  )

export const DiffReview = (input: DiffReviewInput): Html =>
  h.section(
    [
      className("grid gap-3"),
      h.DataAttribute("autopilot-diff-review", ""),
      h.DataAttribute("autopilot-diff-file-count", String(input.summary.fileCount)),
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
      input.files.length === 0
        ? h.p([className("m-0 text-sm text-[var(--text-secondary,#8a8c93)]")], ["No file changes in this turn"])
        : h.div([className("grid gap-2")], input.files.map(fileRow)),
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
