import { describe, expect, test } from "vite-plus/test"

import {
  PROOF_REDACTION_PATTERN_REFS,
  scanProofSerialization,
} from "./proof-redaction.js"

describe("scanProofSerialization", () => {
  test("returns empty for clean payload", () => {
    expect(scanProofSerialization(JSON.stringify({ ok: true, score: 1 }))).toEqual(
      [],
    )
  })

  test("flags local user paths", () => {
    expect(scanProofSerialization("/Users/chris/secret")).toContain(
      "redaction.local_user_path",
    )
  })

  test("flags sk- prefixes and auth schemes", () => {
    const hits = scanProofSerialization("Authorization: Bearer sk-abcdefghij")
    expect(hits).toContain("redaction.sk_prefix")
    expect(hits).toContain("redaction.auth_scheme")
  })

  test("exports pattern refs that do not self-trigger on their own names", () => {
    const serialized = JSON.stringify({ patternRefs: PROOF_REDACTION_PATTERN_REFS })
    expect(scanProofSerialization(serialized)).toEqual([])
  })
})
