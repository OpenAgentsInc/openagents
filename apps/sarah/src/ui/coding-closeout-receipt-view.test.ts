import { describe, expect, test } from "bun:test"
import { Schema as NativeSchema } from "@effect-native/core/effect"
import { Schema } from "effect"

import { SarahCodingCloseoutReceipt } from "../contracts/coding-closeout-receipt.ts"
import {
  SARAH_CODING_RECEIPT_ACTION_INTENT,
  SARAH_CODING_RECEIPT_EVIDENCE_TOGGLE_INTENT,
  SarahCodingReceiptAction,
  SarahCodingReceiptEvidenceToggle,
  sarahCodingCloseoutReceiptView,
} from "./coding-closeout-receipt-view.ts"
import { SARAH_OWNER_FLEET_INTERACTIVE } from "./owner-fleet-interaction.ts"

type AnyNode = { readonly _tag?: string; readonly [key: string]: unknown }

const receipt = Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)({
  schema: "sarah.coding_closeout_receipt.v1",
  cardRef: "attempt.receipt.codex",
  runRef: "fleet.run.receipt",
  workUnitRef: "unit.receipt.codex",
  attemptRef: "attempt.receipt.codex",
  assignmentRef: "assignment.receipt.codex",
  sections: [
    {
      kind: "outcome",
      status: "succeeded",
      attemptState: "succeeded",
      closeoutRef: "closeout.receipt.codex",
      blockerRefs: [],
      summary: "Attempt succeeded",
    },
    {
      kind: "verification",
      status: "passed",
      verificationRef: "verifier.receipt.codex",
      evidenceRefs: ["test.receipt.codex"],
      summary: "Verification passed",
    },
    {
      kind: "changes",
      status: "reported",
      changeClass: "attempt_evidence",
      artifactRef: "artifact.receipt.codex",
      artifactRefs: ["artifact.receipt.codex"],
      proofRefs: ["proof.receipt.codex"],
      summary: "Attempt artifacts and proofs reported",
    },
    {
      kind: "capacity_and_cost",
      status: "reported",
      harnessKind: "codex",
      pylonRef: "pylon-owner-1",
      accountRefHash: `account.pylon.codex.${"1".repeat(24)}`,
      capacityClass: "owner_local",
      marginalCostClass: "subscription",
      usageEvidence: {
        schema: "openagents.pylon.fleet_run_usage_evidence.v1",
        truth: "exact",
        harnessKind: "codex",
        evidenceRef: "evidence.receipt.codex",
        assignmentRef: "assignment.receipt.codex",
        pylonRef: "pylon-owner-1",
        provider: "pylon-codex-own-capacity",
        model: "openagents/pylon-codex",
        demandKind: "own_capacity",
        demandSource: "khala_coding_delegation",
        inputTokens: 8,
        outputTokens: 5,
        reasoningTokens: 2,
        cacheReadTokens: 3,
        totalTokens: 13,
        tokenRows: 1,
        tokenUsageRefs: ["usage.receipt.codex"],
        proofRefs: ["proof.usage.receipt.codex"],
        closeoutChecklistRefs: ["check.closeout.receipt.codex"],
        proofChecklistRefs: ["check.proof.receipt.codex"],
      },
      summary: "Capacity reported. Exact usage 13 tokens.",
    },
    {
      kind: "approval_and_authority",
      approvalStatus: "not_required",
      approvalRefs: [],
      authorityStatus: "reported",
      authorityClass: "attempt_authority_receipt",
      authorityRef: "authority.receipt.codex",
      authorityReceiptRefs: ["authority.receipt.codex"],
      summary: "Approval not required. Authority reported.",
    },
    {
      kind: "next_action",
      next: {
        action: "open_artifact",
        targetRef: "artifact.receipt.codex",
      },
      summary: "Open safe artifact",
    },
  ],
})

const grokReceipt = Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)({
  ...receipt,
  cardRef: "attempt.receipt.grok",
  workUnitRef: "unit.receipt.grok",
  attemptRef: "attempt.receipt.grok",
  assignmentRef: null,
  sections: [
    { ...receipt.sections[0], closeoutRef: "closeout.receipt.grok" },
    { ...receipt.sections[1], verificationRef: "verifier.receipt.grok" },
    {
      ...receipt.sections[2],
      artifactRef: "artifact.receipt.grok",
      artifactRefs: ["artifact.receipt.grok"],
      proofRefs: ["proof.receipt.grok"],
    },
    {
      kind: "capacity_and_cost",
      status: "reported",
      harnessKind: "grok",
      pylonRef: "pylon-owner-1",
      accountRefHash: `account.pylon.grok.${"2".repeat(24)}`,
      capacityClass: "owner_local",
      marginalCostClass: "api_metered",
      usageEvidence: {
        schema: "openagents.pylon.fleet_run_usage_evidence.v1",
        truth: "not_measured",
        harnessKind: "grok",
        evidenceRef: "evidence.receipt.grok",
        assignmentRef: "assignment.receipt.grok.usage",
        receiptRef: "receipt.receipt.grok",
        tokenUsageRefs: [],
        caveatRefs: ["caveat.receipt.grok.not_measured"],
      },
      summary: "Capacity reported. Usage not measured.",
    },
    {
      ...receipt.sections[4],
      authorityRef: "authority.receipt.grok",
      authorityReceiptRefs: ["authority.receipt.grok"],
    },
    {
      kind: "next_action",
      next: {
        action: "open_artifact",
        targetRef: "artifact.receipt.grok",
      },
      summary: "Open safe artifact",
    },
  ],
})

const failedReceipt = Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)({
  ...receipt,
  cardRef: "attempt.receipt.failed",
  workUnitRef: "unit.receipt.failed",
  attemptRef: "attempt.receipt.failed",
  sections: [
    {
      kind: "outcome",
      status: "failed",
      attemptState: "failed",
      closeoutRef: "closeout.receipt.failed",
      blockerRefs: ["blocker.receipt.failed"],
      summary: "Attempt failed",
    },
    {
      kind: "verification",
      status: "not_reported",
      verificationRef: null,
      evidenceRefs: [],
      summary: "Verification not reported",
    },
    {
      kind: "changes",
      status: "not_reported",
      changeClass: null,
      artifactRef: null,
      artifactRefs: [],
      proofRefs: ["proof.receipt.failed"],
      summary: "Changes not reported",
    },
    receipt.sections[3],
    {
      kind: "approval_and_authority",
      approvalStatus: "not_required",
      approvalRefs: [],
      authorityStatus: "not_reported",
      authorityClass: null,
      authorityRef: null,
      authorityReceiptRefs: [],
      summary: "Approval not required. Authority not reported.",
    },
    {
      kind: "next_action",
      next: {
        action: "open_closeout",
        targetRef: "closeout.receipt.failed",
      },
      summary: "Open closeout",
    },
  ],
})

const keyBase = `coding-receipt-${receipt.cardRef}`
const failedKeyBase = `coding-receipt-${failedReceipt.cardRef}`

const findByKey = (node: unknown, key: string): AnyNode | null => {
  if (node === null || typeof node !== "object") return null
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = findByKey(child, key)
      if (found !== null) return found
    }
    return null
  }
  const record = node as AnyNode
  if (record.key === key) return record
  for (const value of Object.values(record)) {
    const found = findByKey(value, key)
    if (found !== null) return found
  }
  return null
}

const findAllByTag = (node: unknown, tag: string): ReadonlyArray<AnyNode> => {
  if (node === null || typeof node !== "object") return []
  if (Array.isArray(node)) {
    return node.flatMap((child) => findAllByTag(child, tag))
  }
  const record = node as AnyNode
  return [
    ...(record._tag === tag ? [record] : []),
    ...Object.values(record).flatMap((value) => findAllByTag(value, tag)),
  ]
}

const visibleTextOutsideAccordion = (node: unknown): ReadonlyArray<string> => {
  if (node === null || typeof node !== "object") return []
  if (Array.isArray(node)) {
    return node.flatMap((child) => visibleTextOutsideAccordion(child))
  }
  const record = node as AnyNode
  if (record._tag === "Accordion") return []
  return [
    ...(record._tag === "Text" && typeof record.content === "string"
      ? [record.content]
      : []),
    ...Object.values(record).flatMap((value) =>
      visibleTextOutsideAccordion(value),
    ),
  ]
}

describe("FC-3 Sarah attempt-backed closeout receipt view", () => {
  test("renders the fixed six-section reading order", () => {
    const view = sarahCodingCloseoutReceiptView(receipt)
    const sectionList = findByKey(view, `${keyBase}-sections`)
    const children = sectionList?.children as ReadonlyArray<AnyNode>
    expect(children.map((child) => child.key)).toEqual([
      `${keyBase}-section-outcome`,
      `${keyBase}-section-verification`,
      `${keyBase}-section-changes`,
      `${keyBase}-section-capacity_and_cost`,
      `${keyBase}-section-approval_and_authority`,
      `${keyBase}-section-next_action`,
    ])
    expect(findByKey(view, `${keyBase}-verification-status`)).toMatchObject({
      label: "Passed",
      tone: "success",
    })
  })

  test("keeps complete attempt evidence behind one disclosure", () => {
    const collapsed = sarahCodingCloseoutReceiptView(receipt)
    const expanded = sarahCodingCloseoutReceiptView(receipt, {
      evidenceExpanded: true,
    })
    const visibleText = visibleTextOutsideAccordion(collapsed).join(" ")
    const disclosure = findByKey(expanded, `${keyBase}-evidence`)
    const references = [
      receipt.attemptRef,
      receipt.assignmentRef,
      receipt.sections[0].closeoutRef,
      receipt.sections[1].verificationRef,
      ...receipt.sections[1].evidenceRefs,
      ...receipt.sections[2].artifactRefs,
      ...receipt.sections[2].proofRefs,
      receipt.sections[3].usageEvidence.truth === "exact"
        ? receipt.sections[3].usageEvidence.evidenceRef
        : null,
      ...(receipt.sections[3].usageEvidence.truth === "exact"
        ? [
            ...receipt.sections[3].usageEvidence.tokenUsageRefs,
            ...receipt.sections[3].usageEvidence.proofRefs,
            ...receipt.sections[3].usageEvidence.closeoutChecklistRefs,
            ...receipt.sections[3].usageEvidence.proofChecklistRefs,
          ]
        : []),
      ...receipt.sections[4].authorityReceiptRefs,
    ].filter((reference): reference is string => reference !== null)
    for (const reference of references) {
      expect(visibleText).not.toContain(reference)
      expect(JSON.stringify(disclosure)).toContain(reference)
    }
    expect(disclosure?.expandedIds).toEqual(["references"])
    expect(disclosure?.onToggle).toEqual({
      name: SARAH_CODING_RECEIPT_EVIDENCE_TOGGLE_INTENT,
      payload: {
        _tag: "StaticPayload",
        value: { cardRef: receipt.cardRef },
      },
    })
  })

  test("renders nullable assignment and Grok not-measured caveats honestly", () => {
    const view = sarahCodingCloseoutReceiptView(grokReceipt, {
      evidenceExpanded: true,
    })
    const serialized = JSON.stringify(view)
    expect(serialized).not.toContain("Assignment: null")
    expect(serialized).toContain("evidence.receipt.grok")
    expect(serialized).toContain("assignment.receipt.grok.usage")
    expect(serialized).toContain("receipt.receipt.grok")
    expect(serialized).toContain("caveat.receipt.grok.not_measured")
    expect(serialized).toContain("Usage not measured")
  })

  test("emits the closed next-action payload behind one accessible control", () => {
    const view = sarahCodingCloseoutReceiptView(receipt, {
      interactionMode: SARAH_OWNER_FLEET_INTERACTIVE,
    })
    const button = findByKey(view, `${keyBase}-next-action`)
    expect(button).toMatchObject({
      _tag: "Button",
      label: "Open artifact",
      onPress: {
        name: SARAH_CODING_RECEIPT_ACTION_INTENT,
        payload: { _tag: "StaticPayload", value: receipt.sections[5].next },
      },
    })
    const next = receipt.sections[5].next
    if (next.action !== "open_artifact") {
      throw new Error("fixture must expose the artifact action")
    }
    expect(
      NativeSchema.decodeUnknownSync(SarahCodingReceiptAction.payloadSchema)(
        next,
      ),
    ).toEqual(next)
    expect(() =>
      NativeSchema.decodeUnknownSync(SarahCodingReceiptEvidenceToggle.payloadSchema)(
        { cardRef: "owner@example.com" },
      ),
    ).toThrow()
  })

  test("marks failed outcome without inventing failed verification", () => {
    const view = sarahCodingCloseoutReceiptView(failedReceipt)
    expect(findByKey(view, `${failedKeyBase}-overall-status`)).toMatchObject({
      label: "Failed",
      tone: "danger",
    })
    expect(
      findByKey(view, `${failedKeyBase}-verification-status`),
    ).toMatchObject({ label: "Not reported", tone: "neutral" })
  })

  test("uses one receipt Card and scopes every keyed child to its attempt", () => {
    const first = sarahCodingCloseoutReceiptView(receipt)
    const second = sarahCodingCloseoutReceiptView(grokReceipt)
    expect(findAllByTag(first, "Card")).toHaveLength(1)
    expect(findAllByTag(first, "Accordion")).toHaveLength(1)
    expect(JSON.stringify(first)).not.toContain("React")

    const firstKeys = new Set(
      findAllByTag(first, "Stack").map((node) => node.key as string),
    )
    const secondKeys = new Set(
      findAllByTag(second, "Stack").map((node) => node.key as string),
    )
    expect([...firstKeys].every((key) => key.startsWith(keyBase))).toBe(true)
    expect([...firstKeys].some((key) => secondKeys.has(key))).toBe(false)
  })
})
