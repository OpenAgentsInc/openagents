export type ResumeCheckpoint<Ref = string> = {
  readonly seq: number
  readonly ref: Ref
}

export type ResumeResolution<Ref = string> = {
  readonly resumedRef: Ref
  readonly truncatedRefs: readonly Ref[]
}

function validateCheckpoints<Ref>(checkpoints: readonly ResumeCheckpoint<Ref>[]): void {
  if (checkpoints.length === 0) {
    throw new Error("resume checkpoints must not be empty")
  }

  let previousSeq = 0
  for (const checkpoint of checkpoints) {
    if (!Number.isInteger(checkpoint.seq) || checkpoint.seq <= 0) {
      throw new Error("resume checkpoint seq must be a positive integer")
    }
    if (checkpoint.seq <= previousSeq) {
      throw new Error("resume checkpoint seq values must be strictly increasing")
    }
    previousSeq = checkpoint.seq
  }
}

export function resolveResume<Ref>(
  checkpoints: readonly ResumeCheckpoint<Ref>[],
  targetSeq?: number,
): ResumeResolution<Ref> {
  validateCheckpoints(checkpoints)

  if (targetSeq === undefined) {
    return {
      resumedRef: checkpoints[checkpoints.length - 1]!.ref,
      truncatedRefs: [],
    }
  }

  if (!Number.isInteger(targetSeq) || targetSeq <= 0) {
    throw new Error("resume target seq must be a positive integer")
  }

  const targetIndex = checkpoints.findIndex((checkpoint) => checkpoint.seq === targetSeq)
  if (targetIndex === -1) {
    throw new Error("resume target seq does not match a checkpoint")
  }

  return {
    resumedRef: checkpoints[targetIndex]!.ref,
    truncatedRefs: checkpoints.slice(targetIndex + 1).map((checkpoint) => checkpoint.ref),
  }
}
