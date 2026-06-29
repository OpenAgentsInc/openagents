// VCODE-10 (#5927): public-safe projection for the Diff/Artifacts pane.
//
// This helper keeps path/log/ref safety decisions out of the view. It composes
// the existing bounded event parser and retained artifact browser rows, then
// preserves operator UI state such as selected/expanded files across polls.

import type {
  SessionArtifactStats,
  SessionEventRow,
} from "../shared/rpc.js"
import {
  artifactBrowserSections,
  diffReviewProvenance,
  parseChangeSetFromEvents,
  publicSafeFileRef,
  type ArtifactBrowserSection,
  type DesktopChangeSet,
} from "./helpers.js"

export type DiffArtifactsProjection = {
  readonly sessionRef: string
  readonly scrollKey: string
  readonly changeSet: DesktopChangeSet
  readonly patchSummary: string
  readonly provenance: string
  readonly selectedFilePath: string | null
  readonly expandedFiles: readonly string[]
  readonly artifactSections: readonly ArtifactBrowserSection[]
  readonly checkRefs: readonly string[]
  readonly receiptRefs: readonly string[]
  readonly screenshotRefs: readonly string[]
  readonly proofLinks: readonly string[]
  readonly hasContent: boolean
}

const unsafeArtifactValue = (value: string): boolean => {
  const trimmed = value.trim()
  const lower = trimmed.toLowerCase()
  return (
    trimmed.startsWith("/") ||
    trimmed.startsWith("~/") ||
    trimmed.startsWith("../") ||
    trimmed.includes("\\") ||
    /^[A-Za-z]:[\\/]/.test(trimmed) ||
    lower.includes("begin private key") ||
    lower.includes("mnemonic") ||
    lower.includes("secret") ||
    lower.includes("token=") ||
    lower.includes("sk-")
  )
}

const safeArtifactSections = (
  stats: SessionArtifactStats | undefined | null,
): readonly ArtifactBrowserSection[] =>
  artifactBrowserSections(stats)
    .map((section) => ({
      ...section,
      rows: section.rows.filter((row) => !unsafeArtifactValue(row.value)),
    }))
    .filter((section) => section.rows.length > 0)

const unique = (values: ReadonlyArray<string>): readonly string[] => [
  ...new Set(values.filter((value) => value.trim().length > 0)),
]

const rowsByLabel = (
  sections: readonly ArtifactBrowserSection[],
  predicate: (label: string, sectionId: string) => boolean,
): readonly string[] =>
  unique(
    sections.flatMap((section) =>
      section.rows
        .filter((row) => predicate(row.label.toLowerCase(), section.id.toLowerCase()))
        .map((row) => row.value),
    ),
  )

export const projectDiffArtifactsPanel = (input: {
  readonly sessionRef: string
  readonly events: readonly SessionEventRow[]
  readonly stats: SessionArtifactStats | undefined | null
  readonly expandedFiles: readonly string[]
  readonly selectedFilePath: string | null
}): DiffArtifactsProjection => {
  const changeSet = parseChangeSetFromEvents(input.events)
  const filePathSet = new Set(changeSet.files.map((file) => file.path))
  const selectedPublicRef =
    input.selectedFilePath === null ? null : publicSafeFileRef(input.selectedFilePath)
  const selectedFilePath =
    selectedPublicRef !== null && filePathSet.has(selectedPublicRef)
      ? selectedPublicRef
      : changeSet.files[0]?.path ?? null
  const expandedFiles = unique(
    input.expandedFiles
      .map((path) => publicSafeFileRef(path))
      .filter((path): path is string => path !== null && filePathSet.has(path)),
  )
  const artifactSections = safeArtifactSections(input.stats)
  const patchSummary = `${changeSet.summary.fileCount} file${
    changeSet.summary.fileCount === 1 ? "" : "s"
  } · +${changeSet.summary.totalAdded} -${changeSet.summary.totalRemoved}`

  return {
    sessionRef: input.sessionRef,
    scrollKey: `diff-artifacts:${input.sessionRef}`,
    changeSet,
    patchSummary,
    provenance: diffReviewProvenance(changeSet, input.stats?.editedFileCount),
    selectedFilePath,
    expandedFiles,
    artifactSections,
    checkRefs: rowsByLabel(
      artifactSections,
      (label) => label.includes("verify") || label.includes("dev-check") || label.includes("redaction"),
    ),
    receiptRefs: rowsByLabel(artifactSections, (_label, sectionId) => sectionId === "receipts"),
    screenshotRefs: rowsByLabel(
      artifactSections,
      (label) => label.includes("screenshot") || label.includes("image"),
    ),
    proofLinks: rowsByLabel(
      artifactSections,
      (label, sectionId) =>
        sectionId === "proof" ||
        label.includes("objective") ||
        label.includes("response") ||
        label.includes("external session"),
    ),
    hasContent: changeSet.files.length > 0 || artifactSections.length > 0,
  }
}
