import { describe, expect, test } from "bun:test"
import {
  buildEvidenceReceipt,
  decodeEvidenceReceipt,
  type EvidenceReceiptKind,
} from "../src/tas/evidence-receipt"

const digestRef = `sha256:${"a".repeat(64)}`

function receipt(kind: EvidenceReceiptKind) {
  return {
    receiptKind: kind,
    subjectRef: `${kind}.fixture.0001`,
    digestRef,
    producedAt: "2026-06-13T12:00:00.000Z",
    status: "produced" as const,
  }
}

describe("evidence receipt", () => {
  test("decodes a refs-only evidence receipt", () => {
    expect(decodeEvidenceReceipt(receipt("task"))).toEqual(receipt("task"))
  })

  test("rejects invalid receipt data", () => {
    expect(() =>
      decodeEvidenceReceipt({
        ...receipt("schedule"),
        digestRef: "raw digest",
      }),
    ).toThrow("sha256")

    expect(() =>
      decodeEvidenceReceipt({
        ...receipt("decision"),
        receiptKind: "payment",
      }),
    ).toThrow()
  })

  test("builder strips raw payload fields and keeps only receipt refs", () => {
    const built = buildEvidenceReceipt({
      ...receipt("notification"),
      rawPayload: "private terminal output",
      prompt: "ship the private worktree",
      log: "/Users/example/private.log",
    })

    expect(built).toEqual(receipt("notification"))
    expect(Object.keys(built).sort()).toEqual([
      "digestRef",
      "producedAt",
      "receiptKind",
      "status",
      "subjectRef",
    ])
    expect(JSON.stringify(built)).not.toContain("private terminal output")
    expect(JSON.stringify(built)).not.toContain("ship the private worktree")
    expect(JSON.stringify(built)).not.toContain("/Users/example/private.log")
  })

  test("decoder rejects raw payload fields instead of projecting them", () => {
    expect(() =>
      decodeEvidenceReceipt({
        ...receipt("review"),
        rawPayload: "private review text",
      }),
    ).toThrow("rawPayload")
  })

  test("supports every terminal-agent receipt kind", () => {
    const kinds: EvidenceReceiptKind[] = [
      "schedule",
      "task",
      "decision",
      "notification",
      "review",
      "smoke",
    ]

    expect(kinds.map((kind) => buildEvidenceReceipt(receipt(kind)).receiptKind)).toEqual(kinds)
  })
})
