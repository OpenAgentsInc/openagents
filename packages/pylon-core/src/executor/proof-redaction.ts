const PROOF_REDACTION_PATTERNS: Array<{ ref: string; pattern: RegExp }> = [
  { ref: "redaction.local_user_path", pattern: /\/Users\// },
  { ref: "redaction.local_home_path", pattern: /\/home\// },
  { ref: "redaction.tmp_path", pattern: /\/(?:private\/)?tmp\// },
  // Pattern ref names deliberately avoid containing the matched text, so a
  // retained artifact's own patternRefs list can never trip the scan.
  { ref: "redaction.sk_prefix", pattern: /\bsk-[A-Za-z0-9_-]{8,}/ },
  { ref: "redaction.provider_key_name", pattern: /api[_-]?key/i },
  { ref: "redaction.auth_scheme", pattern: /\bbearer\b/i },
  { ref: "redaction.credential_file", pattern: /auth\.json|\.credentials/i },
  {
    ref: "redaction.raw_session_uuid",
    pattern: /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i,
  },
]

export const PROOF_REDACTION_PATTERN_REFS = PROOF_REDACTION_PATTERNS.map(({ ref }) => ref)

export function scanProofSerialization(serialized: string): string[] {
  return PROOF_REDACTION_PATTERNS.filter(({ pattern }) => pattern.test(serialized)).map(
    ({ ref }) => ref,
  )
}
