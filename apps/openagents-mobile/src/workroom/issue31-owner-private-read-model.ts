import {
  parseEngramBody,
  parseReminderContent,
  validateReadStateBlob,
  mergeReadContexts,
  type SarahEngramBody,
  type SarahReadContexts,
  type SarahReminderContent,
} from "@openagentsinc/sarah/nostr-memory";
import {
  foldIssue31Grant,
  type Issue31CommandIntentV2,
  type Issue31CommandResultV2,
  type Issue31GrantState,
  type Issue31OwnerProjectionRecord,
} from "@openagentsinc/sarah/issue31-nostr";

import type { Issue31NostrClientSnapshot } from "./issue31-nostr-client.ts";

export const ISSUE31_OWNER_TRANSCRIPT_PAGE_SIZE = 40 as const;

export interface Issue31OwnerTranscriptRow {
  readonly sourceEventId: string;
  readonly sourceCreatedAt: number;
  readonly role: "owner" | "sarah";
  readonly text: string;
  readonly conversation: string;
  readonly deepLink: string;
}

export interface Issue31OwnerActivityRow {
  readonly sourceEventId: string;
  readonly sourceCreatedAt: number;
  readonly turnRef: string;
  readonly sequence: number;
  readonly entry: string;
  readonly label: string;
  readonly terminal: boolean;
  readonly deepLink: string;
}

export interface Issue31OwnerReceiptRow {
  readonly sourceEventId: string;
  readonly receiptRef: string;
  readonly turnRef: string;
  readonly authorityState: "allowed" | "refused";
  readonly decisionRef: string;
  readonly authorityReasonRef: string | null;
  readonly targetState: "pending" | "succeeded" | "failed" | "stopped" | "unavailable";
  readonly outcomeRef: string | null;
  readonly outcomeReasonRef: string | null;
  readonly deepLink: string;
}

export interface Issue31OwnerMemoryRow {
  readonly sourceEventId: string;
  readonly sourceCreatedAt: number;
  readonly dTag: string;
  readonly body: SarahEngramBody;
}

export interface Issue31OwnerReminderRow {
  readonly sourceEventId: string;
  readonly sourceCreatedAt: number;
  readonly reminderId: string;
  readonly content: SarahReminderContent;
  readonly notBefore: number | null;
  readonly expiration: number | null;
  readonly deepLink: string;
}

export type Issue31OwnerCommandState =
  | Readonly<{
      state: "queued" | "accepted";
      intentEventId: string;
      actionRef: string;
      idempotencyRef: string;
      handlingRef: string | null;
      sourceEventId: string | null;
    }>
  | Readonly<{
      state: "refused" | "failed" | "unavailable";
      intentEventId: string;
      actionRef: string;
      idempotencyRef: string;
      handlingRef: string;
      reasonRef: string | null;
    }>
  | Readonly<{
      state: "terminal";
      intentEventId: string;
      actionRef: string;
      idempotencyRef: string;
      handlingRef: string;
      sourceEventId: string;
    }>;

export interface Issue31OwnerPrivateReadModel {
  readonly status: "ready" | "unavailable" | "gap";
  readonly reasonRef: string | null;
  readonly grantRef: string | null;
  readonly generation: number | null;
  readonly transcript: ReadonlyArray<Issue31OwnerTranscriptRow>;
  readonly transcriptTotal: number;
  readonly hasEarlierTranscript: boolean;
  readonly activity: ReadonlyArray<Issue31OwnerActivityRow>;
  readonly receipts: ReadonlyArray<Issue31OwnerReceiptRow>;
  readonly memory: ReadonlyArray<Issue31OwnerMemoryRow>;
  readonly readContexts: SarahReadContexts;
  readonly reminders: ReadonlyArray<Issue31OwnerReminderRow>;
  readonly commands: ReadonlyArray<Issue31OwnerCommandState>;
  readonly attentionDeepLinks: ReadonlyArray<string>;
  readonly rejectedProjectionCount: number;
}

export const emptyIssue31OwnerPrivateReadModel = (
  reasonRef = "reason.issue31.owner_private.active_grant_missing",
): Issue31OwnerPrivateReadModel => ({
  status: "unavailable",
  reasonRef,
  grantRef: null,
  generation: null,
  transcript: [],
  transcriptTotal: 0,
  hasEarlierTranscript: false,
  activity: [],
  receipts: [],
  memory: [],
  readContexts: {},
  reminders: [],
  commands: [],
  attentionDeepLinks: [],
  rejectedProjectionCount: 0,
});

const deepLinkFor = (sourceEventId: string): string =>
  `openagents://omega/workroom?room=owner_private&sourceEventId=${sourceEventId}`;

const activeGrantFor = (
  snapshot: Issue31NostrClientSnapshot,
  nowUnixSeconds: number,
): Issue31GrantState | null => {
  const pairingEvents = snapshot.confirmedEvents.flatMap((event) =>
    event.privateRecord?.schema === "openagents.omega.issue31.pairing.v1"
      ? [{ eventId: event.canonicalRecordId, record: event.privateRecord }]
      : [],
  );
  const grantRefs = new Set(
    pairingEvents.flatMap(({ record }) => ("grantRef" in record ? [record.grantRef] : [])),
  );
  return (
    [...grantRefs]
      .flatMap((grantRef) => {
        try {
          const grant = foldIssue31Grant(pairingEvents, grantRef);
          return grant?.status === "active" &&
            grant.expiresAt !== null &&
            grant.expiresAt > nowUnixSeconds &&
            grant.devicePublicKeyHex === snapshot.devicePublicKeyHex &&
            snapshot.selectedHostPublicKeys.includes(grant.hostPublicKeyHex)
            ? [grant]
            : [];
        } catch {
          return [];
        }
      })
      .sort(
        (left, right) =>
          right.generation - left.generation ||
          right.issuedAt - left.issuedAt ||
          right.sourceEventId.localeCompare(left.sourceEventId),
      )[0] ?? null
  );
};

const admittedOwnerProjection = (
  record: Issue31OwnerProjectionRecord,
  grant: Issue31GrantState,
): boolean => {
  if (
    record.hostRef !== grant.hostRef ||
    record.hostPublicKeyHex !== grant.hostPublicKeyHex ||
    record.devicePublicKeyHex !== grant.devicePublicKeyHex ||
    record.grantRef !== grant.grantRef ||
    record.expectedGeneration !== grant.generation
  ) {
    return false;
  }
  if (record.sourceRole === "sarah") {
    return record.sourceAuthorPublicKeyHex === grant.sarahPublicKeyHex;
  }
  return record.sourceAuthorPublicKeyHex === grant.hostPublicKeyHex;
};

const sourceProjections = (
  snapshot: Issue31NostrClientSnapshot,
  grant: Issue31GrantState,
): Readonly<{ records: ReadonlyArray<Issue31OwnerProjectionRecord>; rejected: number }> => {
  const bySourceEventId = new Map<string, Issue31OwnerProjectionRecord>();
  const conflictedSourceEventIds = new Set<string>();
  let rejected = 0;
  for (const event of snapshot.confirmedEvents) {
    const record = event.privateRecord;
    if (record?.schema !== "openagents.omega.issue31.owner_projection.v1") continue;
    if (!admittedOwnerProjection(record, grant)) {
      rejected += 1;
      continue;
    }
    if (conflictedSourceEventIds.has(record.sourceEventId)) {
      rejected += 1;
      continue;
    }
    const prior = bySourceEventId.get(record.sourceEventId);
    if (prior !== undefined && JSON.stringify(prior) !== JSON.stringify(record)) {
      rejected += 1;
      bySourceEventId.delete(record.sourceEventId);
      conflictedSourceEventIds.add(record.sourceEventId);
      continue;
    }
    if (!bySourceEventId.has(record.sourceEventId))
      bySourceEventId.set(record.sourceEventId, record);
  }
  return {
    records: [...bySourceEventId.values()].sort(
      (left, right) =>
        left.sourceCreatedAt - right.sourceCreatedAt ||
        left.sourceEventId.localeCompare(right.sourceEventId),
    ),
    rejected,
  };
};

const transcriptRows = (
  records: ReadonlyArray<Issue31OwnerProjectionRecord>,
): ReadonlyArray<Issue31OwnerTranscriptRow> =>
  records.flatMap((record) => {
    if (record.projection.kind === "message") {
      return [
        {
          sourceEventId: record.sourceEventId,
          sourceCreatedAt: record.sourceCreatedAt,
          role: record.projection.role,
          text: record.projection.text,
          conversation: record.projection.conversation,
          deepLink: deepLinkFor(record.sourceEventId),
        },
      ];
    }
    if (record.projection.kind !== "turn" || record.projection.payload.entry !== "turn.finished") {
      return [];
    }
    const text = record.projection.payload.payload["text"];
    return typeof text === "string" && text.trim() !== ""
      ? [
          {
            sourceEventId: record.sourceEventId,
            sourceCreatedAt: record.sourceCreatedAt,
            role: "sarah" as const,
            text: text.slice(0, 12_000),
            conversation: record.projection.payload.conversation,
            deepLink: deepLinkFor(record.sourceEventId),
          },
        ]
      : [];
  });

const activityLabel = (record: Issue31OwnerProjectionRecord): string => {
  if (record.projection.kind !== "turn") return "";
  const payload = record.projection.payload.payload;
  const tool = typeof payload["tool"] === "string" ? ` · ${payload["tool"]}` : "";
  const reason = typeof payload["reason"] === "string" ? ` · ${payload["reason"]}` : "";
  return `${record.projection.payload.entry}${tool}${reason}`;
};

const projectionSettlesCommand = (
  intent: Issue31CommandIntentV2,
  projection: Issue31OwnerProjectionRecord,
): boolean => {
  const argumentsValue = intent.arguments;
  if (argumentsValue.kind === "send_message") {
    return (
      projection.projection.kind === "message" &&
      projection.projection.role === "owner" &&
      projection.projection.conversation === argumentsValue.conversation &&
      projection.projection.text === argumentsValue.text
    );
  }
  if (argumentsValue.kind === "interrupt_turn") {
    return (
      projection.projection.kind === "turn" &&
      projection.projection.payload.conversation === argumentsValue.conversation &&
      projection.projection.payload.turnRef === argumentsValue.turnRef &&
      projection.projection.payload.entry === "turn.interrupted"
    );
  }
  if (argumentsValue.kind === "read_state_patch") {
    if (
      projection.projection.kind !== "read_state" ||
      projection.projection.dTag !== `read-state:${argumentsValue.slotId}`
    ) {
      return false;
    }
    const state = validateReadStateBlob(projection.projection.plaintext);
    return (
      state !== null &&
      state.client_id === argumentsValue.clientId &&
      (state.contexts[argumentsValue.contextRef] ?? -1) >= argumentsValue.readAt
    );
  }
  if (projection.projection.kind !== "reminder") return false;
  if (projection.projection.reminderId !== argumentsValue.reminderId) return false;
  const reminder = parseReminderContent(projection.projection.plaintext);
  if (reminder === null) return false;
  if (argumentsValue.kind === "reminder_complete") return reminder.status === "done";
  if (argumentsValue.kind === "reminder_cancel") return reminder.status === "cancelled";
  return (
    reminder.status === "pending" &&
    reminder.note === argumentsValue.note &&
    reminder.target?.id === argumentsValue.targetEventId &&
    projection.projection.notBefore === argumentsValue.notBefore &&
    projection.projection.expiration === argumentsValue.expiration
  );
};

const commandStates = (
  snapshot: Issue31NostrClientSnapshot,
  grant: Issue31GrantState,
  projectionsBySourceId: ReadonlyMap<string, Issue31OwnerProjectionRecord>,
): Readonly<{ commands: ReadonlyArray<Issue31OwnerCommandState>; conflicts: number }> => {
  const intents = new Map<string, Readonly<{ eventId: string; record: Issue31CommandIntentV2 }>>();
  const results = new Map<string, Readonly<{ eventId: string; record: Issue31CommandResultV2 }>>();
  const conflictedIdempotencyRefs = new Set<string>();
  for (const event of snapshot.confirmedEvents) {
    const record = event.privateRecord;
    if (
      record?.schema !== "openagents.omega.issue31.command.v2" ||
      record.hostRef !== grant.hostRef ||
      record.hostPublicKeyHex !== grant.hostPublicKeyHex ||
      record.devicePublicKeyHex !== grant.devicePublicKeyHex ||
      record.grantRef !== grant.grantRef ||
      record.expectedGeneration !== grant.generation
    ) {
      continue;
    }
    if (conflictedIdempotencyRefs.has(record.idempotencyRef)) continue;
    if (record.recordType === "command_intent") {
      const prior = intents.get(record.idempotencyRef);
      if (prior !== undefined && JSON.stringify(prior.record) !== JSON.stringify(record)) {
        intents.delete(record.idempotencyRef);
        results.delete(record.idempotencyRef);
        conflictedIdempotencyRefs.add(record.idempotencyRef);
      } else if (prior === undefined) {
        intents.set(record.idempotencyRef, { eventId: event.canonicalRecordId, record });
      }
    } else {
      const prior = results.get(record.idempotencyRef);
      if (prior !== undefined && JSON.stringify(prior.record) !== JSON.stringify(record)) {
        intents.delete(record.idempotencyRef);
        results.delete(record.idempotencyRef);
        conflictedIdempotencyRefs.add(record.idempotencyRef);
      } else if (prior === undefined) {
        results.set(record.idempotencyRef, { eventId: event.canonicalRecordId, record });
      }
    }
  }
  const commands = [...intents.values()]
    .sort((left, right) => left.record.issuedAt - right.record.issuedAt)
    .map(({ eventId, record }) => {
      const result = results.get(record.idempotencyRef)?.record;
      const validResult =
        result !== undefined &&
        result.intentEventId === eventId &&
        result.actionRef === record.arguments.actionRef;
      if (!validResult || result === undefined) {
        return {
          state: "queued" as const,
          intentEventId: eventId,
          actionRef: record.arguments.actionRef,
          idempotencyRef: record.idempotencyRef,
          handlingRef: null,
          sourceEventId: null,
        };
      }
      if (result.status !== "accepted") {
        return {
          state: result.status,
          intentEventId: eventId,
          actionRef: record.arguments.actionRef,
          idempotencyRef: record.idempotencyRef,
          handlingRef: result.handlingRef,
          reasonRef: result.reasonRef ?? null,
        };
      }
      const sourceEventId = result.sourceEventId ?? null;
      const sourceProjection =
        sourceEventId === null ? undefined : projectionsBySourceId.get(sourceEventId);
      if (sourceProjection !== undefined && projectionSettlesCommand(record, sourceProjection)) {
        return {
          state: "terminal" as const,
          intentEventId: eventId,
          actionRef: record.arguments.actionRef,
          idempotencyRef: record.idempotencyRef,
          handlingRef: result.handlingRef,
          sourceEventId: sourceProjection.sourceEventId,
        };
      }
      return {
        state: "accepted" as const,
        intentEventId: eventId,
        actionRef: record.arguments.actionRef,
        idempotencyRef: record.idempotencyRef,
        handlingRef: result.handlingRef,
        sourceEventId,
      };
    });
  const incoherentResults = [...results.entries()].filter(([idempotencyRef, { record }]) => {
    const intent = intents.get(idempotencyRef);
    return (
      intent !== undefined &&
      (record.intentEventId !== intent.eventId ||
        record.actionRef !== intent.record.arguments.actionRef)
    );
  }).length;
  return { commands, conflicts: conflictedIdempotencyRefs.size + incoherentResults };
};

export const projectIssue31OwnerPrivateReadModel = (
  snapshot: Issue31NostrClientSnapshot,
  input: Readonly<{ nowUnixSeconds: number; transcriptLimit?: number }>,
): Issue31OwnerPrivateReadModel => {
  const grant = activeGrantFor(snapshot, input.nowUnixSeconds);
  if (grant === null) {
    return emptyIssue31OwnerPrivateReadModel();
  }
  const { records, rejected } = sourceProjections(snapshot, grant);
  const projectionsBySourceId = new Map(records.map((record) => [record.sourceEventId, record]));
  const transcript = transcriptRows(records);
  const transcriptLimit = Math.max(
    1,
    Math.min(200, input.transcriptLimit ?? ISSUE31_OWNER_TRANSCRIPT_PAGE_SIZE),
  );
  const memory = records.flatMap((record) => {
    if (record.projection.kind !== "engram") return [];
    const body = parseEngramBody(record.projection.plaintext);
    return body === null
      ? []
      : [
          {
            sourceEventId: record.sourceEventId,
            sourceCreatedAt: record.sourceCreatedAt,
            dTag: record.projection.dTag,
            body,
          },
        ];
  });
  const readContexts = mergeReadContexts(
    ...records.flatMap((record) => {
      if (record.projection.kind !== "read_state") return [];
      const state = validateReadStateBlob(record.projection.plaintext);
      return state === null ? [] : [state.contexts];
    }),
  );
  const remindersById = new Map<string, Issue31OwnerReminderRow>();
  for (const record of records) {
    if (record.projection.kind !== "reminder") continue;
    const content = parseReminderContent(record.projection.plaintext);
    if (content === null) continue;
    const row = {
      sourceEventId: record.sourceEventId,
      sourceCreatedAt: record.sourceCreatedAt,
      reminderId: record.projection.reminderId,
      content,
      notBefore: record.projection.notBefore ?? null,
      expiration: record.projection.expiration ?? null,
      deepLink: deepLinkFor(record.sourceEventId),
    };
    const prior = remindersById.get(row.reminderId);
    if (
      prior === undefined ||
      row.sourceCreatedAt > prior.sourceCreatedAt ||
      (row.sourceCreatedAt === prior.sourceCreatedAt && row.sourceEventId > prior.sourceEventId)
    ) {
      remindersById.set(row.reminderId, row);
    }
  }
  const commandProjection = commandStates(snapshot, grant, projectionsBySourceId);
  const commands = commandProjection.commands;
  const receipts = records.flatMap((record): ReadonlyArray<Issue31OwnerReceiptRow> => {
    if (record.projection.kind !== "authority_receipt") return [];
    return [
      {
        sourceEventId: record.sourceEventId,
        receiptRef: record.projection.receiptRef,
        turnRef: record.projection.turnRef,
        authorityState: record.projection.authorityDecision.state,
        decisionRef: record.projection.authorityDecision.decisionRef,
        authorityReasonRef: record.projection.authorityDecision.reasonRef ?? null,
        targetState: record.projection.targetOutcome.state,
        outcomeRef: record.projection.targetOutcome.outcomeRef ?? null,
        outcomeReasonRef: record.projection.targetOutcome.reasonRef ?? null,
        deepLink: deepLinkFor(record.sourceEventId),
      },
    ];
  });
  const attentionDeepLinks = [
    ...commands
      .filter((command) => command.state !== "terminal")
      .map(
        (command) =>
          `openagents://omega/workroom?room=owner_private&intentEventId=${command.intentEventId}`,
      ),
    ...receipts
      .filter(
        (receipt) => receipt.authorityState === "refused" || receipt.targetState !== "succeeded",
      )
      .map((receipt) => receipt.deepLink),
    ...[...remindersById.values()]
      .filter(
        (reminder) =>
          reminder.content.status === "pending" &&
          reminder.notBefore !== null &&
          reminder.notBefore <= input.nowUnixSeconds,
      )
      .map((reminder) => reminder.deepLink),
  ].slice(0, 64);
  return {
    status: rejected === 0 && commandProjection.conflicts === 0 ? "ready" : "gap",
    reasonRef:
      commandProjection.conflicts > 0
        ? "reason.issue31.command_idempotency_conflict"
        : rejected === 0
          ? null
          : "reason.issue31.owner_projection_rejected",
    grantRef: grant.grantRef,
    generation: grant.generation,
    transcript: transcript.slice(-transcriptLimit),
    transcriptTotal: transcript.length,
    hasEarlierTranscript: transcript.length > transcriptLimit,
    activity: records.flatMap(
      (record): ReadonlyArray<Issue31OwnerActivityRow> =>
        record.projection.kind === "turn"
          ? [
              {
                sourceEventId: record.sourceEventId,
                sourceCreatedAt: record.sourceCreatedAt,
                turnRef: record.projection.payload.turnRef,
                sequence: record.projection.payload.seq,
                entry: record.projection.payload.entry,
                label: activityLabel(record),
                terminal:
                  record.projection.payload.entry === "turn.finished" ||
                  record.projection.payload.entry === "turn.interrupted",
                deepLink: deepLinkFor(record.sourceEventId),
              },
            ]
          : [],
    ),
    receipts,
    memory,
    readContexts,
    reminders: [...remindersById.values()].sort(
      (left, right) => left.sourceCreatedAt - right.sourceCreatedAt,
    ),
    commands,
    attentionDeepLinks,
    rejectedProjectionCount: rejected + commandProjection.conflicts,
  };
};

export const searchIssue31LocalMemory = (
  memory: ReadonlyArray<Issue31OwnerMemoryRow>,
  query: string,
): ReadonlyArray<Issue31OwnerMemoryRow> => {
  const normalized = query.trim().toLocaleLowerCase().slice(0, 256);
  if (normalized === "") return [];
  return memory
    .filter((row) => JSON.stringify(row.body).toLocaleLowerCase().includes(normalized))
    .slice(0, 100);
};
