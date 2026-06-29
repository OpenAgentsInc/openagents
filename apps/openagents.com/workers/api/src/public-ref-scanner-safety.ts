export type PublicScannerUnsafeReason = 'jwt_shaped' | 'long_base64url_shaped'

export type PublicScannerUnsafeFinding = Readonly<{
  length: number
  path: string
  preview: string
  reason: PublicScannerUnsafeReason
}>

const jwtShapedPattern =
  /eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/
const longBase64UrlShapedPattern = /^[A-Za-z0-9_-]{40,}$/

const unsafeScannerReason = (
  value: string,
): PublicScannerUnsafeReason | null => {
  if (jwtShapedPattern.test(value)) {
    return 'jwt_shaped'
  }

  return longBase64UrlShapedPattern.test(value) ? 'long_base64url_shaped' : null
}

export const publicRefTriggersAgentSecretScanner = (value: string): boolean =>
  unsafeScannerReason(value.trim()) !== null

const publicRefAliasHash = (value: string): string =>
  Array.from(value)
    .reduce(
      (hash, char) => Math.imul(hash ^ char.charCodeAt(0), 16_777_619) >>> 0,
      0x811c9dc5,
    )
    .toString(16)
    .padStart(8, '0')

export const publicScannerSafeRef = (scope: string, value: string): string => {
  const normalized = value.trim()

  return publicRefTriggersAgentSecretScanner(normalized)
    ? `${scope}.scanner_safe.${publicRefAliasHash(normalized)}`
    : normalized
}

export const publicScannerSafeRefs = (
  scope: string,
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set(
      (refs ?? [])
        .map(ref => publicScannerSafeRef(scope, ref))
        .filter(ref => ref !== ''),
    ),
  ].sort()

const previewScannerUnsafeValue = (value: string): string =>
  value.length <= 16
    ? value
    : `${value.slice(0, 8)}...${value.slice(value.length - 4)}`

export const findPublicScannerUnsafeStrings = (
  value: unknown,
  path = '$',
): ReadonlyArray<PublicScannerUnsafeFinding> => {
  if (typeof value === 'string') {
    const reason = unsafeScannerReason(value.trim())

    return reason === null
      ? []
      : [
          {
            length: value.length,
            path,
            preview: previewScannerUnsafeValue(value),
            reason,
          },
        ]
  }

  if (Array.isArray(value)) {
    return value.flatMap((item, index) =>
      findPublicScannerUnsafeStrings(item, `${path}[${index}]`),
    )
  }

  if (value !== null && typeof value === 'object') {
    return Object.entries(value).flatMap(([key, item]) =>
      findPublicScannerUnsafeStrings(item, `${path}.${key}`),
    )
  }

  return []
}
