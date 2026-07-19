import { Schema } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  IDE_CURSOR_QUALITY_CORPUS,
  IdeCursorQualityCorpusSchema,
} from "./cursor-quality-corpus.ts"

describe("IDE-09 pinned quality corpus", () => {
  test("is schema-valid, license-safe, multilingual, and fault-complete", () => {
    const decoded = Schema.decodeUnknownSync(IdeCursorQualityCorpusSchema)(IDE_CURSOR_QUALITY_CORPUS)
    expect(new Set(decoded.cases.map(row => row.language)).size).toBeGreaterThanOrEqual(7)
    expect(new Set(decoded.cases.map(row => row.intent)).size).toBe(5)
    expect(new Set(decoded.cases.map(row => row.qualityClass)).size).toBeGreaterThanOrEqual(20)
    expect(decoded.cases.every(row => row.license === "CC0-1.0")).toBe(true)
  })
})
