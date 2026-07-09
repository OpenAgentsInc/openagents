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
  cardRef: "closeout.receipt.codex",
  runRef: "fleet.run.receipt",
  workUnitRef: "#8639",
  assignmentRef: "assignment.receipt.codex",
  sections: [
    {
      kind: "outcome",
      status: "succeeded",
      assignmentStatus: "accepted_work",
      summary: "Work unit succeeded",
    },
    {
      kind: "verification",
      status: "passed",
      verificationRef: "verification.receipt.codex",
      summary: "Verification passed",
    },
    {
      kind: "changes",
      status: "reported",
      changeClass: "source_and_tests",
      artifactRef: "artifact.public.receipt.codex",
      summary: "Changed source and tests",
    },
    {
      kind: "capacity_and_cost",
      status: "reported",
      harnessKind: "codex",
      accountRefHash: "account.pylon.codex.11111111",
      capacityClass: "owner_local",
      marginalCostClass: "not_measured",
      summary: "Capacity reported. Cost not measured.",
    },
    {
      kind: "approval_and_authority",
      approvalStatus: "allowed",
      approvalRefs: ["approval.receipt.codex"],
      authorityStatus: "reported",
      authorityClass: "coding_session_control",
      authorityRef: "authority.owner.receipt.codex",
      summary: "Approval allowed. Authority reported.",
    },
    {
      kind: "next_action",
      next: {
        action: "open_artifact",
        targetRef: "artifact.public.receipt.codex",
      },
      summary: "Open safe artifact",
    },
  ],
})

const notReportedReceipt = Schema.decodeUnknownSync(
  SarahCodingCloseoutReceipt,
)({
  ...receipt,
  cardRef: "closeout.receipt.unreported",
  sections: [
    {
      ...receipt.sections[0],
      status: "in_progress",
      summary: "Work unit in progress",
    },
    {
      kind: "verification",
      status: "not_reported",
      verificationRef: null,
      summary: "Verification not reported",
    },
    {
      kind: "changes",
      status: "not_reported",
      changeClass: null,
      artifactRef: null,
      summary: "Changes not reported",
    },
    {
      kind: "capacity_and_cost",
      status: "not_reported",
      harnessKind: null,
      accountRefHash: null,
      capacityClass: null,
      marginalCostClass: "not_measured",
      summary: "Capacity not reported. Cost not measured.",
    },
    {
      kind: "approval_and_authority",
      approvalStatus: "not_reported",
      approvalRefs: [],
      authorityStatus: "not_reported",
      authorityClass: null,
      authorityRef: null,
      summary: "Approval not reported. Authority not reported.",
    },
    {
      kind: "next_action",
      next: { action: "none", targetRef: null },
      summary: "No action available",
    },
  ],
})

const failedReceipt = Schema.decodeUnknownSync(SarahCodingCloseoutReceipt)({
  ...receipt,
  cardRef: "closeout.receipt.failed",
  sections: [
    {
      ...receipt.sections[0],
      status: "failed",
      summary: "Work unit failed",
    },
    {
      ...receipt.sections[1],
      status: "failed",
      summary: "Verification failed",
    },
    ...receipt.sections.slice(2),
  ],
})

const keyBase = `coding-receipt-${receipt.cardRef}`
const notReportedKeyBase = `coding-receipt-${notReportedReceipt.cardRef}`
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

describe("FC-3 Sarah coding closeout receipt view", () => {
  test("renders the six contract sections in the fixed one-minute reading order", () => {
    const view = sarahCodingCloseoutReceiptView(receipt)
    const sectionList = findByKey(view, `${keyBase}-sections`)
    const children = sectionList?.children as ReadonlyArray<AnyNode>

    expect(sectionList?.a11y).toEqual({
      role: "list",
      label: "Coding closeout summary in reading order",
    })
    expect(children.map((child) => child.key)).toEqual([
      `${keyBase}-section-outcome`,
      `${keyBase}-section-verification`,
      `${keyBase}-section-changes`,
      `${keyBase}-section-capacity_and_cost`,
      `${keyBase}-section-approval_and_authority`,
      `${keyBase}-section-next_action`,
    ])
    expect(children.every((child) => (child.a11y as AnyNode)?.role === "listitem")).toBe(
      true,
    )
    expect(findByKey(view, `${keyBase}-verification-status`)).toMatchObject({
      label: "Passed",
      tone: "success",
    })
  })

  test("keeps audit refs in one expandable disclosure rather than primary copy", () => {
    const collapsed = sarahCodingCloseoutReceiptView(receipt)
    const expanded = sarahCodingCloseoutReceiptView(receipt, {
      evidenceExpanded: true,
    })
    const visibleText = visibleTextOutsideAccordion(collapsed).join(" ")
    const disclosure = findByKey(expanded, `${keyBase}-evidence`)

    const references = [
      receipt.cardRef,
      receipt.runRef,
      receipt.workUnitRef,
      receipt.assignmentRef,
      receipt.sections[1].verificationRef,
      receipt.sections[2].artifactRef,
      receipt.sections[3].accountRefHash,
      receipt.sections[4].authorityRef,
    ].filter((reference): reference is string => reference !== null)
    for (const reference of references) {
      expect(visibleText).not.toContain(reference)
      expect(JSON.stringify(disclosure)).toContain(reference)
    }
    expect(findByKey(collapsed, `${keyBase}-evidence`)?.expandedIds).toEqual(
      [],
    )
    expect(disclosure?.expandedIds).toEqual(["references"])
    expect(disclosure?.a11y).toMatchObject({ expanded: true })
    expect(disclosure?.onToggle).toEqual({
      name: SARAH_CODING_RECEIPT_EVIDENCE_TOGGLE_INTENT,
      payload: {
        _tag: "StaticPayload",
        value: { cardRef: receipt.cardRef },
      },
    })
  })

  test("emits the closed next-action payload behind one accessible control", () => {
    const view = sarahCodingCloseoutReceiptView(receipt, {
      interactionMode: SARAH_OWNER_FLEET_INTERACTIVE,
    })
    const button = findByKey(view, `${keyBase}-next-action`)

    expect(button).toMatchObject({
      _tag: "Button",
      label: "Open artifact",
      variant: "secondary",
      a11y: {
        label: "Open the safe change artifact for this coding work",
      },
      onPress: {
        name: SARAH_CODING_RECEIPT_ACTION_INTENT,
        payload: {
          _tag: "StaticPayload",
          value: receipt.sections[5].next,
        },
      },
    })
    const decodedAction = NativeSchema.decodeUnknownSync(
      SarahCodingReceiptAction.payloadSchema,
    )(receipt.sections[5].next)
    expect(decodedAction).toEqual({
      action: "open_artifact",
      targetRef: "artifact.public.receipt.codex",
    })
    expect(() =>
      NativeSchema.decodeUnknownSync(SarahCodingReceiptAction.payloadSchema)({
        action: "open_artifact",
        targetRef: "/Users/alice/private/repo",
      }),
    ).toThrow()
    expect(() =>
      NativeSchema.decodeUnknownSync(SarahCodingReceiptAction.payloadSchema)({
        action: "none",
        targetRef: null,
      }),
    ).toThrow()
    expect(() =>
      NativeSchema.decodeUnknownSync(
        SarahCodingReceiptEvidenceToggle.payloadSchema,
      )({ cardRef: "owner@example.com" }),
    ).toThrow()
  })

  test("states missing verdicts and measurements without implying success", () => {
    const view = sarahCodingCloseoutReceiptView(notReportedReceipt)
    const serialized = JSON.stringify(view)

    expect(serialized).toContain("Verification not reported")
    expect(serialized).toContain("Changes not reported")
    expect(serialized).toContain("Capacity not reported. Cost not measured.")
    expect(serialized).toContain("Approval not reported. Authority not reported.")
    expect(serialized).toContain("Cost: Not measured")
    expect(serialized).toContain("Harness not reported")
    expect(serialized).not.toContain('"label":"Passed"')
    expect(
      findByKey(view, `${notReportedKeyBase}-verification-status`),
    ).toMatchObject({ label: "Not reported", tone: "neutral" })
    expect(findByKey(view, `${notReportedKeyBase}-cost-status`)).toMatchObject({
      label: "Cost: Not measured",
      tone: "neutral",
    })
    expect(findByKey(view, `${notReportedKeyBase}-next-action`)).toBeNull()
  })

  test("marks failed outcomes and verification as danger, never as success", () => {
    const view = sarahCodingCloseoutReceiptView(failedReceipt)

    expect(findByKey(view, `${failedKeyBase}-overall-status`)).toMatchObject({
      label: "Failed",
      tone: "danger",
    })
    expect(findByKey(view, `${failedKeyBase}-verification-status`)).toMatchObject({
      label: "Failed",
      tone: "danger",
    })
  })

  test("uses one receipt Card and no React or local substitute primitives", () => {
    const view = sarahCodingCloseoutReceiptView(receipt)

    expect(findAllByTag(view, "Card")).toHaveLength(1)
    expect(findAllByTag(view, "Accordion")).toHaveLength(1)
    expect(findAllByTag(view, "BackgroundGradient")).toHaveLength(0)
    expect(JSON.stringify(view)).not.toContain("React")
  })

  test("scopes every keyed child to its receipt when several closeouts render", () => {
    const first = sarahCodingCloseoutReceiptView(receipt)
    const second = sarahCodingCloseoutReceiptView(notReportedReceipt)
    const firstKeys = new Set(
      findAllByTag(first, "Stack").map((node) => node.key as string),
    )
    const secondKeys = new Set(
      findAllByTag(second, "Stack").map((node) => node.key as string),
    )

    expect([...firstKeys].every((key) => key.startsWith(keyBase))).toBe(true)
    expect([...secondKeys].every((key) => key.startsWith(notReportedKeyBase))).toBe(
      true,
    )
    expect([...firstKeys].some((key) => secondKeys.has(key))).toBe(false)
  })
})
