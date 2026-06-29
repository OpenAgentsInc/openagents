import { describe, expect, test } from "bun:test"

import { indexReceipts } from "../src/coordinator/receipts-ledger"

describe("receipts ledger", () => {
  test("indexes receipts by total and schema", () => {
    expect(
      indexReceipts([
        {
          schema: "openagents.receipt.ship",
          observedAt: "2026-06-13T14:00:00.000Z",
        },
        {
          schema: "openagents.receipt.ship",
          observedAt: "2026-06-13T14:05:00.000Z",
        },
        {
          schema: "openagents.receipt.build",
          observedAt: "2026-06-13T14:10:00.000Z",
        },
      ]),
    ).toMatchObject({
      total: 3,
      bySchema: {
        "openagents.receipt.ship": 2,
        "openagents.receipt.build": 1,
      },
    })
  })

  test("sorts receipts by observedAt descending", () => {
    const ledger = indexReceipts([
      {
        schema: "openagents.receipt.ship",
        observedAt: "2026-06-13T14:00:00.000Z",
        id: "first",
      },
      {
        schema: "openagents.receipt.ship",
        observedAt: "2026-06-13T14:15:00.000Z",
        id: "latest",
      },
      {
        schema: "openagents.receipt.ship",
        observedAt: "2026-06-13T14:05:00.000Z",
        id: "middle",
      },
    ])

    expect(ledger.sorted.map((receipt) => receipt.id)).toEqual([
      "latest",
      "middle",
      "first",
    ])
  })

  test("projects the latest receipt identity", () => {
    expect(
      indexReceipts([
        {
          schema: "openagents.receipt.ship",
          observedAt: "2026-06-13T14:00:00.000Z",
        },
        {
          schema: "openagents.receipt.build",
          observedAt: "2026-06-13T14:30:00.000Z",
          extra: "kept only in sorted",
        },
      ]).latest,
    ).toEqual({
      schema: "openagents.receipt.build",
      observedAt: "2026-06-13T14:30:00.000Z",
    })
  })

  test("returns an empty ledger for no receipts", () => {
    expect(indexReceipts([])).toEqual({
      total: 0,
      bySchema: {},
      latest: null,
      sorted: [],
    })
  })

  test("skips receipts with unusable required fields", () => {
    const ledger = indexReceipts([
      {
        schema: "openagents.receipt.ship",
        observedAt: "2026-06-13T14:00:00.000Z",
      },
      {
        schema: "",
        observedAt: "2026-06-13T14:05:00.000Z",
      },
      {
        schema: "openagents.receipt.build",
        observedAt: " ",
      },
    ])

    expect(ledger).toMatchObject({
      total: 1,
      bySchema: {
        "openagents.receipt.ship": 1,
      },
      latest: {
        schema: "openagents.receipt.ship",
        observedAt: "2026-06-13T14:00:00.000Z",
      },
    })
  })

  test("does not mutate the caller's array or receipt objects", () => {
    const receipts = [
      {
        schema: "openagents.receipt.ship",
        observedAt: "2026-06-13T14:00:00.000Z",
        nested: { state: "first" },
      },
      {
        schema: "openagents.receipt.ship",
        observedAt: "2026-06-13T14:15:00.000Z",
        nested: { state: "latest" },
      },
    ]
    const originalOrder = receipts.map((receipt) => receipt.observedAt)

    const ledger = indexReceipts(receipts)

    expect(receipts.map((receipt) => receipt.observedAt)).toEqual(originalOrder)
    expect(ledger.sorted[0]).not.toBe(receipts[1])
    expect(ledger.sorted[0]).toEqual(receipts[1])
  })

  test("keeps invalid timestamps after parseable timestamps", () => {
    const ledger = indexReceipts([
      {
        schema: "openagents.receipt.manual",
        observedAt: "not-a-timestamp",
      },
      {
        schema: "openagents.receipt.ship",
        observedAt: "2026-06-13T14:00:00.000Z",
      },
    ])

    expect(ledger.sorted.map((receipt) => receipt.observedAt)).toEqual([
      "2026-06-13T14:00:00.000Z",
      "not-a-timestamp",
    ])
  })
})
