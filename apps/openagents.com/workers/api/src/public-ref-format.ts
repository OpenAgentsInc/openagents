export const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

export const publicRefSegment = (
  value: string,
  fallback: string,
): string =>
  value
    .trim()
    .replaceAll(/[^A-Za-z0-9_.-]+/g, '_')
    .replaceAll(/_{2,}/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 120) || fallback
