import { describe, expect, test } from "bun:test"

import {
  assessDelivery,
  buildDeliveryReceipt,
  type DeliveryReadiness,
  type DeliveryReceipt,
} from "../src/tas/delivery-receipt"

const readyReadiness: DeliveryReadiness = {
  repoRef: "repo.github.openagents.openagents",
  headRef: "head.sha.fixture",
  requiredChecks: [
    {
      name: "typecheck",
      status: "passed",
    },
    {
      name: "unit tests",
      status: "passed",
    },
  ],
  evidenceRefs: ["evidence.delivery.fixture.tests"],
}

const receiptKeys: Array<keyof DeliveryReceipt> = [
  "decision",
  "evidenceRefs",
  "headRef",
  "repoRef",
  "requiredChecks",
]

describe("tas delivery receipt", () => {
  test("marks all-passed checks with evidence ready", () => {
    expect(assessDelivery(readyReadiness)).toEqual({
      ready: true,
      blockers: [],
    })
  })

  test("blocks failing and pending checks", () => {
    expect(
      assessDelivery({
        ...readyReadiness,
        requiredChecks: [
          {
            name: "typecheck",
            status: "failed",
          },
          {
            name: "integration smoke",
            status: "pending",
          },
        ],
      }),
    ).toEqual({
      ready: false,
      blockers: [
        "required check typecheck is failed",
        "required check integration smoke is pending",
      ],
    })
  })

  test("blocks delivery with no evidence refs", () => {
    expect(
      assessDelivery({
        ...readyReadiness,
        evidenceRefs: [],
      }),
    ).toEqual({
      ready: false,
      blockers: ["at least one evidence ref is required"],
    })
  })

  test("builds refs-only delivery receipts", () => {
    const readinessWithRawFields = {
      ...readyReadiness,
      rawPatch: "diff --git a/private b/private",
      privateLog: "/Users/example/private/repo.log",
      requiredChecks: [
        {
          name: "unit tests",
          status: "passed" as const,
          rawOutput: "private terminal output",
        },
      ],
    }

    const receipt = buildDeliveryReceipt(readinessWithRawFields, {
      decision: "ready",
    })

    expect(receipt).toEqual({
      repoRef: readyReadiness.repoRef,
      headRef: readyReadiness.headRef,
      requiredChecks: [
        {
          name: "unit tests",
          status: "passed",
        },
      ],
      evidenceRefs: readyReadiness.evidenceRefs,
      decision: "ready",
    })
    expect(Object.keys(receipt).sort()).toEqual([...receiptKeys].sort())
    expect(JSON.stringify(receipt)).not.toContain("diff --git")
    expect(JSON.stringify(receipt)).not.toContain("/Users/example/private/repo.log")
    expect(JSON.stringify(receipt)).not.toContain("private terminal output")
  })
})
