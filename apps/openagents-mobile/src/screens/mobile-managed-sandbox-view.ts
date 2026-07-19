import {
  Badge,
  Button,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core";
import type {
  ManagedSandboxSupervisionOutcome,
  ManagedSandboxSupervisionProjection,
} from "@openagentsinc/managed-sandbox-contract";

import type {
  MobileManagedSandboxControlAction,
  MobileManagedSandboxSnapshot,
} from "../managed-sandbox/mobile-managed-sandbox";

export type MobileManagedSandboxViewState = Readonly<{
  snapshot: MobileManagedSandboxSnapshot | null;
  pending: Readonly<{
    sandboxRef: string;
    action: MobileManagedSandboxControlAction;
  }> | null;
  lastOutcome: ManagedSandboxSupervisionOutcome | null;
  deleteConfirmRef: string | null;
}>;

const lifecycleTone = (
  projection: ManagedSandboxSupervisionProjection,
): "neutral" | "info" | "success" | "warn" | "danger" => {
  if (projection.attention.state === "recovery_required") return "danger";
  if (projection.attention.state === "needs_action") return "warn";
  switch (projection.state.lifecycle) {
    case "ready":
    case "idle":
      return "success";
    case "running":
    case "provisioning":
    case "resuming":
      return "info";
    case "stopping":
    case "deleting":
      return "warn";
    case "failed":
    case "recovery_required":
      return "danger";
    default:
      return "neutral";
  }
};

const titleCase = (value: string): string =>
  value.replaceAll("_", " ").replace(/^./u, (letter) => letter.toUpperCase());

const duration = (seconds: number): string => {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3_600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3_600)}h ${Math.floor((seconds % 3_600) / 60)}m`;
};

const costCap = (micros: number): string => `$${(micros / 1_000_000).toFixed(2)} cap`;

const actorLabel = (actorRef: string): string => {
  switch (actorRef) {
    case "principal.sarah":
      return "Sarah";
    case "principal.desktop":
      return "Desktop";
    case "principal.mobile":
      return "Mobile";
    case "principal.web":
      return "Web";
    default:
      return "OpenAgents";
  }
};

const availableActions = (
  projection: ManagedSandboxSupervisionProjection,
): ReadonlyArray<MobileManagedSandboxControlAction> => {
  const actions: MobileManagedSandboxControlAction[] = [];
  if (
    projection.runtime !== null &&
    (projection.runtime.status === "running" || projection.runtime.status === "interrupting")
  )
    actions.push("interrupt");
  if (["ready", "idle", "running"].includes(projection.state.lifecycle)) actions.push("stop");
  if (projection.state.lifecycle === "stopped") actions.push("resume");
  if (projection.state.lifecycle !== "deleted" && projection.state.lifecycle !== "deleting") {
    actions.push("delete");
  }
  return actions;
};

const outcomeCopy = (
  outcome: ManagedSandboxSupervisionOutcome | null,
  sandboxRef: string,
): string | null => {
  if (outcome === null || outcome.projection?.sandboxRef !== sandboxRef) return null;
  if (outcome.state === "applied") return `Confirmed · ${outcome.receiptRefs.join(", ")}`;
  if (outcome.state === "pending") return "Queued — awaiting the durable authority outcome.";
  return `${titleCase(outcome.state)} · ${outcome.reasonRef ?? "reason.unknown"}`;
};

const outcomeRefViews = (projection: ManagedSandboxSupervisionProjection): ReadonlyArray<View> => {
  const groups = [
    ["Files", projection.outcomes.fileRefs],
    ["Changes", projection.outcomes.changeRefs],
    ["Artifacts", projection.outcomes.artifactRefs],
    ["Evidence", projection.outcomes.evidenceRefs],
    ["Receipts", projection.outcomes.receiptRefs],
  ] as const;
  return groups.flatMap(([label, refs]) =>
    refs.length === 0
      ? []
      : [
          Text({
            key: `managed-sandbox-${projection.sandboxRef}-${label}`,
            content: `${label}: ${refs.join(", ")}`,
            variant: "caption",
            color: "textMuted",
          }),
        ],
  );
};

const actionViews = (
  projection: ManagedSandboxSupervisionProjection,
  state: MobileManagedSandboxViewState,
): ReadonlyArray<View> => {
  const pending = state.pending?.sandboxRef === projection.sandboxRef ? state.pending.action : null;
  if (state.deleteConfirmRef === projection.sandboxRef) {
    return [
      Text({
        key: `managed-sandbox-${projection.sandboxRef}-delete-warning`,
        content:
          "Delete requests teardown. Completion still requires a zero-residue cleanup receipt.",
        variant: "caption",
        color: "warning",
      }),
      Stack(
        {
          key: `managed-sandbox-${projection.sandboxRef}-delete-actions`,
          direction: "row",
          gap: "2",
          style: { width: "full" },
        },
        [
          Button({
            key: `managed-sandbox-${projection.sandboxRef}-delete-confirm`,
            label: pending === "delete" ? "Requesting delete…" : "Confirm delete",
            variant: "secondary",
            disabled: pending !== null,
            onPress: IntentRef(
              "ManagedSandboxDeleteConfirmed",
              StaticPayload({
                sandboxRef: projection.sandboxRef,
              }),
            ),
          }),
          Button({
            key: `managed-sandbox-${projection.sandboxRef}-delete-cancel`,
            label: "Cancel",
            variant: "ghost",
            disabled: pending !== null,
            onPress: IntentRef(
              "ManagedSandboxDeleteDismissed",
              StaticPayload({
                sandboxRef: projection.sandboxRef,
              }),
            ),
          }),
        ],
      ),
    ];
  }
  const actions = availableActions(projection);
  return actions.length === 0
    ? []
    : [
        Stack(
          {
            key: `managed-sandbox-${projection.sandboxRef}-actions`,
            direction: "row",
            gap: "2",
            style: { width: "full" },
          },
          actions.map((action) =>
            Button({
              key: `managed-sandbox-${projection.sandboxRef}-${action}`,
              label:
                pending === action
                  ? `${titleCase(action)}…`
                  : action === "delete"
                    ? "Delete…"
                    : titleCase(action),
              variant:
                action === "resume" ? "primary" : action === "delete" ? "ghost" : "secondary",
              disabled: pending !== null,
              onPress: IntentRef(
                action === "delete"
                  ? "ManagedSandboxDeleteRequested"
                  : "ManagedSandboxControlRequested",
                StaticPayload({ sandboxRef: projection.sandboxRef, action }),
              ),
            }),
          ),
        ),
      ];
};

const panel = (
  projection: ManagedSandboxSupervisionProjection,
  state: MobileManagedSandboxViewState,
): View => {
  const runtime = projection.runtime;
  const lastOutcome = outcomeCopy(state.lastOutcome, projection.sandboxRef);
  const facts = [
    `${projection.target.region} · ${titleCase(projection.target.isolation)}`,
    `generation ${projection.resourceGeneration} · attachment ${projection.attachmentGeneration}`,
    `elapsed ${duration(projection.timing.elapsedSeconds)} · idle ${duration(projection.timing.idleSeconds)}`,
    `lease ${titleCase(projection.timing.leaseState)} until ${projection.timing.leaseExpiresAt}`,
    `${costCap(projection.budget.maxCostMicros)} · cost ${projection.budget.state.replaceAll("_", " ")}`,
  ];
  return Stack(
    {
      key: `managed-sandbox-${projection.sandboxRef}`,
      direction: "column",
      gap: "1",
      style: {
        width: "full",
        padding: "2",
        borderColor: "border",
        borderWidth: 1,
        borderRadius: "lg",
      },
      a11y: {
        role: "region",
        label: `Managed sandbox ${projection.sandboxRef}, ${projection.state.lifecycle}, cleanup ${projection.cleanup.state}`,
      },
    },
    [
      Stack(
        {
          key: `managed-sandbox-${projection.sandboxRef}-heading`,
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          Badge({
            key: `managed-sandbox-${projection.sandboxRef}-lifecycle`,
            label: titleCase(projection.state.lifecycle),
            tone: lifecycleTone(projection),
          }),
          Text({
            key: `managed-sandbox-${projection.sandboxRef}-work`,
            content: projection.workUnitRef,
            variant: "heading",
            color: "textPrimary",
            style: { flex: 1 },
          }),
        ],
      ),
      ...facts.map((fact, index) =>
        Text({
          key: `managed-sandbox-${projection.sandboxRef}-fact-${index}`,
          content: fact,
          variant: "caption",
          color: "textMuted",
        }),
      ),
      ...(runtime === null
        ? []
        : [
            Text({
              key: `managed-sandbox-${projection.sandboxRef}-runtime`,
              content: `${actorLabel(runtime.actorRef)} · ${runtime.identity.provider} · ${runtime.identity.modelRef} · ${runtime.identity.harnessRef} · ${runtime.status}`,
              variant: "body",
              color: "textPrimary",
            }),
          ]),
      ...(projection.lastStructuralEvent === null
        ? []
        : [
            Text({
              key: `managed-sandbox-${projection.sandboxRef}-event`,
              content: `Last event ${projection.lastStructuralEvent.kind} #${projection.lastStructuralEvent.sequence}`,
              variant: "caption",
              color: "textMuted",
            }),
          ]),
      Text({
        key: `managed-sandbox-${projection.sandboxRef}-cleanup`,
        content: `Cleanup ${projection.cleanup.state.replaceAll("_", " ")}${projection.cleanup.receiptRef === null ? "" : ` · ${projection.cleanup.receiptRef}`}`,
        variant: "caption",
        color: projection.cleanup.state === "recovery_required" ? "danger" : "textMuted",
      }),
      ...outcomeRefViews(projection),
      ...(lastOutcome === null
        ? []
        : [
            Text({
              key: `managed-sandbox-${projection.sandboxRef}-outcome`,
              content: lastOutcome,
              variant: "caption",
              color: state.lastOutcome?.state === "applied" ? "success" : "warning",
            }),
          ]),
      ...actionViews(projection, state),
    ],
  );
};

export const renderMobileManagedSandboxViews = (
  state: MobileManagedSandboxViewState,
): ReadonlyArray<View> => {
  if (state.snapshot === null || state.snapshot.state === "unauthorized") return [];
  if (state.snapshot.state === "unavailable") {
    return [
      Text({
        key: "managed-sandbox-unavailable",
        content:
          "Managed sandbox status is unavailable. Queued controls will reconcile after reconnect.",
        variant: "caption",
        color: "warning",
      }),
    ];
  }
  const visible = state.snapshot.envelope.projections.slice(0, 3);
  if (visible.length === 0) return [];
  return [
    Text({
      key: "managed-sandbox-heading",
      content: "Managed agents",
      variant: "heading",
      color: "textPrimary",
    }),
    ...visible.map((projection) => panel(projection, state)),
    ...(state.snapshot.envelope.projections.length <= visible.length
      ? []
      : [
          Text({
            key: "managed-sandbox-omitted",
            content: `${state.snapshot.envelope.projections.length - visible.length} more managed sandboxes available in the controller directory.`,
            variant: "caption",
            color: "textMuted",
          }),
        ]),
  ];
};
