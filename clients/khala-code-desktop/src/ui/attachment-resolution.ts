import { settleFanout } from "./isolated-fanout"

/**
 * Shared per-attachment-isolated resolution helper for the composer's
 * "read every staged attachment's bytes before submitting the turn" step.
 *
 * `imageAttachmentsForSubmit` previously ran a bare
 * `Promise.all(attachments.map(async attachment => { ... await file.arrayBuffer() ... }))`
 * before `submitComposer`'s own try/catch even started. One stale/revoked
 * attachment URL throwing during `arrayBuffer()` rejected the whole batch,
 * discarding every other attachment that had already resolved successfully
 * and leaving `submitComposer()` (invoked as a bare `void submitComposer()`
 * at one call site) to reject with zero user feedback.
 *
 * `resolveAttachments` isolates each attachment's resolution so one
 * stale/revoked attachment never prevents the other valid attachments from
 * being submitted, and reports which ones (by name) failed so the caller can
 * surface a scoped message instead of losing the whole submission silently.
 */

export type AttachmentResolutionFailure = {
  readonly name: string
  readonly error: string
}

export type AttachmentResolutionResult<R> = {
  readonly resolved: readonly R[]
  readonly failures: readonly AttachmentResolutionFailure[]
}

export const resolveAttachments = async <T, R>(
  items: readonly T[],
  resolve: (item: T) => Promise<R | null>,
  nameFor: (item: T) => string,
): Promise<AttachmentResolutionResult<R>> => {
  const settled = await settleFanout(items, resolve)
  const resolved: R[] = []
  const failures: AttachmentResolutionFailure[] = []
  for (const outcome of settled) {
    if (!outcome.ok) {
      failures.push({ name: nameFor(outcome.item), error: outcome.error })
      continue
    }
    if (outcome.value !== null) resolved.push(outcome.value)
  }
  return { resolved, failures }
}
