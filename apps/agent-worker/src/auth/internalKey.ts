export function extractInternalKey(headers: Headers): string | null {
  const header = headers.get('x-oa-internal-key');
  if (!header) return null;
  const token = header.trim();
  return token.length > 0 ? token : null;
}

export function isInternalKeyValid(headers: Headers, expected: string | undefined): boolean {
  if (!expected) return false;
  const token = extractInternalKey(headers);
  return !!token && token === expected;
}
