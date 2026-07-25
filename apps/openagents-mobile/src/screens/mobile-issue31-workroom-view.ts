import {
  Badge,
  Button,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core";

import type { MobileAccessibilityProfile } from "./khala-core";
import { mobileInteractiveStyle } from "./khala-core";
import {
  issue31RowsForRoom,
  type Issue31CapabilityProjection,
  type Issue31SourceStatus,
  type Issue31WorkroomReadModel,
  type Issue31WorkroomRoom,
} from "../workroom/issue31-workroom-read-model";

const statusLabel: Readonly<Record<Issue31SourceStatus, string>> = {
  ready: "Ready",
  unavailable: "Unavailable",
  gap: "Gap",
};

const statusTone = (
  status: Issue31SourceStatus,
): "neutral" | "info" | "success" | "warn" | "danger" => {
  if (status === "ready") return "success";
  if (status === "gap") return "warn";
  return "neutral";
};

const sourceCopy = (row: Issue31CapabilityProjection): string => {
  const source = row.source;
  if (source.status === "ready") {
    return `${source.authority === "signed_nostr_record" ? "Signed Nostr authority" : "Omega host projection"} · ${source.freshness} · ${source.recordRefs.length} ${source.recordRefs.length === 1 ? "record" : "records"}`;
  }
  return `${statusLabel[source.status]} · ${source.reasonRef ?? "reason.issue31.unknown"}`;
};

const actionCopy = (row: Issue31CapabilityProjection): string => {
  const action = row.source.actionState;
  if (action.kind === "idle") return "Action · idle";
  if (action.kind === "pending") return `Action · pending · ${action.intentRef}`;
  if (action.kind === "refused") return `Action · refused · ${action.decisionRef}`;
  return `Action · ${action.state} · ${action.outcomeRef}`;
};

const capabilityCard = (row: Issue31CapabilityProjection): View =>
  Stack(
    {
      key: `issue31-capability-${row.id}`,
      direction: "column",
      gap: "1",
      padding: "3",
      style: {
        width: "full",
        borderWidth: 1,
        borderColor: "border",
        borderRadius: "lg",
        backgroundColor: "surface",
      },
    },
    [
      Stack(
        {
          key: `issue31-capability-${row.id}-header`,
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          Text({
            key: `issue31-capability-${row.id}-title`,
            content: row.label,
            variant: "heading",
            color: "textPrimary",
            style: { flex: 1 },
          }),
          Badge({
            key: `issue31-capability-${row.id}-status`,
            label: statusLabel[row.source.status],
            tone: statusTone(row.source.status),
          }),
        ],
      ),
      Text({
        key: `issue31-capability-${row.id}-source`,
        content: sourceCopy(row),
        variant: "caption",
        color: "textMuted",
      }),
      Text({
        key: `issue31-capability-${row.id}-action`,
        content: actionCopy(row),
        variant: "caption",
        color:
          row.source.actionState.kind === "refused" ||
          (row.source.actionState.kind === "terminal" &&
            row.source.actionState.state !== "succeeded")
            ? "warning"
            : "textMuted",
      }),
      ...(row.hostObservation === null
        ? []
        : [
            Text({
              key: `issue31-capability-${row.id}-host-observation`,
              content: `Host observation · ${row.hostObservation.projection.freshness} · ${row.hostObservation.projection.gap} · ${row.hostObservation.projection.commandState.kind}`,
              variant: "caption",
              color: "textMuted",
            }),
          ]),
      Text({
        key: `issue31-capability-${row.id}-see`,
        content: `See · ${row.ownerCanSee}`,
        variant: "body",
        color: "textPrimary",
      }),
      Text({
        key: `issue31-capability-${row.id}-do`,
        content: `Do · ${row.permittedUserCanDo}`,
        variant: "body",
        color:
          row.source.role === "none" || row.source.status === "unavailable"
            ? "textMuted"
            : "textPrimary",
      }),
      Text({
        key: `issue31-capability-${row.id}-role`,
        content: `Role · ${row.source.role.replace("_", " ")} (${row.source.roleStatus}) · Source · ${row.source.sourceRef}`,
        variant: "caption",
        color: "textMuted",
      }),
    ],
  );

const roomButton = (
  room: Issue31WorkroomRoom,
  selectedRoom: Issue31WorkroomRoom,
  accessibility: MobileAccessibilityProfile,
): View =>
  Button({
    key: `issue31-room-${room}`,
    label: room === "owner_private" ? "Owner-private" : "Community",
    variant: selectedRoom === room ? "primary" : "secondary",
    onPress: IntentRef("Issue31WorkroomRoomSelected", StaticPayload({ room })),
    a11y: {
      label: `${room === "owner_private" ? "Owner-private Sarah" : "Community"} room${selectedRoom === room ? ", selected" : ""}`,
    },
    style: { flex: 1, ...mobileInteractiveStyle(accessibility) },
  });

export const renderMobileIssue31WorkroomView = (
  model: Issue31WorkroomReadModel,
  selectedRoom: Issue31WorkroomRoom,
  accessibility: MobileAccessibilityProfile,
): View => {
  const rows = issue31RowsForRoom(model, selectedRoom);
  return Stack(
    {
      key: "issue31-workroom",
      direction: "column",
      gap: "3",
      padding: "4",
      a11y: {
        role: "region",
        label: `Omega issue 31 Workroom. ${model.coverage.ready} of ${model.coverage.total} sources ready.`,
      },
      style: { width: "full", height: "full", backgroundColor: "background" },
    },
    [
      Text({
        key: "issue31-workroom-title",
        content: "Workroom",
        variant: "title",
        color: "textPrimary",
      }),
      Text({
        key: "issue31-workroom-summary",
        content: `${model.coverage.ready}/${model.coverage.total} sources ready · ${model.coverage.gaps} gaps · ${model.coverage.pending} pending · ${model.coverage.refused} refused`,
        variant: "body",
        color: model.coverage.ready === model.coverage.total ? "textPrimary" : "textMuted",
      }),
      Stack(
        {
          key: "issue31-room-selector",
          direction: "row",
          gap: "2",
          style: { width: "full" },
        },
        [
          roomButton("owner_private", selectedRoom, accessibility),
          roomButton("community", selectedRoom, accessibility),
        ],
      ),
      Text({
        key: "issue31-room-boundary",
        content:
          selectedRoom === "owner_private"
            ? "Owner-private history, memory, and attention stay separate from community membership and work."
            : "Community history and membership stay separate from the owner-private room. v1 awards experience and pays no money.",
        variant: "caption",
        color: "textMuted",
      }),
      ...rows.map(capabilityCard),
    ],
  );
};
