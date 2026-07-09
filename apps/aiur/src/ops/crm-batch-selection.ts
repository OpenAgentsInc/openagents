/**
 * Pure selection helpers for the OB-4 CRM batch approval panel.
 * Kept free of React so unit tests do not need a DOM.
 */

export const toggleId = (
  selected: ReadonlySet<string>,
  id: string,
): Set<string> => {
  const next = new Set(selected)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  return next
}

export const selectAllIds = (ids: ReadonlyArray<string>): Set<string> =>
  new Set(ids)

export const clearSelection = (): Set<string> => new Set()

export const selectedCount = (selected: ReadonlySet<string>): number =>
  selected.size

export const summarizeApproveResult = (
  result: Readonly<{
    batchRef: string
    requestedCount: number
    executedCount: number
    failedCount: number
    notPendingCount: number
    notFoundCount: number
    cappedCount: number
  }>,
): string =>
  `Batch ${result.batchRef}: ${result.executedCount}/${result.requestedCount} executed` +
  (result.failedCount > 0 ? `, ${result.failedCount} failed` : '') +
  (result.notPendingCount > 0 ? `, ${result.notPendingCount} not pending` : '') +
  (result.notFoundCount > 0 ? `, ${result.notFoundCount} not found` : '') +
  (result.cappedCount > 0 ? `, ${result.cappedCount} capped (daily send limit)` : '') +
  '.'

export const commandSubjectLine = (
  payload: Readonly<Record<string, unknown>>,
): string => {
  const template =
    typeof payload.templateSlug === 'string' ? payload.templateSlug : '—'
  const channel =
    typeof payload.channel === 'string' ? payload.channel : 'unknown'
  return `${template} via ${channel}`
}
