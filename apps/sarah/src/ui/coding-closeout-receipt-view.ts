import {
  Accordion,
  Badge,
  Button,
  Card,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  defineIntent,
  type ButtonVariant,
  type TextView,
  type Tone,
  type View,
} from "@effect-native/core"
import { Schema } from "@effect-native/core/effect"

import type { SarahCodingCloseoutReceipt } from "../contracts/coding-closeout-receipt.ts"
import {
  SARAH_OWNER_FLEET_INTERACTIVE,
  SARAH_OWNER_FLEET_READ_ONLY,
  type SarahOwnerFleetInteractionMode,
} from "./owner-fleet-interaction.ts"

export const SARAH_CODING_RECEIPT_ACTION_INTENT =
  "SarahCodingReceiptAction" as const
export const SARAH_CODING_RECEIPT_EVIDENCE_TOGGLE_INTENT =
  "SarahCodingReceiptEvidenceToggle" as const

// Keep receipt interaction decoding on the Effect Native runtime. These
// constraints mirror the owner-safe receipt contract while the domain and EN
// workspaces resolve different Effect v4 beta builds.
const SarahCodingReceiptPublicRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const SarahCodingReceiptNextAction = Schema.Union([
  Schema.Struct({
    action: Schema.Literal("resolve_approval"),
    targetRef: SarahCodingReceiptPublicRef,
    decisions: Schema.Array(Schema.Literals(["allow", "deny"])),
  }),
  Schema.Struct({
    action: Schema.Literal("open_artifact"),
    targetRef: SarahCodingReceiptPublicRef,
  }),
  Schema.Struct({
    action: Schema.Literal("open_verification"),
    targetRef: SarahCodingReceiptPublicRef,
  }),
  Schema.Struct({
    action: Schema.Literal("open_closeout"),
    targetRef: SarahCodingReceiptPublicRef,
  }),
  Schema.Struct({
    action: Schema.Literal("control_run"),
    targetRef: SarahCodingReceiptPublicRef,
    runControl: Schema.Literals(["pause", "resume", "drain", "stop"]),
  }),
])

export const SarahCodingReceiptAction = defineIntent(
  SARAH_CODING_RECEIPT_ACTION_INTENT,
  SarahCodingReceiptNextAction,
)
export const SarahCodingReceiptEvidenceToggle = defineIntent(
  SARAH_CODING_RECEIPT_EVIDENCE_TOGGLE_INTENT,
  Schema.Struct({ cardRef: SarahCodingReceiptPublicRef }),
)
export const sarahCodingReceiptIntents = [
  SarahCodingReceiptAction,
  SarahCodingReceiptEvidenceToggle,
] as const

export type SarahCodingCloseoutReceiptViewOptions = Readonly<{
  evidenceExpanded?: boolean
  interactionMode?: SarahOwnerFleetInteractionMode
}>

type ReceiptSection = SarahCodingCloseoutReceipt["sections"][number]
type ReceiptNextAction = SarahCodingCloseoutReceipt["sections"][5]["next"]

const keyed = <V extends View>(view: V): V & { key: string } =>
  view as V & { key: string }

const text = (
  key: string,
  content: string,
  variant: TextView["variant"] = "body",
  color: TextView["color"] = "textPrimary",
): TextView => Text({ key, content, variant, color })

const humanize = (value: string): string => value.replaceAll("_", " ")

const sentenceCase = (value: string): string => {
  const words = humanize(value)
  return words.length === 0
    ? words
    : `${words[0]?.toUpperCase()}${words.slice(1)}`
}

const statusBadge = (key: string, label: string, tone: Tone): View =>
  Badge({
    key,
    label,
    tone,
    a11y: { label: `Status: ${label}` },
  })

const sectionRow = (
  keyBase: string,
  section: ReceiptSection,
  title: string,
  badges: ReadonlyArray<View>,
  extra: ReadonlyArray<View> = [],
): View =>
  keyed(
    Stack(
      {
        key: `${keyBase}-section-${section.kind}`,
        direction: "column",
        gap: "1.5",
        padding: "2",
        a11y: {
          role: "listitem",
          label: `${title}. ${section.summary}`,
        },
        style: { width: "full" },
      },
      [
        Stack(
          {
            key: `${keyBase}-section-${section.kind}-heading`,
            direction: { base: "column", sm: "row" },
            gap: "1",
            align: "start",
            justify: "between",
            style: { width: "full" },
          },
          [
            text(
              `${keyBase}-section-${section.kind}-title`,
              title,
              "label",
            ),
            ...(badges.length === 0
              ? []
              : [
                  Stack(
                    {
                      key: `${keyBase}-section-${section.kind}-badges`,
                      direction: { base: "column", sm: "row" },
                      gap: "1",
                      align: "start",
                    },
                    badges,
                  ),
                ]),
          ],
        ),
        text(
          `${keyBase}-section-${section.kind}-summary`,
          section.summary,
          "body",
          "textMuted",
        ),
        ...extra,
      ],
    ),
  )

const outcomeTone = (
  status: SarahCodingCloseoutReceipt["sections"][0]["status"],
): Tone => {
  if (status === "succeeded") return "success"
  if (status === "failed") return "danger"
  if (status === "blocked") return "warn"
  return "info"
}

const verificationTone = (
  status: SarahCodingCloseoutReceipt["sections"][1]["status"],
): Tone => {
  if (status === "passed") return "success"
  if (status === "failed") return "danger"
  return "neutral"
}

const approvalTone = (
  status: SarahCodingCloseoutReceipt["sections"][4]["approvalStatus"],
): Tone => {
  if (status === "allowed") return "success"
  if (status === "denied") return "danger"
  if (status === "pending") return "warn"
  return "neutral"
}

const costTone = (
  cost: SarahCodingCloseoutReceipt["sections"][3]["marginalCostClass"],
): Tone => {
  if (cost === "free") return "success"
  if (cost === "api_metered") return "warn"
  if (cost === "subscription") return "info"
  return "neutral"
}

const nextActionPresentation = (
  next: ReceiptNextAction,
): Readonly<{
  label: string
  accessibleLabel: string
  variant: ButtonVariant
}> => {
  if (next.action === "resolve_approval") {
    return {
      label: "Review approval",
      accessibleLabel: "Review the approval required for this coding work",
      variant: "primary",
    }
  }
  if (next.action === "open_artifact") {
    return {
      label: "Open artifact",
      accessibleLabel: "Open the safe change artifact for this coding work",
      variant: "secondary",
    }
  }
  if (next.action === "open_verification") {
    return {
      label: "View verification",
      accessibleLabel: "Open verification evidence for this coding work",
      variant: "secondary",
    }
  }
  if (next.action === "open_closeout") {
    return {
      label: "View closeout",
      accessibleLabel: "Open closeout evidence for this coding work",
      variant: "secondary",
    }
  }
  if (next.action === "control_run") {
    return {
      label: `${sentenceCase(next.runControl)} run`,
      accessibleLabel: `${sentenceCase(next.runControl)} the fleet run for this coding work`,
      variant: "primary",
    }
  }
  return {
    label: "No action available",
    accessibleLabel: "No next action is available for this coding work",
    variant: "ghost",
  }
}

const nextActionButton = (
  keyBase: string,
  next: ReceiptNextAction,
): ReadonlyArray<View> => {
  if (next.action === "none") return []
  const presentation = nextActionPresentation(next)
  return [
    Button({
      key: `${keyBase}-next-action`,
      label: presentation.label,
      variant: presentation.variant,
      onPress: IntentRef(
        SarahCodingReceiptAction.name,
        StaticPayload(next),
      ),
      a11y: { label: presentation.accessibleLabel },
      style: { alignSelf: "start" },
    }),
  ]
}

const evidenceRows = (
  keyBase: string,
  receipt: SarahCodingCloseoutReceipt,
): ReadonlyArray<View> => {
  const verification = receipt.sections[1]
  const changes = receipt.sections[2]
  const capacity = receipt.sections[3]
  const approval = receipt.sections[4]
  const rows: Array<readonly [string, string]> = [
    ["Receipt", receipt.cardRef],
    ["Run", receipt.runRef],
    ["Work unit", receipt.workUnitRef],
    ["Assignment", receipt.assignmentRef],
  ]

  if (verification.verificationRef !== null) {
    rows.push(["Verification", verification.verificationRef])
  }
  if (changes.artifactRef !== null) {
    rows.push(["Artifact", changes.artifactRef])
  }
  if (capacity.accountRefHash !== null) {
    rows.push(["Account (hashed)", capacity.accountRefHash])
  }
  approval.approvalRefs.forEach((approvalRef, index) => {
    rows.push([`Approval ${index + 1}`, approvalRef])
  })
  if (approval.authorityRef !== null) {
    rows.push(["Authority", approval.authorityRef])
  }

  return rows.map(([label, value], index) =>
    keyed(
      Stack(
        {
          key: `${keyBase}-evidence-${index}`,
          direction: { base: "column", sm: "row" },
          gap: "1",
          align: "start",
          a11y: { role: "listitem", label: `${label}: ${value}` },
          style: { width: "full" },
        },
        [
          text(
            `${keyBase}-evidence-${index}-label`,
            label,
            "caption",
          ),
          text(
            `${keyBase}-evidence-${index}-value`,
            value,
            "caption",
            "textMuted",
          ),
        ],
      ),
    ),
  )
}

const evidenceDisclosure = (
  keyBase: string,
  receipt: SarahCodingCloseoutReceipt,
  expanded: boolean,
): View =>
  Accordion({
    key: `${keyBase}-evidence`,
    mode: "single",
    expandedIds: expanded ? ["references"] : [],
    onToggle: IntentRef(
      SarahCodingReceiptEvidenceToggle.name,
      StaticPayload({ cardRef: receipt.cardRef }),
    ),
    items: [
      {
        id: "references",
        header: "Evidence references",
        content: [
          Stack(
            {
              key: `${keyBase}-evidence-list`,
              direction: "column",
              gap: "1.5",
              padding: "2",
              a11y: {
                role: "list",
                label: "Audit and evidence references",
              },
              style: { width: "full" },
            },
            evidenceRows(keyBase, receipt),
          ),
        ],
      },
    ],
    a11y: {
      role: "group",
      label: "Evidence references for this coding receipt",
      expanded,
    },
    style: { width: "full" },
  })

/**
 * Pure FC-3 receipt view. The contract's tuple defines the reading order;
 * renderer/runtime wiring only handles the two exported intent names.
 */
export function sarahCodingCloseoutReceiptView(
  receipt: SarahCodingCloseoutReceipt,
  options: SarahCodingCloseoutReceiptViewOptions = {},
): View {
  const [outcome, verification, changes, capacity, approval, nextAction] =
    receipt.sections
  const action = nextActionPresentation(nextAction.next)
  const interactionMode =
    options.interactionMode ?? SARAH_OWNER_FLEET_READ_ONLY
  const keyBase = `coding-receipt-${receipt.cardRef}`
  const capacityContext =
    capacity.harnessKind === null
      ? "Harness not reported"
      : capacity.capacityClass === null
        ? `${sentenceCase(capacity.harnessKind)} harness. Capacity classification not reported.`
        : `${sentenceCase(capacity.harnessKind)} harness. ${sentenceCase(capacity.capacityClass)} capacity.`
  const authorityContext =
    approval.authorityClass === null
      ? []
      : [
          text(
            `${keyBase}-authority-class`,
            `Authority class: ${sentenceCase(approval.authorityClass)}`,
            "caption",
            "textMuted",
          ),
        ]

  return Card(
    {
      key: keyBase,
      padding: "4",
      radius: "lg",
      a11y: {
        role: "region",
        label: [
          "Coding closeout receipt",
          outcome.summary,
          verification.summary,
          changes.summary,
          capacity.summary,
          approval.summary,
          nextAction.summary,
        ].join(". "),
      },
      style: {
        width: "full",
        backgroundColor: "surfaceRaised",
        borderColor: "border",
        borderWidth: 1,
      },
    },
    [
      Stack(
        {
          key: `${keyBase}-header`,
          direction: { base: "column", sm: "row" },
          gap: "2",
          align: "start",
          justify: "between",
          style: { width: "full" },
        },
        [
          Stack(
            {
              key: `${keyBase}-heading-copy`,
              direction: "column",
              gap: "0.5",
            },
            [
              text(`${keyBase}-title`, "Coding closeout", "title"),
              text(
                `${keyBase}-subtitle`,
                "What happened, what was verified, and what needs attention.",
                "caption",
                "textMuted",
              ),
            ],
          ),
          statusBadge(
            `${keyBase}-overall-status`,
            sentenceCase(outcome.status),
            outcomeTone(outcome.status),
          ),
        ],
      ),
      Stack(
        {
          key: `${keyBase}-sections`,
          direction: "column",
          gap: "1",
          padding: "2",
          a11y: {
            role: "list",
            label: "Coding closeout summary in reading order",
          },
          style: { width: "full" },
        },
        [
          sectionRow(keyBase, outcome, "Outcome", [
            statusBadge(
              `${keyBase}-outcome-status`,
              sentenceCase(outcome.status),
              outcomeTone(outcome.status),
            ),
          ]),
          sectionRow(keyBase, verification, "Verification", [
            statusBadge(
              `${keyBase}-verification-status`,
              sentenceCase(verification.status),
              verificationTone(verification.status),
            ),
          ]),
          sectionRow(keyBase, changes, "Changes", [
            statusBadge(
              `${keyBase}-changes-status`,
              sentenceCase(changes.status),
              changes.status === "reported" ? "info" : "neutral",
            ),
          ]),
          sectionRow(
            keyBase,
            capacity,
            "Capacity & cost",
            [
              statusBadge(
                `${keyBase}-capacity-status`,
                sentenceCase(capacity.status),
                capacity.status === "reported" ? "info" : "neutral",
              ),
              statusBadge(
                `${keyBase}-cost-status`,
                `Cost: ${sentenceCase(capacity.marginalCostClass)}`,
                costTone(capacity.marginalCostClass),
              ),
            ],
            [
              text(
                `${keyBase}-capacity-context`,
                capacityContext,
                "caption",
                "textMuted",
              ),
            ],
          ),
          sectionRow(
            keyBase,
            approval,
            "Approval & authority",
            [
              statusBadge(
                `${keyBase}-approval-status`,
                `Approval: ${sentenceCase(approval.approvalStatus)}`,
                approvalTone(approval.approvalStatus),
              ),
              statusBadge(
                `${keyBase}-authority-status`,
                `Authority: ${sentenceCase(approval.authorityStatus)}`,
                approval.authorityStatus === "reported" ? "info" : "neutral",
              ),
            ],
            authorityContext,
          ),
          sectionRow(
            keyBase,
            nextAction,
            "Next step",
            [
              statusBadge(
                `${keyBase}-next-status`,
                nextAction.next.action === "none" ? "No action" : "Available",
                nextAction.next.action === "none" ? "neutral" : "info",
              ),
            ],
            nextAction.next.action === "none"
              ? [
                  text(
                    `${keyBase}-next-action-unavailable`,
                    action.accessibleLabel,
                    "caption",
                    "textMuted",
                  ),
                ]
              : interactionMode === SARAH_OWNER_FLEET_INTERACTIVE
                ? nextActionButton(keyBase, nextAction.next)
                : [
                    text(
                      `${keyBase}-next-action-read-only`,
                      "Controls unavailable in this surface. The reported next action cannot be submitted here.",
                      "caption",
                      "textMuted",
                    ),
                  ],
          ),
        ],
      ),
      evidenceDisclosure(
        keyBase,
        receipt,
        options.evidenceExpanded === true,
      ),
    ],
  )
}
