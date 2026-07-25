import {
  Badge,
  Button,
  ComponentValueBinding,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  TextField,
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
import type { Issue31MobileNostrControlState } from "../workroom/issue31-mobile-nostr-runtime";
import { searchIssue31LocalMemory } from "../workroom/issue31-owner-private-read-model";

export interface Issue31OwnerPrivateViewState {
  readonly draft: string;
  readonly memoryQuery: string;
  readonly reminderDraft: string;
  readonly transcriptLimit: number;
  readonly notice: string | null;
}

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

const recordRefsCopy = (recordRefs: ReadonlyArray<string>): string => {
  const displayed = recordRefs.slice(0, 8);
  const hiddenCount = recordRefs.length - displayed.length;
  return `Records · ${displayed.join(" · ")}${hiddenCount === 0 ? "" : ` · +${hiddenCount} more`}`;
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
      ...(row.source.recordRefs.length === 0
        ? []
        : [
            Text({
              key: `issue31-capability-${row.id}-record-refs`,
              content: recordRefsCopy(row.source.recordRefs),
              variant: "caption",
              color: "textMuted",
            }),
          ]),
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

const ownerPrivateDetail = (
  model: Issue31WorkroomReadModel,
  control: Issue31MobileNostrControlState,
  state: Issue31OwnerPrivateViewState,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => {
  const owner = model.ownerPrivate;
  const transcript = owner.transcript.slice(-state.transcriptLimit);
  const memory = searchIssue31LocalMemory(owner.memory, state.memoryQuery);
  const activeTurn = [...owner.activity].reverse().find((row) => !row.terminal);
  const pairedToV2 =
    control.phase === "paired" &&
    control.hosts.some(
      (host) =>
        host.hostPublicKeyHex === control.selectedHostPublicKeyHex &&
        host.supportsCommandV2 === true &&
        host.conversation !== null,
    );
  return [
    Text({
      key: "issue31-owner-status",
      content:
        owner.status === "ready"
          ? `Owner-private source ready · generation ${owner.generation ?? "unknown"}`
          : `${owner.status === "gap" ? "Owner-private gap" : "Owner-private unavailable"} · ${owner.reasonRef ?? "reason.issue31.unknown"}`,
      variant: "body",
      color: owner.status === "ready" ? "textPrimary" : "warning",
    }),
    Text({ key: "issue31-owner-transcript-title", content: "Conversation", variant: "heading" }),
    ...(transcript.length === 0
      ? [
          Text({
            key: "issue31-owner-transcript-empty",
            content: "No confirmed owner-private messages are stored on this device.",
            variant: "body",
            color: "textMuted",
          }),
        ]
      : transcript.flatMap((row) => [
          Stack(
            {
              key: `issue31-owner-message-${row.sourceEventId}`,
              direction: "column",
              gap: "1",
              padding: "3",
              style: { width: "full", borderWidth: 1, borderColor: "border", borderRadius: "lg" },
            },
            [
              Text({
                key: `issue31-owner-message-${row.sourceEventId}-role`,
                content: `${row.role === "owner" ? "You" : "Sarah"} · ${row.sourceCreatedAt}`,
                variant: "caption",
                color: "textMuted",
              }),
              Text({
                key: `issue31-owner-message-${row.sourceEventId}-text`,
                content: row.text,
                variant: "body",
                color: "textPrimary",
              }),
              Button({
                key: `issue31-owner-message-${row.sourceEventId}-read`,
                label: "Mark read here",
                variant: "ghost",
                disabled: !pairedToV2,
                onPress: IntentRef(
                  "Issue31OwnerMarkReadRequested",
                  StaticPayload({ sourceEventId: row.sourceEventId }),
                ),
                style: mobileInteractiveStyle(accessibility),
              }),
            ],
          ),
        ])),
    ...(owner.transcriptTotal <= transcript.length
      ? []
      : [
          Button({
            key: "issue31-owner-transcript-earlier",
            label: `Load earlier (${owner.transcriptTotal - transcript.length} remaining)`,
            variant: "secondary",
            onPress: IntentRef("Issue31OwnerTranscriptEarlierRequested", StaticPayload({})),
            style: mobileInteractiveStyle(accessibility),
          }),
        ]),
    TextField({
      key: "issue31-owner-composer",
      value: state.draft,
      placeholder: "Message Sarah through your Omega host",
      multiline: true,
      disabled: !pairedToV2,
      onChange: IntentRef("Issue31OwnerDraftChanged", ComponentValueBinding()),
      a11y: { label: "Owner-private message" },
      style: { width: "full", minHeight: 96 },
    }),
    Button({
      key: "issue31-owner-send",
      label: "Send signed message",
      variant: "primary",
      disabled: !pairedToV2 || state.draft.trim() === "",
      onPress: IntentRef("Issue31OwnerSendRequested", StaticPayload({})),
      style: { width: "full", ...mobileInteractiveStyle(accessibility) },
    }),
    ...(state.notice === null
      ? []
      : [
          Text({
            key: "issue31-owner-command-notice",
            content: state.notice,
            variant: "caption",
            color:
              state.notice.includes("could not") || state.notice.includes("unavailable")
                ? "warning"
                : "textMuted",
          }),
        ]),
    Text({ key: "issue31-owner-activity-title", content: "Live activity", variant: "heading" }),
    ...owner.activity.slice(-20).map((row) =>
      Text({
        key: `issue31-owner-activity-${row.sourceEventId}`,
        content: `${row.sequence} · ${row.label}`,
        variant: "caption",
        color: row.terminal ? "textMuted" : "textPrimary",
      }),
    ),
    ...(activeTurn === undefined
      ? []
      : [
          Button({
            key: `issue31-owner-interrupt-${activeTurn.turnRef}`,
            label: `Interrupt ${activeTurn.turnRef}`,
            variant: "secondary",
            disabled: !pairedToV2,
            onPress: IntentRef(
              "Issue31OwnerInterruptRequested",
              StaticPayload({
                turnRef: activeTurn.turnRef,
                conversation:
                  control.hosts.find(
                    (host) => host.hostPublicKeyHex === control.selectedHostPublicKeyHex,
                  )?.conversation ?? "",
              }),
            ),
            style: mobileInteractiveStyle(accessibility),
          }),
        ]),
    Text({
      key: "issue31-owner-receipts-title",
      content: "Authority receipts",
      variant: "heading",
    }),
    ...(owner.receipts.length === 0
      ? [
          Text({
            key: "issue31-owner-receipts-empty",
            content: "No confirmed authority receipts.",
            variant: "body",
            color: "textMuted",
          }),
        ]
      : owner.receipts.map((row) =>
          Button({
            key: `issue31-owner-receipt-${row.sourceEventId}`,
            label: `${row.authorityState} · ${row.targetState} · ${row.receiptRef}`,
            variant: "ghost",
            onPress: IntentRef("Issue31OwnerDeepLinkOpened", StaticPayload({ url: row.deepLink })),
            a11y: { label: `Authority ${row.authorityState}, target ${row.targetState}` },
            style: { width: "full", ...mobileInteractiveStyle(accessibility) },
          }),
        )),
    Text({ key: "issue31-owner-memory-title", content: "Local memory", variant: "heading" }),
    TextField({
      key: "issue31-owner-memory-search",
      value: state.memoryQuery,
      placeholder: "Search decrypted memory on this device",
      onChange: IntentRef("Issue31OwnerMemoryQueryChanged", ComponentValueBinding()),
      a11y: { label: "Search local owner-private memory" },
      style: { width: "full", ...mobileInteractiveStyle(accessibility) },
    }),
    ...memory.map((row) =>
      Text({
        key: `issue31-owner-memory-${row.sourceEventId}`,
        content: JSON.stringify(row.body),
        variant: "caption",
        color: "textPrimary",
      }),
    ),
    Text({ key: "issue31-owner-read-title", content: "Read state", variant: "heading" }),
    ...Object.entries(owner.readContexts)
      .slice(0, 32)
      .map(([contextRef, readAt]) =>
        Text({
          key: `issue31-owner-read-${contextRef}`,
          content: `${contextRef} · ${readAt}`,
          variant: "caption",
          color: "textMuted",
        }),
      ),
    Text({ key: "issue31-owner-reminders-title", content: "Reminders", variant: "heading" }),
    TextField({
      key: "issue31-owner-reminder-draft",
      value: state.reminderDraft,
      placeholder: "Reminder note or replacement note",
      disabled: !pairedToV2,
      onChange: IntentRef("Issue31OwnerReminderDraftChanged", ComponentValueBinding()),
      a11y: { label: "Reminder note" },
      style: { width: "full", ...mobileInteractiveStyle(accessibility) },
    }),
    Button({
      key: "issue31-owner-reminder-create",
      label: "Create reminder now",
      variant: "secondary",
      disabled: !pairedToV2 || state.reminderDraft.trim() === "",
      onPress: IntentRef("Issue31OwnerReminderCreated", StaticPayload({})),
      style: mobileInteractiveStyle(accessibility),
    }),
    ...owner.reminders.map((row) =>
      Stack(
        {
          key: `issue31-owner-reminder-${row.reminderId}`,
          direction: "column",
          gap: "1",
          padding: "3",
          style: { width: "full", borderWidth: 1, borderColor: "border", borderRadius: "lg" },
        },
        [
          Text({
            key: `issue31-owner-reminder-${row.reminderId}-copy`,
            content: `${row.content.status} · ${row.content.note ?? "No note"} · ${row.notBefore ?? "no schedule"}`,
            variant: "body",
            color: "textPrimary",
          }),
          Stack(
            {
              key: `issue31-owner-reminder-${row.reminderId}-controls`,
              direction: "row",
              gap: "2",
            },
            [
              Button({
                key: `issue31-owner-reminder-${row.reminderId}-change`,
                label: "Change",
                variant: "ghost",
                disabled:
                  !pairedToV2 ||
                  row.content.status !== "pending" ||
                  row.notBefore === null ||
                  state.reminderDraft.trim() === "",
                onPress: IntentRef(
                  "Issue31OwnerReminderChanged",
                  StaticPayload({ reminderId: row.reminderId }),
                ),
                style: mobileInteractiveStyle(accessibility),
              }),
              Button({
                key: `issue31-owner-reminder-${row.reminderId}-complete`,
                label: "Complete",
                variant: "ghost",
                disabled: !pairedToV2 || row.content.status !== "pending",
                onPress: IntentRef(
                  "Issue31OwnerReminderCompleted",
                  StaticPayload({ reminderId: row.reminderId }),
                ),
                style: mobileInteractiveStyle(accessibility),
              }),
              Button({
                key: `issue31-owner-reminder-${row.reminderId}-cancel`,
                label: "Cancel",
                variant: "ghost",
                disabled: !pairedToV2 || row.content.status !== "pending",
                onPress: IntentRef(
                  "Issue31OwnerReminderCancelled",
                  StaticPayload({ reminderId: row.reminderId }),
                ),
                style: mobileInteractiveStyle(accessibility),
              }),
            ],
          ),
        ],
      ),
    ),
    Text({
      key: "issue31-owner-commands-title",
      content: "Command reconciliation",
      variant: "heading",
    }),
    ...owner.commands.map((command) =>
      Text({
        key: `issue31-owner-command-${command.intentEventId}`,
        content: `${command.state} · ${command.actionRef} · ${command.idempotencyRef}`,
        variant: "caption",
        color:
          command.state === "failed" ||
          command.state === "refused" ||
          command.state === "unavailable"
            ? "warning"
            : "textMuted",
      }),
    ),
    Button({
      key: "issue31-owner-clear-local",
      label: "Clear local owner-private data",
      variant: "ghost",
      onPress: IntentRef("Issue31OwnerLocalDataCleared", StaticPayload({})),
      a11y: { label: "Clear decrypted owner-private projections stored on this device" },
      style: mobileInteractiveStyle(accessibility),
    }),
  ];
};

export const renderMobileIssue31WorkroomView = (
  model: Issue31WorkroomReadModel,
  selectedRoom: Issue31WorkroomRoom,
  nostrControl: Issue31MobileNostrControlState,
  accessibility: MobileAccessibilityProfile,
  ownerState: Issue31OwnerPrivateViewState,
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
      Text({
        key: "issue31-device-identity",
        content:
          nostrControl.deviceNpub === null
            ? "Device identity · unavailable"
            : `Device identity · ${nostrControl.deviceNpub}`,
        variant: "caption",
        color: "textMuted",
      }),
      ...(nostrControl.hosts.length === 0
        ? [
            Text({
              key: "issue31-host-discovery-empty",
              content:
                "No unexpired announcement from an out-of-band admitted Omega host has arrived yet.",
              variant: "body",
              color: "textMuted",
            }),
          ]
        : nostrControl.hosts.map((host) =>
            Button({
              key: `issue31-host-${host.hostPublicKeyHex}`,
              label: `${host.displayName} · ${host.hostFingerprint} · generation ${host.generation} · Sarah ${host.sarahFingerprint}`,
              variant:
                nostrControl.selectedHostPublicKeyHex === host.hostPublicKeyHex
                  ? "primary"
                  : "secondary",
              onPress: IntentRef(
                "Issue31HostSelected",
                StaticPayload({ hostPublicKeyHex: host.hostPublicKeyHex }),
              ),
              a11y: {
                label: `Confirm admitted Omega host ${host.displayName}, fingerprint ${host.hostFingerprint}, bound Sarah fingerprint ${host.sarahFingerprint}`,
              },
              style: mobileInteractiveStyle(accessibility),
            }),
          )),
      Button({
        key: "issue31-pair-device",
        label:
          nostrControl.phase === "paired"
            ? "Device paired"
            : nostrControl.phase === "pairing" || nostrControl.phase === "awaiting_grant"
              ? "Pairing in progress"
              : "Pair this device",
        variant: "primary",
        disabled:
          nostrControl.selectedHostPublicKeyHex === null ||
          nostrControl.phase === "pairing" ||
          nostrControl.phase === "awaiting_grant" ||
          nostrControl.phase === "paired",
        onPress: IntentRef("Issue31PairingRequested", StaticPayload({})),
        a11y: { label: "Pair this device with the selected signed Omega host" },
        style: mobileInteractiveStyle(accessibility),
      }),
      ...(nostrControl.notice === null
        ? []
        : [
            Text({
              key: "issue31-pairing-notice",
              content: nostrControl.notice,
              variant: "caption",
              color: nostrControl.phase === "failed" ? "warning" : "textMuted",
            }),
          ]),
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
      ...(selectedRoom === "owner_private"
        ? ownerPrivateDetail(model, nostrControl, ownerState, accessibility)
        : []),
      ...rows.map(capabilityCard),
    ],
  );
};
