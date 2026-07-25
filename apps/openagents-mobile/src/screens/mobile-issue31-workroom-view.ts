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
import type {
  Issue31CommunityControlKind,
  Issue31CommunityReadModel,
  Issue31CommunityWorkUnitRow,
} from "../workroom/issue31-community-read-model";
import {
  searchIssue31LocalMemory,
  type Issue31OwnerCommandState,
} from "../workroom/issue31-owner-private-read-model";
import type {
  Issue31EvidenceRow,
  Issue31FullAutoReadModel,
  Issue31FullAutoRunRow,
  Issue31ProviderAccountRow,
  Issue31ProviderHandoffRow,
} from "../workroom/issue31-full-auto-read-model";
import {
  issue31FullAutoControlIsInFlight,
  issue31FullAutoControlSettlementCopy,
  settleIssue31FullAutoControl,
} from "../workroom/issue31-full-auto-control-settlement";

export interface Issue31CommunityViewState {
  readonly draft: string
  readonly subject: string
  readonly appealDraft: string
  readonly notice: string | null
}

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

/**
 * Exact duration copy from the host's own measurement (omega#47).
 *
 * The phone never reads a clock here. A run's unattended time is what the Omega
 * host measured and sent, so it cannot drift with the device's own time or keep
 * counting after the projection went stale.
 */
const unattendedCopy = (unattendedMs: number): string => {
  const totalSeconds = Math.floor(unattendedMs / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const padded = (value: number): string => (value < 10 ? `0${value}` : `${value}`);
  return `${padded(hours)}:${padded(minutes)}:${padded(seconds)} unattended`;
};

const evidenceUnavailableCopy: Readonly<Record<string, string>> = {
  hop_missing: "a step of the chain is missing",
  hop_mismatched: "two records disagree about this run",
  hop_private: "a step cannot be shown on this device",
  self_reported: "the run reported its own success",
  host_unavailable: "the host could not be reached",
};

/**
 * One finished unit from objective through authority receipt, or an explicit
 * statement that it cannot be followed.
 *
 * There is no third rendering. A partial hop list would let the owner read a
 * broken chain as partial proof, which is the exact failure omega#47 forbids.
 */
const evidenceBlock = (runRef: string, evidence: Issue31EvidenceRow): ReadonlyArray<View> => {
  if (evidence.state === "unavailable") {
    return [
      Text({
        key: `issue31-fa-${runRef}-evidence`,
        content: `Evidence unavailable · ${evidenceUnavailableCopy[evidence.reasonClass] ?? evidence.reasonClass}${
          evidence.brokenAt === null ? "" : ` · first broken at ${evidence.brokenAt}`
        }`,
        variant: "body",
        color: "warning",
      }),
    ];
  }
  return [
    Text({
      key: `issue31-fa-${runRef}-evidence`,
      content: `Evidence complete · host verified · authority ${
        evidence.authorityAllowed ? "allowed" : "refused"
      }`,
      variant: "body",
      color: evidence.authorityAllowed ? "textPrimary" : "warning",
    }),
    // The workroom receipt-inspector grammar: one label, one reference, in the
    // order the work actually happened.
    ...evidence.hops.map((hop) =>
      Text({
        key: `issue31-fa-${runRef}-hop-${hop.kind}`,
        content: `${hop.kind} · ${hop.ref}${hop.detail === null ? "" : ` · ${hop.detail}`}`,
        variant: "caption",
        color: "textMuted",
      }),
    ),
  ];
};

const runCard = (
  run: Issue31FullAutoRunRow,
  commands: ReadonlyArray<Issue31OwnerCommandState>,
  accessibility: MobileAccessibilityProfile,
): View =>
  Stack(
    {
      key: `issue31-fa-run-${run.runRef}`,
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
          key: `issue31-fa-run-${run.runRef}-header`,
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          Text({
            key: `issue31-fa-run-${run.runRef}-objective`,
            content: run.objective,
            variant: "heading",
            color: "textPrimary",
            style: { flex: 1 },
          }),
          Badge({
            key: `issue31-fa-run-${run.runRef}-lifecycle`,
            label: run.lifecycle,
            tone: run.isTerminal
              ? run.lifecycle === "succeeded"
                ? "success"
                : "warn"
              : run.lifecycle === "stalled" || run.lifecycle === "retrying"
                ? "warn"
                : "info",
          }),
        ],
      ),
      Text({
        key: `issue31-fa-run-${run.runRef}-lane`,
        content: `Lane · ${run.laneRef} · ${unattendedCopy(run.unattendedMs)}`,
        variant: "caption",
        color: "textMuted",
      }),
      Text({
        key: `issue31-fa-run-${run.runRef}-work`,
        content: run.isTerminal
          ? `Ended · ${run.terminalReasonRef ?? "reason.issue31.unknown"}`
          : `Working on · ${run.liveWorkRef ?? "no unit reported"}`,
        variant: "body",
        color: "textPrimary",
      }),
      ...(run.controls.length === 0
        ? [
            Text({
              key: `issue31-fa-run-${run.runRef}-no-controls`,
              content: run.isTerminal
                ? "This run has finished. No control can change it."
                : "Your host is not offering a control for this run.",
              variant: "caption",
              color: "textMuted",
            }),
          ]
        : run.controls.flatMap((control) => {
            const settlement = settleIssue31FullAutoControl(control, commands);
            return [
              Button({
                key: `issue31-fa-control-${control.idempotencyRef}`,
                label: `${control.kind} · generation ${control.runGeneration}`,
                variant: "secondary",
                // Disabled while the host holds it, so the owner cannot mint a
                // second command against the same idempotency reference.
                disabled: issue31FullAutoControlIsInFlight(settlement),
                onPress: IntentRef(
                  "Issue31FullAutoControlRequested",
                  StaticPayload({
                    runRef: run.runRef,
                    actionRef: control.actionRef,
                    kind: control.kind,
                    runGeneration: control.runGeneration,
                    idempotencyRef: control.idempotencyRef,
                  }),
                ),
                a11y: {
                  label: `${control.kind} Full Auto run, bound to generation ${control.runGeneration}`,
                },
                style: { width: "full", ...mobileInteractiveStyle(accessibility) },
              }),
              Text({
                key: `issue31-fa-control-${control.idempotencyRef}-settlement`,
                // Never "done" until an Omega-owned terminal result says so.
                content: issue31FullAutoControlSettlementCopy(settlement),
                variant: "caption",
                color: settlement.state === "completed" ? "textPrimary" : "textMuted",
              }),
            ];
          })),
      ...evidenceBlock(run.runRef, run.evidence),
    ],
  );

const handoffCopy = (handoff: Issue31ProviderHandoffRow): string => {
  if (!handoff.isTerminal) {
    return `Connection ${handoff.state} · your Omega host owns the login`;
  }
  return `Connection ${handoff.state} · ${handoff.outcomeRef ?? "no outcome"}${
    handoff.reasonClass === null ? "" : ` · ${handoff.reasonClass}`
  }`;
};

const accountCard = (account: Issue31ProviderAccountRow): View =>
  Stack(
    {
      key: `issue31-fa-account-${account.accountRef}`,
      direction: "column",
      gap: "1",
      padding: "3",
      style: { width: "full", borderWidth: 1, borderColor: "border", borderRadius: "lg" },
    },
    [
      Stack(
        {
          key: `issue31-fa-account-${account.accountRef}-header`,
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          Text({
            key: `issue31-fa-account-${account.accountRef}-label`,
            content: `${account.label} · ${account.provider}`,
            variant: "body",
            color: "textPrimary",
            style: { flex: 1 },
          }),
          Badge({
            key: `issue31-fa-account-${account.accountRef}-readiness`,
            label: account.readiness,
            tone:
              account.readiness === "ready"
                ? "success"
                : account.readiness === "revoked" || account.readiness === "exhausted"
                  ? "danger"
                  : "warn",
          }),
        ],
      ),
      // A lane is not an account. The relation is stated, never implied by
      // putting the two on the same row and hoping the owner infers it.
      Text({
        key: `issue31-fa-account-${account.accountRef}-lane`,
        content: `Account ${account.accountRef} serves lane ${account.laneRef} · quota ${account.quota} · ${
          account.runRefs.length === 0
            ? "no runs on that lane"
            : `runs ${account.runRefs.join(" · ")}`
        }`,
        variant: "caption",
        color: "textMuted",
      }),
      ...(account.handoff === null
        ? []
        : [
            Text({
              key: `issue31-fa-account-${account.accountRef}-handoff`,
              content: handoffCopy(account.handoff),
              variant: "caption",
              color: account.handoff.state === "completed" ? "textMuted" : "warning",
            }),
          ]),
    ],
  );

const fullAutoUnavailableCopy: Readonly<Record<string, string>> = {
  no_host_projection: "This device is not paired to an Omega host yet.",
  host_projection_unreadable:
    "Your Omega host sent a Full Auto projection this app refuses to read. Nothing is shown rather than part of it.",
  snapshot_mismatch:
    "The Full Auto detail belongs to a different host snapshot. It is withheld rather than shown as current.",
};

/**
 * Full Auto work, the accounts behind it, and the evidence for it — in the
 * Workroom, beside the conversation. The owner never leaves for another product
 * surface to answer "what is my machine doing and did it really do it".
 */
const fullAutoSection = (
  fullAuto: Issue31FullAutoReadModel,
  commands: ReadonlyArray<Issue31OwnerCommandState>,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => {
  const heading = Text({
    key: "issue31-fa-title",
    content: "Full Auto",
    variant: "heading",
    color: "textPrimary",
  });
  if (fullAuto.state === "unavailable") {
    return [
      heading,
      Text({
        key: "issue31-fa-unavailable",
        content: fullAutoUnavailableCopy[fullAuto.reason] ?? fullAuto.reason,
        variant: "body",
        color: "warning",
      }),
    ];
  }
  return [
    heading,
    Text({
      key: "issue31-fa-binding",
      content: `Host ${fullAuto.hostRef} · snapshot ${fullAuto.snapshotRef}`,
      variant: "caption",
      color: "textMuted",
    }),
    ...(fullAuto.runs.length === 0
      ? [
          Text({
            key: "issue31-fa-runs-empty",
            content: "Your Omega host reports no Full Auto runs.",
            variant: "body",
            color: "textMuted",
          }),
        ]
      : fullAuto.runs.map((run) => runCard(run, commands, accessibility))),
    Text({
      key: "issue31-fa-accounts-title",
      content: "Provider accounts",
      variant: "heading",
      color: "textPrimary",
    }),
    Text({
      key: "issue31-fa-accounts-boundary",
      content:
        "Your Omega host holds every provider login and token. This device asks for a connection and is told the outcome.",
      variant: "caption",
      color: "textMuted",
    }),
    ...(fullAuto.accounts.length === 0
      ? [
          Text({
            key: "issue31-fa-accounts-empty",
            content: "No provider accounts were reported. A capacity lane is not an account.",
            variant: "body",
            color: "textMuted",
          }),
        ]
      : fullAuto.accounts.map(accountCard)),
    Button({
      key: "issue31-fa-connect-provider",
      label: "Connect a provider account on my Omega host",
      variant: "secondary",
      onPress: IntentRef("Issue31ProviderHandoffRequested", StaticPayload({})),
      a11y: {
        label:
          "Ask your Omega host to connect a provider account. The login happens on the host, in an isolated home.",
      },
      style: { width: "full", ...mobileInteractiveStyle(accessibility) },
    }),
    // A handoff the host has not bound to an account stays here rather than
    // being attributed to a working account of the same provider.
    ...fullAuto.unboundHandoffs.map((handoff) =>
      Text({
        key: `issue31-fa-unbound-handoff-${handoff.handoffRef}`,
        content: `${handoff.provider} · ${handoffCopy(handoff)} · not bound to an account`,
        variant: "caption",
        color: "warning",
      }),
    ),
  ];
};

const controlLabel: Readonly<Record<Issue31CommunityControlKind, string>> = {
  invite_member: "Invite a developer to this room",
  revoke_member: "Remove a member from this room",
  attach_agent: "Attach an agent",
  revoke_agent: "Revoke this agent",
  post_message: "Post to the room",
  quote_work_unit: "Quote this work unit",
  submit_result: "Return the result",
  verify_result: "Verify this result",
  file_appeal: "Appeal this rejection",
};

/**
 * What an agent-signed action needs, said plainly.
 *
 * The operator's compute holds the agent key and this phone never does, so
 * these are stated rather than offered. A button here would claim the phone can
 * do something it cannot.
 */
const agentComputeCopy: Readonly<Record<Issue31CommunityControlKind, string>> = {
  attach_agent: "Your agent attaches itself by publishing its persona with its own key, from your compute.",
  quote_work_unit: "Your agent quotes this unit with its own key, from your compute.",
  submit_result: "Your agent returns the result with its own key, from your compute.",
  verify_result: "Your agent verifies this result with its own key, from your compute.",
  invite_member: "",
  revoke_member: "",
  revoke_agent: "",
  post_message: "",
  file_appeal: "",
};

const lifecycleTone = (
  lifecycle: Issue31CommunityWorkUnitRow["lifecycle"],
): "neutral" | "info" | "success" | "warn" | "danger" => {
  if (lifecycle === "expired") return "neutral";
  if (lifecycle === "disputed") return "warn";
  if (lifecycle === "decided" || lifecycle === "ruled") return "success";
  return "info";
};

const communityUnitCard = (
  unit: Issue31CommunityWorkUnitRow,
  appealDraft: string,
  accessibility: MobileAccessibilityProfile,
): View =>
  Stack(
    {
      key: `issue31-community-unit-${unit.unitRef}`,
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
          key: `issue31-community-unit-${unit.unitRef}-header`,
          direction: "row",
          gap: "2",
          align: "center",
          style: { width: "full" },
        },
        [
          Text({
            key: `issue31-community-unit-${unit.unitRef}-ref`,
            content: unit.unitRef,
            variant: "heading",
            color: "textPrimary",
            style: { flex: 1 },
          }),
          Badge({
            key: `issue31-community-unit-${unit.unitRef}-lifecycle`,
            label: unit.lifecycle,
            tone: lifecycleTone(unit.lifecycle),
          }),
        ],
      ),
      // Exact bounds, named. A unit whose target, actions, budget, expiry, or
      // idempotency is vague is a unit nobody can hold to anything.
      Text({
        key: `issue31-community-unit-${unit.unitRef}-bounds`,
        content: `Target ${unit.targetRefs.join(" · ") || "none"} · ${unit.allowedActionRefs.join(" · ") || "no permitted action"}`,
        variant: "caption",
        color: "textMuted",
      }),
      Text({
        key: `issue31-community-unit-${unit.unitRef}-budget`,
        content: `${unit.experienceTierCopy} · ${unit.expired ? "grant expired" : `grant expires at ${unit.expiresAtUnix}`} · idempotency ${unit.idempotencyRef}`,
        variant: "caption",
        color: unit.expired ? "warning" : "textMuted",
      }),
      Text({
        key: `issue31-community-unit-${unit.unitRef}-quotes`,
        content:
          unit.quotes.length === 0
            ? "No quotes yet."
            : `${unit.quotes.length} ${unit.quotes.length === 1 ? "quote" : "quotes"}${
                unit.acceptedProviderPubkey === null
                  ? " · none accepted"
                  : ` · accepted ${unit.acceptedProviderPubkey.slice(0, 12)}…`
              }`,
        variant: "body",
        color: "textPrimary",
      }),
      ...(unit.result === null
        ? []
        : [
            Text({
              key: `issue31-community-unit-${unit.unitRef}-result`,
              content: `Result from ${unit.result.providerPubkey.slice(0, 12)}… · ${unit.result.displaySummary.slice(0, 200) || "no summary"}`,
              variant: "body",
              color: "textPrimary",
            }),
          ]),
      ...(unit.verification === null
        ? []
        : [
            Text({
              key: `issue31-community-unit-${unit.unitRef}-verification`,
              // Producer and verifier must have distinct operators. A
              // verification that fails that is shown as refused, not as proof.
              content: unit.verification.operatorsAreIndependent
                ? `Verified independently · verifier operator ${unit.verification.verifierOperatorPubkey?.slice(0, 12) ?? "unknown"}…`
                : `Verification refused · ${unit.verification.refusalReason === "self_dealing_operators" ? "the producer and the verifier are the same operator" : "the verifier's operator is unknown"}`,
              variant: "body",
              color: unit.verification.operatorsAreIndependent ? "textPrimary" : "warning",
            }),
          ]),
      ...(unit.decision === null
        ? []
        : [
            Text({
              key: `issue31-community-unit-${unit.unitRef}-decision`,
              content:
                unit.decision.outcome === "accepted"
                  ? `Accepted · receipt ${unit.decision.authorityReceiptRef ?? "none"}`
                  : `Rejected · ${unit.decision.reasonClass ?? "no reason class"} · ${unit.decision.displayReasonSummary ?? "no summary"}`,
              variant: "body",
              color: unit.decision.outcome === "accepted" ? "textPrimary" : "warning",
            }),
            // A rejection always names where it goes next.
            ...(unit.decision.appealDestination === null
              ? []
              : [
                  Text({
                    key: `issue31-community-unit-${unit.unitRef}-appeal-destination`,
                    content:
                      unit.decision.appealDestination === "needs_owner.owner_appeal_npub"
                        ? "Appeals go to the owner, but no owner appeal key is registered yet, so a ruling could not be verified."
                        : `Appeal destination · the owner, as arbiter of last resort · ${unit.decision.appealDestination}`,
                    variant: "caption",
                    color: "textMuted",
                  }),
                ]),
          ]),
      ...(unit.appeal === null
        ? []
        : [
            Text({
              key: `issue31-community-unit-${unit.unitRef}-appeal`,
              content: `Appeal open · ${unit.appeal.grounds} · ${unit.appeal.displayGroundsSummary.slice(0, 200)}`,
              variant: "body",
              color: "textPrimary",
            }),
          ]),
      ...(unit.ruling === null
        ? []
        : [
            Text({
              key: `issue31-community-unit-${unit.unitRef}-ruling`,
              content: unit.ruling.authoredByAdmittedOwnerKey
                ? `Owner ruling · ${unit.ruling.outcome}`
                : `A ruling was published by a key that is not the admitted owner appeal identity. It decides nothing.`,
              variant: "body",
              color: unit.ruling.authoredByAdmittedOwnerKey ? "textPrimary" : "warning",
            }),
          ]),
      // Controls are exactly what the projection derived from the signed role,
      // the grant, and the lifecycle state. The view adds none of its own.
      ...unit.controls.flatMap((control): ReadonlyArray<View> =>
        control.signedBy === "agent_compute"
          ? [
              Text({
                key: `issue31-community-control-${control.idempotencyRef}`,
                content: agentComputeCopy[control.kind],
                variant: "caption",
                color: "textMuted",
              }),
            ]
          : control.kind === "file_appeal"
            ? [
                TextField({
                  key: `issue31-community-appeal-draft-${unit.unitRef}`,
                  value: appealDraft,
                  placeholder: "Why this rejection is disputed",
                  multiline: true,
                  onChange: IntentRef(
                    "Issue31CommunityAppealDraftChanged",
                    ComponentValueBinding(),
                  ),
                  a11y: { label: "Appeal reason" },
                  style: { width: "full", minHeight: 72 },
                }),
                Button({
                  key: `issue31-community-control-${control.idempotencyRef}`,
                  label: controlLabel[control.kind],
                  variant: "secondary",
                  disabled: appealDraft.trim() === "",
                  onPress: IntentRef(
                    "Issue31CommunityAppealFiled",
                    StaticPayload({ unitRef: unit.unitRef }),
                  ),
                  style: { width: "full", ...mobileInteractiveStyle(accessibility) },
                }),
              ]
            : [],
      ),
    ],
  );

/**
 * The community room.
 *
 * Renders the transcript, the roster, the attested agents, the work-unit
 * lifecycle, and the recomputed experience total — and offers exactly the
 * controls the projection derived. There is no branch here that can invent a
 * control: everything actionable comes out of `model.controls` or
 * `unit.controls`, so an unauthorized role has nothing to render.
 */
const communityDetail = (
  model: Issue31CommunityReadModel,
  state: Issue31CommunityViewState,
  accessibility: MobileAccessibilityProfile,
): ReadonlyArray<View> => {
  const has = (kind: Issue31CommunityControlKind): boolean =>
    model.controls.some((control) => control.kind === kind);

  if (model.status === "unavailable") {
    return [
      Text({
        key: "issue31-community-unavailable",
        content:
          model.reasonRef === "reason.issue31.community.admin_keys_not_configured"
            ? "This build has no admitted community admin keys, so membership cannot be checked. Nothing is shown rather than something unverified."
            : "The community room is not configured on this build.",
        variant: "body",
        color: "warning",
      }),
    ];
  }

  return [
    Text({
      key: "issue31-community-header",
      content: `${model.groupName ?? model.groupId ?? "Community"} · you are ${model.viewerRole.replace("_", " ")} (${model.viewerRoleStatus})`,
      variant: "heading",
      color: "textPrimary",
    }),
    ...(model.viewerRoleStatus === "revoked"
      ? [
          Text({
            key: "issue31-community-revoked",
            content:
              "Your membership was revoked. You can read what is already on this device and take no further action in this room.",
            variant: "body",
            color: "warning",
          }),
        ]
      : []),
    ...(model.status === "gap"
      ? [
          Text({
            key: "issue31-community-gap",
            content: `Some community records were refused · ${model.reasonRef ?? "unknown"} · ${model.rejectedRecordCount} refused`,
            variant: "caption",
            color: "warning",
          }),
        ]
      : []),

    Text({ key: "issue31-community-transcript-title", content: "Room", variant: "heading" }),
    ...(model.transcript.length === 0
      ? [
          Text({
            key: "issue31-community-transcript-empty",
            content: "No confirmed community messages have arrived on this device.",
            variant: "body",
            color: "textMuted",
          }),
        ]
      : model.transcript.map((row) =>
          Stack(
            {
              key: `issue31-community-message-${row.sourceEventId}`,
              direction: "column",
              gap: "1",
              padding: "3",
              style: { width: "full", borderWidth: 1, borderColor: "border", borderRadius: "lg" },
            },
            [
              Text({
                key: `issue31-community-message-${row.sourceEventId}-author`,
                content: `${row.authorPubkey.slice(0, 12)}… · ${row.authorRole.replace("_", " ")} · ${row.sourceCreatedAt}`,
                variant: "caption",
                color: "textMuted",
              }),
              Text({
                key: `issue31-community-message-${row.sourceEventId}-text`,
                // Display only. The quoted form beside it is the only thing
                // that may reach a model.
                content: row.displayText,
                variant: "body",
                color: "textPrimary",
              }),
            ],
          ),
        )),
    ...(has("post_message")
      ? [
          TextField({
            key: "issue31-community-composer",
            value: state.draft,
            placeholder: "Post to the community room",
            multiline: true,
            onChange: IntentRef("Issue31CommunityDraftChanged", ComponentValueBinding()),
            a11y: { label: "Community message" },
            style: { width: "full", minHeight: 96 },
          }),
          Button({
            key: "issue31-community-post",
            label: controlLabel.post_message,
            variant: "primary",
            disabled: state.draft.trim() === "",
            onPress: IntentRef("Issue31CommunityMessageSent", StaticPayload({})),
            style: { width: "full", ...mobileInteractiveStyle(accessibility) },
          }),
        ]
      : []),
    ...(state.notice === null
      ? []
      : [
          Text({
            key: "issue31-community-notice",
            content: state.notice,
            variant: "caption",
            color: state.notice.includes("Signed and sent") ? "textMuted" : "warning",
          }),
        ]),

    Text({ key: "issue31-community-roster-title", content: "Members", variant: "heading" }),
    ...(model.roster.length === 0
      ? [
          Text({
            key: "issue31-community-roster-empty",
            content: "No signed membership record has arrived for this group.",
            variant: "body",
            color: "textMuted",
          }),
        ]
      : model.roster.map((row) =>
          Text({
            key: `issue31-community-member-${row.operatorPubkey}`,
            content: `${row.operatorPubkey.slice(0, 12)}… · ${row.status} · ${row.role.replace("_", " ")} · ${row.admittedAgentCount}/${row.agentCount} agents admitted`,
            variant: "caption",
            color: row.status === "revoked" ? "textMuted" : "textPrimary",
          }),
        )),
    ...(has("invite_member") || has("revoke_member")
      ? [
          TextField({
            key: "issue31-community-subject",
            value: state.subject,
            placeholder: "Developer public key (64 hex)",
            onChange: IntentRef("Issue31CommunitySubjectChanged", ComponentValueBinding()),
            a11y: { label: "Community member public key" },
            style: { width: "full", ...mobileInteractiveStyle(accessibility) },
          }),
          Stack(
            { key: "issue31-community-admin-controls", direction: "row", gap: "2" },
            [
              ...(has("invite_member")
                ? [
                    Button({
                      key: "issue31-community-invite",
                      label: controlLabel.invite_member,
                      variant: "secondary",
                      disabled: state.subject.trim().length !== 64,
                      onPress: IntentRef("Issue31CommunityMemberInvited", StaticPayload({})),
                      style: mobileInteractiveStyle(accessibility),
                    }),
                  ]
                : []),
              ...(has("revoke_member")
                ? [
                    Button({
                      key: "issue31-community-revoke-member",
                      label: controlLabel.revoke_member,
                      variant: "ghost",
                      disabled: state.subject.trim().length !== 64,
                      onPress: IntentRef("Issue31CommunityMemberRevoked", StaticPayload({})),
                      style: mobileInteractiveStyle(accessibility),
                    }),
                  ]
                : []),
            ],
          ),
        ]
      : []),

    Text({ key: "issue31-community-agents-title", content: "Attested agents", variant: "heading" }),
    Text({
      key: "issue31-community-agents-boundary",
      content:
        "Each agent runs on its operator's own compute with its own credentials. OpenAgents never receives a provider key and never touches an agent home.",
      variant: "caption",
      color: "textMuted",
    }),
    ...(model.agents.length === 0
      ? [
          Text({
            key: "issue31-community-agents-empty",
            content: "No attested agent has been bound in this group.",
            variant: "body",
            color: "textMuted",
          }),
        ]
      : model.agents.map((row) =>
          Stack(
            {
              key: `issue31-community-agent-${row.agentPubkey}`,
              direction: "column",
              gap: "1",
              padding: "3",
              style: { width: "full", borderWidth: 1, borderColor: "border", borderRadius: "lg" },
            },
            [
              Text({
                key: `issue31-community-agent-${row.agentPubkey}-id`,
                content: `${row.personaDisplayName ?? row.personaDTag ?? row.agentPubkey.slice(0, 12)} · ${row.agentPubkey.slice(0, 12)}…`,
                variant: "body",
                color: "textPrimary",
              }),
              Text({
                key: `issue31-community-agent-${row.agentPubkey}-state`,
                content: `Operator ${row.operatorPubkey.slice(0, 12)}… · attested · binding ${row.status} · grant ${row.capabilityGrant}${row.burned ? " · key burned by revocation" : ""}`,
                variant: "caption",
                color: row.status === "revoked" ? "warning" : "textMuted",
              }),
              ...(row.declaredCapabilities.length === 0
                ? []
                : [
                    Text({
                      key: `issue31-community-agent-${row.agentPubkey}-capabilities`,
                      content: `Declares · ${row.declaredCapabilities.join(" · ")}`,
                      variant: "caption",
                      color: "textMuted",
                    }),
                  ]),
              ...model.controls
                .filter(
                  (control) =>
                    control.kind === "revoke_agent" && control.subjectRef === row.agentPubkey,
                )
                .map((control) =>
                  Button({
                    key: `issue31-community-control-${control.idempotencyRef}`,
                    label: controlLabel.revoke_agent,
                    variant: "ghost",
                    onPress: IntentRef(
                      "Issue31CommunityAgentRevoked",
                      StaticPayload({ agentPubkey: row.agentPubkey }),
                    ),
                    a11y: {
                      label:
                        "Revoke this agent's group access and capability grant. Your machine is not touched.",
                    },
                    style: mobileInteractiveStyle(accessibility),
                  }),
                ),
            ],
          ),
        )),
    ...model.controls
      .filter((control) => control.kind === "attach_agent")
      .map((control) =>
        Text({
          key: `issue31-community-control-${control.idempotencyRef}`,
          content: agentComputeCopy.attach_agent,
          variant: "caption",
          color: "textMuted",
        }),
      ),

    Text({ key: "issue31-community-units-title", content: "Work units", variant: "heading" }),
    ...(model.workUnits.length === 0
      ? [
          Text({
            key: "issue31-community-units-empty",
            content: "No community work unit has been published to this group.",
            variant: "body",
            color: "textMuted",
          }),
        ]
      : model.workUnits.map((unit) =>
          communityUnitCard(unit, state.appealDraft, accessibility),
        )),

    Text({ key: "issue31-community-xp-title", content: "Experience", variant: "heading" }),
    Text({
      key: "issue31-community-xp-total",
      // Recomputed from the awards, every time. The published rank is shown
      // beside it and never instead of it.
      content: `${model.experience.recomputedTotalPoints} points · level ${model.experience.recomputedLevel} ${model.experience.recomputedLevelId} · ${model.experience.awardCount} accepted ${model.experience.awardCount === 1 ? "award" : "awards"}`,
      variant: "body",
      color: "textPrimary",
    }),
    ...(model.experience.publishedRankPoints === null
      ? []
      : [
          Text({
            key: "issue31-community-xp-rank",
            content: model.experience.publishedRankDisagreed
              ? `A published rank of ${model.experience.publishedRankPoints} disagrees with these awards. The awards win and the total above stands.`
              : `Scorer rank agrees · ${model.experience.publishedRankPoints}`,
            variant: "caption",
            color: model.experience.publishedRankDisagreed ? "warning" : "textMuted",
          }),
        ]),
    ...(model.experience.badgeIds.length === 0
      ? []
      : [
          Text({
            key: "issue31-community-xp-badges",
            content: `Badges · ${model.experience.badgeIds.join(" · ")}`,
            variant: "caption",
            color: "textMuted",
          }),
        ]),
    ...model.experience.awards.map((award) =>
      Text({
        key: `issue31-community-award-${award.sourceEventId ?? award.workEventId}`,
        content: `${award.awardKind} · ${award.points} points · work ${award.workEventId.slice(0, 12)}… · receipt ${award.receiptRef}`,
        variant: "caption",
        color: "textMuted",
      }),
    ),
    Text({
      key: "issue31-community-xp-boundary",
      content: model.experienceOnlyCopy,
      variant: "caption",
      color: "textMuted",
    }),
  ];
};

export const renderMobileIssue31WorkroomView = (
  model: Issue31WorkroomReadModel,
  selectedRoom: Issue31WorkroomRoom,
  nostrControl: Issue31MobileNostrControlState,
  accessibility: MobileAccessibilityProfile,
  ownerState: Issue31OwnerPrivateViewState,
  fullAuto: Issue31FullAutoReadModel,
  community: Issue31CommunityReadModel,
  communityState: Issue31CommunityViewState,
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
        ? [
            ...ownerPrivateDetail(model, nostrControl, ownerState, accessibility),
            // Conversation, run, provider, and evidence in one Workroom. The
            // owner never opens an unrelated product surface to see the work.
            ...fullAutoSection(fullAuto, model.ownerPrivate.commands, accessibility),
          ]
        : communityDetail(community, communityState, accessibility)),
      ...rows.map(capabilityCard),
    ],
  );
};
