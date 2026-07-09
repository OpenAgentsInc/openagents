import {
  Button,
  Card,
  IntentRef,
  List,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  type ButtonView,
  type CardView,
  type ColorToken,
  type KeyedView,
  type TextView,
  type View,
} from "@effect-native/core"
import {
  fleetRunControlActions,
  fleetWorkerKinds,
} from "@openagentsinc/khala-fleet-intents"

import type {
  CockpitChipTone,
  EnCockpitState,
} from "./cockpit-projection"

// ---------------------------------------------------------------------------
// EN cockpit view (MH-7 / EN-5)
//
// One real fleet cockpit screen authored as a typed Effect Native tree —
// account/capacity chip strip, per-harness readiness rows, a worker/run list,
// pause/resume/drain/stop run controls, worker-selection pills, and approval
// allow/deny controls. Every control's `onPress` is a typed EN `IntentRef`
// whose name matches a `cockpitIntents` definition; the mount layer converts
// the dispatched intent to a shared `KhalaFleetIntent`.
// ---------------------------------------------------------------------------

const toneColor = (tone: CockpitChipTone): ColorToken => {
  switch (tone) {
    case "ok":
      return "accent"
    case "warn":
      return "focus"
    case "blocked":
      return "danger"
    default:
      return "textMuted"
  }
}

const keyed = <V extends View>(view: V): V & KeyedView => view as V & KeyedView

const text = (
  key: string,
  content: string,
  variant: TextView["variant"] = "body",
  color: ColorToken = "textPrimary",
): TextView =>
  Text({
    key,
    content,
    variant,
    color,
    style: { width: "full" },
  })

const section = (key: string, children: ReadonlyArray<View>): View =>
  Stack(
    {
      key,
      direction: "column",
      gap: "3",
      padding: "4",
      style: {
        width: "full",
        backgroundColor: "surface",
        borderColor: "border",
        borderWidth: 1,
        borderRadius: "lg",
      },
    },
    children,
  )

const runControlButton = (
  action: (typeof fleetRunControlActions)[number],
  runControlTargetRef: string | null,
): ButtonView =>
  Button({
    key: `cockpit-run-control-${action}`,
    label: action,
    variant: action === "stop" ? "secondary" : "primary",
    disabled: runControlTargetRef === null,
    onPress: IntentRef(
      "CockpitRunControl",
      StaticPayload({
        action,
        ...(runControlTargetRef === null ? {} : { runRef: runControlTargetRef }),
      }),
    ),
    style: {
      backgroundColor: action === "stop" ? "surface" : "accent",
      borderColor: "border",
      borderRadius: "md",
      borderWidth: 1,
      color: "textPrimary",
      fontWeight: "semibold",
      paddingTop: "2",
      paddingBottom: "2",
      paddingLeft: "4",
      paddingRight: "4",
      typeScale: "label",
    },
  })

const workerSelectButton = (
  workerKind: (typeof fleetWorkerKinds)[number],
  runControlTargetRef: string | null,
): ButtonView =>
  Button({
    key: `cockpit-worker-select-${workerKind}`,
    label: workerKind,
    variant: "ghost",
    onPress: IntentRef(
      "CockpitWorkerSelect",
      StaticPayload({
        workerKind,
        ...(runControlTargetRef === null ? {} : { runRef: runControlTargetRef }),
      }),
    ),
    style: {
      backgroundColor: "surface",
      borderColor: "border",
      borderRadius: "full",
      borderWidth: 1,
      color: "textPrimary",
      fontWeight: "medium",
      paddingTop: "1",
      paddingBottom: "1",
      paddingLeft: "3",
      paddingRight: "3",
      typeScale: "label",
    },
  })

const capacityChip = (
  chip: EnCockpitState["capacityChips"][number],
): CardView =>
  Card(
    {
      key: `cockpit-chip-${chip.key}`,
      padding: "3",
      radius: "md",
      style: {
        backgroundColor: "background",
        borderColor: "border",
        borderWidth: 1,
        flex: 1,
        minWidth: "xs",
      },
    },
    [
      text(`cockpit-chip-${chip.key}-value`, chip.value, "title", toneColor(chip.tone)),
      text(`cockpit-chip-${chip.key}-label`, chip.label, "caption", "textMuted"),
    ],
  )

const harnessRow = (
  row: EnCockpitState["harnessRows"][number],
): CardView =>
  Card(
    {
      key: `cockpit-${row.key}`,
      padding: "3",
      radius: "md",
      style: {
        backgroundColor: "background",
        borderColor: "border",
        borderWidth: 1,
        width: "full",
      },
    },
    [
      Stack(
        {
          key: `cockpit-${row.key}-line`,
          direction: "row",
          align: "center",
          justify: "between",
          gap: "3",
          style: { width: "full" },
        },
        [
          Stack(
            {
              key: `cockpit-${row.key}-id`,
              direction: "row",
              align: "center",
              gap: "2",
            },
            [
              text(`cockpit-${row.key}-harness`, row.harnessKind, "label", "accent"),
              text(`cockpit-${row.key}-account`, row.accountRef, "body", "textPrimary"),
            ],
          ),
          text(
            `cockpit-${row.key}-readiness`,
            row.paused ? `${row.readinessLabel} (paused)` : row.readinessLabel,
            "label",
            toneColor(row.tone),
          ),
        ],
      ),
    ],
  )

const runRow = (row: EnCockpitState["runRows"][number]): CardView =>
  Card(
    {
      key: `cockpit-${row.key}`,
      padding: "3",
      radius: "md",
      style: {
        backgroundColor: "background",
        borderColor: "border",
        borderWidth: 1,
        width: "full",
      },
    },
    [
      Stack(
        {
          key: `cockpit-${row.key}-line`,
          direction: "row",
          align: "center",
          justify: "between",
          gap: "3",
          style: { width: "full" },
        },
        [
          text(`cockpit-${row.key}-issue`, row.issueRef, "body", "textPrimary"),
          text(`cockpit-${row.key}-status`, row.statusLabel, "label", toneColor(row.tone)),
        ],
      ),
      text(`cockpit-${row.key}-elapsed`, row.elapsedLabel, "caption", "textMuted"),
    ],
  )

const approvalRow = (
  row: EnCockpitState["pendingApprovals"][number],
): CardView =>
  Card(
    {
      key: `cockpit-${row.key}`,
      padding: "3",
      radius: "md",
      style: {
        backgroundColor: "background",
        borderColor: "focus",
        borderWidth: 1,
        width: "full",
      },
    },
    [
      text(`cockpit-${row.key}-issue`, row.issueRef, "body", "textPrimary"),
      text(`cockpit-${row.key}-detail`, row.detail, "caption", "textMuted"),
      Stack(
        {
          key: `cockpit-${row.key}-actions`,
          direction: "row",
          gap: "2",
          style: { width: "full" },
        },
        [
          Button({
            key: `cockpit-${row.key}-allow`,
            label: "allow",
            variant: "primary",
            onPress: IntentRef(
              "CockpitApprovalDecision",
              StaticPayload({ approvalRef: row.approvalRef, decision: "allow" }),
            ),
            style: {
              backgroundColor: "accent",
              borderColor: "border",
              borderRadius: "md",
              borderWidth: 1,
              color: "textPrimary",
              fontWeight: "semibold",
              paddingTop: "1",
              paddingBottom: "1",
              paddingLeft: "3",
              paddingRight: "3",
              typeScale: "label",
            },
          }),
          Button({
            key: `cockpit-${row.key}-deny`,
            label: "deny",
            variant: "secondary",
            onPress: IntentRef(
              "CockpitApprovalDecision",
              StaticPayload({ approvalRef: row.approvalRef, decision: "deny" }),
            ),
            style: {
              backgroundColor: "surface",
              borderColor: "danger",
              borderRadius: "md",
              borderWidth: 1,
              color: "danger",
              fontWeight: "semibold",
              paddingTop: "1",
              paddingBottom: "1",
              paddingLeft: "3",
              paddingRight: "3",
              typeScale: "label",
            },
          }),
        ],
      ),
    ],
  )

const emptyNote = (key: string, message: string): CardView =>
  Card(
    {
      key,
      padding: "3",
      radius: "md",
      style: {
        backgroundColor: "background",
        borderColor: "border",
        borderWidth: 1,
        width: "full",
      },
    },
    [text(`${key}-copy`, message, "body", "textMuted")],
  )

export const enCockpitView = (state: EnCockpitState): View =>
  Stack(
    {
      key: "cockpit-root",
      direction: "column",
      gap: "4",
      padding: "5",
      style: {
        backgroundColor: "background",
        minHeight: "full",
        width: "full",
      },
    },
    [
      Stack(
        {
          key: "cockpit-header",
          direction: "row",
          align: "center",
          justify: "between",
          gap: "3",
          style: { width: "full" },
        },
        [
          text("cockpit-title", "Fleet cockpit", "heading", "textPrimary"),
          text(
            "cockpit-pylon",
            `pylon ${state.pylonStatusLabel}`,
            "label",
            state.pylonStatusLabel === "offline" ? "danger" : "accent",
          ),
        ],
      ),
      Stack(
        {
          key: "cockpit-chip-strip",
          direction: "row",
          gap: "3",
          style: { width: "full" },
        },
        state.capacityChips.map(capacityChip),
      ),
      section("cockpit-controls-section", [
        text("cockpit-controls-title", "Run controls", "title", "textPrimary"),
        Stack(
          {
            key: "cockpit-run-controls",
            direction: "row",
            gap: "2",
            style: { width: "full" },
          },
          fleetRunControlActions.map((action) =>
            runControlButton(action, state.runControlTargetRef),
          ),
        ),
        text("cockpit-worker-select-title", "Worker selection", "label", "textMuted"),
        Stack(
          {
            key: "cockpit-worker-select",
            direction: "row",
            gap: "2",
            style: { width: "full" },
          },
          fleetWorkerKinds.map((workerKind) =>
            workerSelectButton(workerKind, state.runControlTargetRef),
          ),
        ),
      ]),
      section("cockpit-harness-section", [
        text("cockpit-harness-title", "Harness readiness", "title", "textPrimary"),
        List(
          { key: "cockpit-harness-list", style: { gap: "2" } },
          state.harnessRows.length > 0
            ? state.harnessRows.map((row) => keyed(harnessRow(row)))
            : [keyed(emptyNote("cockpit-harness-empty", "No harness accounts connected yet."))],
        ),
      ]),
      section("cockpit-runs-section", [
        text("cockpit-runs-title", "Workers and runs", "title", "textPrimary"),
        List(
          { key: "cockpit-runs-list", style: { gap: "2" } },
          state.runRows.length > 0
            ? state.runRows.map((row) => keyed(runRow(row)))
            : [keyed(emptyNote("cockpit-runs-empty", "No active runs right now."))],
        ),
      ]),
      section("cockpit-approvals-section", [
        text("cockpit-approvals-title", "Pending approvals", "title", "textPrimary"),
        List(
          { key: "cockpit-approvals-list", style: { gap: "2" } },
          state.pendingApprovals.length > 0
            ? state.pendingApprovals.map((row) => keyed(approvalRow(row)))
            : [keyed(emptyNote("cockpit-approvals-empty", "No approvals waiting."))],
        ),
      ]),
      Spacer({ key: "cockpit-bottom-space", size: "6" }),
    ],
  )
