export type ExpoProtocolVersionNegotiation = {
  version: 0 | 1
  supportsDirectives: boolean
}

export function negotiateProtocolVersion(header: string | null): ExpoProtocolVersionNegotiation {
  if (header?.trim() === "1") {
    return { version: 1, supportsDirectives: true }
  }

  return { version: 0, supportsDirectives: false }
}
