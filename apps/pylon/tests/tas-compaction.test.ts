import { describe, expect, test } from "bun:test"

import {
  buildCompactionSummaryRecord,
  decideCompaction,
} from "../src/tas/compaction"

describe("tas compaction decision core", () => {
  test("keeps context under the automatic compaction threshold", () => {
    expect(
      decideCompaction({
        usedTokens: 799,
        maxTokens: 1_000,
        keepTailCount: 12,
      }),
    ).toEqual({
      action: "keep",
      reason: "under_token_threshold",
    })
  })

  test("compacts context over the automatic compaction threshold", () => {
    expect(
      decideCompaction({
        usedTokens: 801,
        maxTokens: 1_000,
        keepTailCount: 12,
      }),
    ).toEqual({
      action: "compact",
      reason: "token_threshold_exceeded",
    })
  })

  test("summary record is refs-only and lists replaced refs", () => {
    const record = buildCompactionSummaryRecord({
      replacedRefs: [
        "message.fixture.user.1",
        "message.fixture.assistant.2",
        "tool.fixture.result.3",
      ],
      summaryRef: "summary.fixture.compaction.1",
    })

    expect(record).toEqual({
      kind: "compaction_summary_record",
      summaryRef: "summary.fixture.compaction.1",
      replacedRefs: [
        "message.fixture.user.1",
        "message.fixture.assistant.2",
        "tool.fixture.result.3",
      ],
    })
    expect(JSON.stringify(record)).not.toContain("raw transcript")
    expect(JSON.stringify(record)).not.toContain("user said")
    expect(Object.keys(record).sort()).toEqual([
      "kind",
      "replacedRefs",
      "summaryRef",
    ])
  })
})
