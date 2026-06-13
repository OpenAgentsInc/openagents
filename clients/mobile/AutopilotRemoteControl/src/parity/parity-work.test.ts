import { describe, expect, test } from "bun:test"

import {
  artifactRowsViewModel,
  receiptRowsViewModel,
  type Artifact,
  type Receipt,
} from "./artifacts-view-model"
import { earningsViewModel } from "./earnings-view-model"
import { verifyViewModel, type VerifyState } from "./verify-view-model"

describe("mobile parity view models", () => {
  test("maps verify passed and failed states to desktop tone semantics", () => {
    const passed: VerifyState = {
      command: ["bun", "test", "src/parity/parity-work.test.ts"],
      status: "passed",
      requiredArtifacts: [{ ref: "artifact.public.verify.report", present: true }],
    }
    const failed: VerifyState = {
      command: ["npm", "run", "verify command"],
      status: "failed",
      requiredArtifacts: [
        { ref: "artifact.public.verify.report", present: true },
        { ref: "artifact.public.verify.diff", present: false },
      ],
    }

    expect(verifyViewModel(passed)).toEqual({
      command: "bun test src/parity/parity-work.test.ts",
      status: "passed",
      statusTone: "success",
      requiredArtifacts: [
        {
          ref: "artifact.public.verify.report",
          status: "present",
          tone: "success",
        },
      ],
    })
    expect(verifyViewModel(failed)).toEqual({
      command: "npm run 'verify command'",
      status: "failed",
      statusTone: "danger",
      requiredArtifacts: [
        {
          ref: "artifact.public.verify.report",
          status: "present",
          tone: "success",
        },
        {
          ref: "artifact.public.verify.diff",
          status: "missing",
          tone: "danger",
        },
      ],
    })
  })

  test("maps artifact and receipt rows as refs-only with status tones", () => {
    const artifacts: Artifact[] = [
      {
        name: "summary",
        digestRef: "artifact.public.autopilot.summary.0123456789abcdef",
        contentType: "application/json",
      },
      {
        name: "trace",
        digestRef: "artifact.public.trace",
      },
    ]
    const receipts: Receipt[] = [
      {
        kind: "verify",
        digestRef: "receipt.public.verify.0123456789abcdef",
        status: "ok",
      },
      {
        kind: "settlement",
        digestRef: "receipt.public.settlement",
        status: "pending",
      },
      {
        kind: "artifact",
        digestRef: "receipt.public.artifact",
        status: "failed",
      },
    ]

    expect(artifactRowsViewModel(artifacts)).toEqual([
      {
        name: "summary",
        digestRef: "artifact.public.autopilot.summary.0123456789abcdef",
        displayDigestRef: "artifact.public.au...6789abcdef",
        contentType: "application/json",
        contentTypeLabel: "application/json / size: ref-only",
      },
      {
        name: "trace",
        digestRef: "artifact.public.trace",
        displayDigestRef: "artifact.public.trace",
        contentType: "unknown",
        contentTypeLabel: "content-type: unknown / size: ref-only",
      },
    ])
    expect(JSON.stringify(artifactRowsViewModel(artifacts))).not.toContain("raw")
    expect(receiptRowsViewModel(receipts).map((receipt) => receipt.statusTone)).toEqual([
      "success",
      "warning",
      "danger",
    ])
  })

  test("maps earnings balance and entries without action fields", () => {
    const viewModel = earningsViewModel({
      balanceSats: 2100,
      entries: [
        {
          ref: "earning.public.accepted_work.1",
          amountSats: 1100,
          at: "2026-06-13T12:00:00.000Z",
        },
        {
          ref: "earning.public.accepted_work.2",
          amountSats: 1000,
          at: "2026-06-13T12:05:00.000Z",
        },
      ],
    })

    expect(viewModel).toEqual({
      balanceSats: 2100,
      balanceLabel: "2100 sats",
      entries: [
        {
          ref: "earning.public.accepted_work.1",
          amountSats: 1100,
          amountLabel: "1100 sats",
          at: "2026-06-13T12:00:00.000Z",
        },
        {
          ref: "earning.public.accepted_work.2",
          amountSats: 1000,
          amountLabel: "1000 sats",
          at: "2026-06-13T12:05:00.000Z",
        },
      ],
    })
    expect("actions" in viewModel).toBe(false)
    expect("action" in viewModel.entries[0]!).toBe(false)
  })
})
