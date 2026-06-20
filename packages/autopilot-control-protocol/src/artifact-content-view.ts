// G3 (EPIC #5492 / #5495): the readable content projection of a session's
// retained artifact, fetched over the bridge `artifact.read` verb (read_artifact
// capability). The node returns `{ sessionRef, kind: "proof"|"failure"|"none",
// artifact: <projection-safe JSON> }`. That JSON is redaction-scanned at write
// time on the node, so it carries refs + counts + a change/dev-check summary
// rather than raw diff bytes or secrets.
//
// `projectArtifactContentView` turns that loose JSON into typed, render-ready
// sections so a read-only mobile/desktop viewer can show "what the agent
// actually did" — the changed-file list (the projection-safe diff view), the
// dev-check command results (the verify transcript), deviations, and a verbatim
// pretty-printed fallback — without each client re-deriving the shape. Pure +
// transport-agnostic so web / desktop / mobile share it. Tolerant of both the
// proof and failure artifact shapes (see apps/pylon control-sessions
// writeRetainedArtifact / writeFailureArtifact).

import { projectArtifactReview, type ArtifactReviewView } from "./artifact-review-view.js"

export type ArtifactReadKind = "proof" | "failure" | "none"

// The raw bridge/dev-token response envelope for artifact.read /
// session.artifact. `artifact` stays `unknown` because the proof and failure
// bodies differ and are intentionally loose/projection-safe on the node.
export type ArtifactReadResponse = {
  sessionRef: string
  kind: ArtifactReadKind
  artifact: unknown
}

// Parse a raw artifact.read / session.artifact result into the typed envelope.
// Defensive: a missing/garbled body degrades to kind "none" rather than throwing,
// so a read-only viewer never hard-crashes on an unexpected node shape.
export function parseArtifactReadResponse(raw: unknown): ArtifactReadResponse {
  if (!isRecord(raw)) return { sessionRef: "", kind: "none", artifact: null }
  const kind =
    raw.kind === "proof" || raw.kind === "failure" ? raw.kind : "none"
  return {
    sessionRef: typeof raw.sessionRef === "string" ? raw.sessionRef : "",
    kind,
    artifact: raw.artifact ?? null,
  }
}

export type ChangedFileRow = {
  fileRef: string
  status: string
  area: string | null
  extension: string | null
}

export type CommandResultRow = {
  commandRef: string
  reasonRef: string | null
  status: string
  exitCode: number | null
  durationMs: number | null
  stdoutBytes: number | null
  stderrBytes: number | null
}

// The render-ready artifact content view. `kind` drives the top-level framing;
// `review` carries the shared stat projection; the section arrays are the
// readable diff/transcript surfaces; `body` is the verbatim pretty-printed JSON
// fallback so nothing the node sent is ever hidden from a read-only operator.
export type ArtifactContentView = {
  kind: ArtifactReadKind
  present: boolean
  schemaRef: string | null
  outcome: string | null
  // Projection-safe "diff": the changed-file list with per-file status. The node
  // never ships raw diff bytes; this is the file-level change set it does ship.
  changedFiles: ChangedFileRow[]
  dirtySummary: string | null
  // The verify transcript: per-command pass/fail with exit codes + byte sizes.
  commandResults: CommandResultRow[]
  devCheckState: string | null
  deviations: string[]
  errorClass: string | null
  errorDigestRef: string | null
  // Shared stat projection (outcome + counts) so the viewer can keep the header
  // line consistent with the session-detail stats row.
  review: ArtifactReviewView
  // Verbatim pretty-printed artifact JSON — the read-only "text view" fallback.
  body: string
}

export function projectArtifactContentView(input: {
  kind?: ArtifactReadKind
  artifact: unknown
}): ArtifactContentView {
  const kind: ArtifactReadKind =
    input.kind === "proof" || input.kind === "failure" || input.kind === "none"
      ? input.kind
      : input.artifact == null
        ? "none"
        : "proof"
  const artifact = input.artifact
  const present = kind !== "none" && artifact != null
  const records = isRecord(artifact) ? artifact : {}
  const review = projectArtifactReview(artifact)

  const changeSummary = resolveChangeSummary(records)

  return {
    kind,
    present,
    schemaRef: stringOrNull(records.schema),
    outcome: review.outcome,
    changedFiles: changedFilesFrom(changeSummary),
    dirtySummary: dirtySummaryFrom(changeSummary),
    commandResults: commandResultsFrom(records),
    devCheckState: review.devCheckState,
    deviations: review.deviations,
    errorClass: stringOrNull(records.errorClass),
    errorDigestRef: stringOrNull(records.errorDigestRef),
    review,
    body: present ? prettyPrint(artifact) : "",
  }
}

function resolveChangeSummary(records: Record<string, unknown>): Record<string, unknown> {
  const devCheck = isRecord(records.devCheck) ? records.devCheck : undefined
  const direct = isRecord(records.changeSummary) ? records.changeSummary : undefined
  const nested = devCheck && isRecord(devCheck.changeSummary) ? devCheck.changeSummary : undefined
  return direct ?? nested ?? {}
}

function changedFilesFrom(changeSummary: Record<string, unknown>): ChangedFileRow[] {
  const refs = changeSummary.changedFileRefs
  if (!Array.isArray(refs)) return []
  return refs.flatMap((entry) => {
    if (typeof entry === "string") {
      return entry.trim() === "" ? [] : [{ fileRef: entry.trim(), status: "unknown", area: null, extension: null }]
    }
    if (!isRecord(entry)) return []
    const fileRef = stringOrNull(entry.fileRef ?? entry.path ?? entry.ref)
    if (fileRef === null) return []
    return [
      {
        fileRef,
        status: stringOrNull(entry.status) ?? "unknown",
        area: stringOrNull(entry.area),
        extension: stringOrNull(entry.extension),
      },
    ]
  })
}

function dirtySummaryFrom(changeSummary: Record<string, unknown>): string | null {
  const dirty = isRecord(changeSummary.dirty) ? changeSummary.dirty : undefined
  if (dirty === undefined) return null
  const state = stringOrNull(dirty.state)
  const changed = numberOrNull(dirty.changedCount)
  const untracked = numberOrNull(dirty.untrackedCount)
  const parts: string[] = []
  if (state !== null) parts.push(state)
  if (changed !== null) parts.push(`${changed} changed`)
  if (untracked !== null && untracked > 0) parts.push(`${untracked} untracked`)
  return parts.length === 0 ? null : parts.join(" · ")
}

function commandResultsFrom(records: Record<string, unknown>): CommandResultRow[] {
  const devCheck = isRecord(records.devCheck) ? records.devCheck : undefined
  const source = Array.isArray(records.commandResults)
    ? records.commandResults
    : devCheck && Array.isArray(devCheck.commandResults)
      ? devCheck.commandResults
      : []
  return source.flatMap((entry) => {
    if (!isRecord(entry)) return []
    const commandRef = stringOrNull(entry.commandRef ?? entry.argvRef ?? entry.reasonRef)
    if (commandRef === null) return []
    return [
      {
        commandRef,
        reasonRef: stringOrNull(entry.reasonRef),
        status: stringOrNull(entry.status) ?? "unknown",
        exitCode: numberOrNull(entry.exitCode),
        durationMs: numberOrNull(entry.durationMs),
        stdoutBytes: numberOrNull(entry.stdoutBytes),
        stderrBytes: numberOrNull(entry.stderrBytes),
      },
    ]
  })
}

function prettyPrint(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function stringOrNull(value: unknown): string | null {
  return typeof value === "string" && value.trim() !== "" ? value.trim() : null
}

function numberOrNull(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null
}
