export type ChangeFileStatus = "added" | "modified" | "deleted"

export type ChangeSetFile = {
  path: string
  added: number
  removed: number
  status: ChangeFileStatus
}

export type ChangeSet = {
  files: ChangeSetFile[]
}

export type ChangeSetSummary = {
  fileCount: number
  totalAdded: number
  totalRemoved: number
}

export type DiffReviewArtifactFile = ChangeSetFile

export type DiffReviewArtifact = {
  artifactKind: "diff_review"
  digestRef: string
  summary: ChangeSetSummary
  files: DiffReviewArtifactFile[]
}

export function summarizeChangeSet(changeSet: ChangeSet): ChangeSetSummary {
  return changeSet.files.reduce(
    (summary, file) => ({
      fileCount: summary.fileCount + 1,
      totalAdded: summary.totalAdded + file.added,
      totalRemoved: summary.totalRemoved + file.removed,
    }),
    {
      fileCount: 0,
      totalAdded: 0,
      totalRemoved: 0,
    },
  )
}

export function buildDiffReviewArtifact(
  changeSet: ChangeSet,
  options: { digestRef: string },
): DiffReviewArtifact {
  return {
    artifactKind: "diff_review",
    digestRef: options.digestRef,
    summary: summarizeChangeSet(changeSet),
    files: changeSet.files.map((file) => ({
      path: file.path,
      added: file.added,
      removed: file.removed,
      status: file.status,
    })),
  }
}
