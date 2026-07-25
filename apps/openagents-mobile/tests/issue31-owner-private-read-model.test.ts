import {
  ISSUE31_COMMAND_SCHEMA_V2,
  ISSUE31_PAIRING_SCHEMA,
  decodeIssue31CommandRecordV2,
  decodeIssue31OwnerProjectionRecord,
  decodeIssue31PairingRecord,
  type Issue31CommandArguments,
  type Issue31CommandIntentV2,
  type Issue31PrivateRecord,
} from "@openagentsinc/sarah/issue31-nostr";
import { describe, expect, test } from "vite-plus/test";

import {
  projectIssue31OwnerPrivateReadModel,
  searchIssue31LocalMemory,
} from "../src/workroom/issue31-owner-private-read-model";
import type {
  Issue31ConfirmedEvent,
  Issue31NostrClientSnapshot,
} from "../src/workroom/issue31-nostr-client";

const hostPublicKeyHex = "1".repeat(64);
const devicePublicKeyHex = "2".repeat(64);
const sarahPublicKeyHex = "3".repeat(64);
const ownerPublicKeyHex = hostPublicKeyHex;
const conversation = "sarah.0123456789abcdef01234567";
const grantRef = "grant.omega.device_1";

const privateEvent = (
  canonicalRecordId: string,
  record: Issue31PrivateRecord,
): Issue31ConfirmedEvent => ({
  relayUrl: "wss://relay.example.com",
  room: "owner_private",
  canonicalRecordId,
  privateRumorId: canonicalRecordId,
  privateRecord: record,
  hostAnnouncement: null,
  event: {
    id: canonicalRecordId.split("").reverse().join(""),
    pubkey: record.recordType === "command_intent" ? devicePublicKeyHex : hostPublicKeyHex,
    created_at: 1_000,
    kind: 1_059,
    tags: [["p", devicePublicKeyHex]],
    content: "ciphertext",
    sig: "a".repeat(128),
  },
});

const pairingEvents = (): ReadonlyArray<Issue31ConfirmedEvent> => {
  const requestId = "a".repeat(64);
  const challengeId = "b".repeat(64);
  const responseId = "c".repeat(64);
  const grantId = "d".repeat(64);
  return [
    privateEvent(
      requestId,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_request",
        hostRef: "omega.host.local",
        hostPublicKeyHex,
        devicePublicKeyHex,
        issuedAt: 700,
        pairingRequestRef: "pairing.request.device_1",
        requestedScopes: ["observe_issue31", "send_message", "interrupt_turn"],
        expiresAt: 1_300,
      }),
    ),
    privateEvent(
      challengeId,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_challenge",
        hostRef: "omega.host.local",
        hostPublicKeyHex,
        devicePublicKeyHex,
        issuedAt: 800,
        pairingChallengeRef: "pairing.challenge.device_1",
        pairingRequestEventId: requestId,
        challenge: "e".repeat(64),
        expiresAt: 1_300,
      }),
    ),
    privateEvent(
      responseId,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_response",
        hostRef: "omega.host.local",
        hostPublicKeyHex,
        devicePublicKeyHex,
        issuedAt: 900,
        pairingResponseRef: "pairing.response.device_1",
        pairingChallengeEventId: challengeId,
        challenge: "e".repeat(64),
        expiresAt: 1_300,
      }),
    ),
    privateEvent(
      grantId,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "scoped_grant",
        hostRef: "omega.host.local",
        hostPublicKeyHex,
        sarahPublicKeyHex,
        devicePublicKeyHex,
        issuedAt: 950,
        pairingResponseEventId: responseId,
        grantRef,
        generation: 1,
        scopes: ["observe_issue31", "send_message", "interrupt_turn"],
        expiresAt: 2_000,
      }),
    ),
  ];
};

const snapshot = (records: ReadonlyArray<Issue31ConfirmedEvent>): Issue31NostrClientSnapshot => ({
  devicePublicKeyHex,
  admittedHostPublicKeys: [hostPublicKeyHex],
  selectedHostPublicKeys: [hostPublicKeyHex],
  ownerPrivateAuthors: [sarahPublicKeyHex],
  ownerRecipientPublicKeys: [hostPublicKeyHex],
  relays: [],
  confirmedEvents: [...pairingEvents(), ...records],
  storedEventIds: {},
  publishRefusals: {},
});

const intent = (
  idempotencyRef: string,
  argumentsValue: Issue31CommandArguments,
): Issue31CommandIntentV2 => {
  const record = decodeIssue31CommandRecordV2({
    schema: ISSUE31_COMMAND_SCHEMA_V2,
    recordType: "command_intent",
    hostRef: "omega.host.local",
    hostPublicKeyHex,
    devicePublicKeyHex,
    grantRef,
    idempotencyRef,
    expectedGeneration: 1,
    arguments: argumentsValue,
    issuedAt: 1_000,
    expiresAt: 1_300,
  });
  if (record.recordType !== "command_intent") throw new Error("expected command intent");
  return record;
};

const accepted = (
  intentEventId: string,
  idempotencyRef: string,
  actionRef: string,
  sourceEventId: string,
) =>
  decodeIssue31CommandRecordV2({
    schema: ISSUE31_COMMAND_SCHEMA_V2,
    recordType: "command_result",
    hostRef: "omega.host.local",
    hostPublicKeyHex,
    devicePublicKeyHex,
    grantRef,
    intentEventId,
    actionRef,
    idempotencyRef,
    expectedGeneration: 1,
    status: "accepted",
    handlingRef: `handling.issue31.${idempotencyRef.split(".").at(-1)}`,
    sourceEventId,
    handledAt: 1_010,
  });

const projection = (
  sourceEventId: string,
  projectionValue: unknown,
  overrides: Readonly<Record<string, unknown>> = {},
) =>
  decodeIssue31OwnerProjectionRecord({
    schema: "openagents.omega.issue31.owner_projection.v1",
    recordType: "owner_projection",
    hostRef: "omega.host.local",
    hostPublicKeyHex,
    devicePublicKeyHex,
    grantRef,
    expectedGeneration: 1,
    sourceEventId,
    sourceAuthorPublicKeyHex:
      (projectionValue as Readonly<{ kind: string }>).kind === "message"
        ? ownerPublicKeyHex
        : sarahPublicKeyHex,
    sourceRole:
      (projectionValue as Readonly<{ kind: string }>).kind === "message" ? "owner" : "sarah",
    sourceKind: (projectionValue as Readonly<{ kind: string }>).kind === "message" ? 14 : 44_300,
    sourceCreatedAt: 1_005,
    projectedAt: 1_010,
    projection: projectionValue,
    ...overrides,
  });

describe("Issue 31 owner-private mobile read model", () => {
  test("keeps host acceptance pending until the exact send source projection arrives", () => {
    const intentId = "5".repeat(64);
    const resultId = "6".repeat(64);
    const sourceId = "7".repeat(64);
    const send = intent("idempotency.issue31.send_1", {
      kind: "send_message",
      actionRef: "action.issue31.sarah.send",
      conversation,
      text: "hello",
    });
    const handling = accepted(intentId, send.idempotencyRef, send.arguments.actionRef, sourceId);
    const pending = projectIssue31OwnerPrivateReadModel(
      snapshot([privateEvent(intentId, send), privateEvent(resultId, handling)]),
      { nowUnixSeconds: 1_100 },
    );
    expect(pending.commands[0]?.state).toBe("accepted");

    const unrelated = projection(sourceId, {
      kind: "message",
      role: "owner",
      conversation,
      text: "different",
    });
    expect(
      projectIssue31OwnerPrivateReadModel(
        snapshot([
          privateEvent(intentId, send),
          privateEvent(resultId, handling),
          privateEvent("8".repeat(64), unrelated),
        ]),
        { nowUnixSeconds: 1_100 },
      ).commands[0]?.state,
    ).toBe("accepted");

    const exact = projection(sourceId, {
      kind: "message",
      role: "owner",
      conversation,
      text: "hello",
    });
    expect(
      projectIssue31OwnerPrivateReadModel(
        snapshot([
          privateEvent(intentId, send),
          privateEvent(resultId, handling),
          privateEvent("9".repeat(64), exact),
        ]),
        { nowUnixSeconds: 1_100 },
      ).commands[0]?.state,
    ).toBe("terminal");
  });

  test("matches interrupt, read-state, and reminder terminal projections by content", () => {
    const cases: ReadonlyArray<
      Readonly<{
        arguments: Issue31CommandArguments;
        projection: ReturnType<typeof projection>;
      }>
    > = [
      {
        arguments: {
          kind: "interrupt_turn",
          actionRef: "action.issue31.sarah.interrupt",
          conversation,
          turnRef: "turn.issue31.one",
        },
        projection: projection("a".repeat(64), {
          kind: "turn",
          payload: {
            schema: "openagents.sarah.turn_record.v1",
            entry: "turn.interrupted",
            conversation,
            turnRef: "turn.issue31.one",
            seq: 2,
            timestamp: "2026-07-25T00:00:00.000Z",
            parents: [],
            payload: {},
          },
        }),
      },
      {
        arguments: {
          kind: "read_state_patch",
          actionRef: "action.issue31.read_state.advance",
          slotId: "owner-mobile",
          clientId: "openagents-mobile",
          contextRef: "msg:" + "f".repeat(64),
          readAt: 1_050,
        },
        projection: projection(
          "b".repeat(64),
          {
            kind: "read_state",
            dTag: "read-state:owner-mobile",
            plaintext: JSON.stringify({
              v: 1,
              client_id: "openagents-mobile",
              contexts: { ["msg:" + "f".repeat(64)]: 1_050 },
            }),
          },
          { sourceKind: 30_078, sourceRole: "owner", sourceAuthorPublicKeyHex: ownerPublicKeyHex },
        ),
      },
      {
        arguments: {
          kind: "reminder_complete",
          actionRef: "action.issue31.reminder.complete",
          reminderId: "c".repeat(32),
        },
        projection: projection(
          "c".repeat(64),
          {
            kind: "reminder",
            reminderId: "c".repeat(32),
            plaintext: JSON.stringify({ status: "done" }),
          },
          { sourceKind: 30_300, sourceRole: "owner", sourceAuthorPublicKeyHex: ownerPublicKeyHex },
        ),
      },
    ];
    cases.forEach((entry, index) => {
      const intentId = String(index + 1).repeat(64);
      const sourceId = entry.projection.sourceEventId;
      const command = intent(`idempotency.issue31.case_${index}`, entry.arguments);
      const handling = accepted(
        intentId,
        command.idempotencyRef,
        command.arguments.actionRef,
        sourceId,
      );
      const unrelated = projection(sourceId, {
        kind: "message",
        role: "owner",
        conversation,
        text: "unrelated accepted source",
      });
      expect(
        projectIssue31OwnerPrivateReadModel(
          snapshot([
            privateEvent(intentId, command),
            privateEvent(String(index + 4).repeat(64), handling),
            privateEvent(String(index + 7).repeat(64), unrelated),
          ]),
          { nowUnixSeconds: 1_100 },
        ).commands[0]?.state,
      ).toBe("accepted");
      const model = projectIssue31OwnerPrivateReadModel(
        snapshot([
          privateEvent(intentId, command),
          privateEvent(String(index + 4).repeat(64), handling),
          privateEvent(String(index + 7).repeat(64), entry.projection),
        ]),
        { nowUnixSeconds: 1_100 },
      );
      expect(model.commands[0]?.state).toBe("terminal");
    });
  });

  test("poisons conflicting source ids and idempotency refs for every replay order", () => {
    const sourceId = "e".repeat(64);
    const first = projection(sourceId, {
      kind: "message",
      role: "owner",
      conversation,
      text: "first",
    });
    const second = projection(sourceId, {
      kind: "message",
      role: "owner",
      conversation,
      text: "second",
    });
    for (const ordered of [
      [first, second, first],
      [
        first,
        second,
        projection(sourceId, {
          kind: "message",
          role: "owner",
          conversation,
          text: "third",
        }),
      ],
    ]) {
      const model = projectIssue31OwnerPrivateReadModel(
        snapshot(
          ordered.map((record, index) => privateEvent(String(index + 5).repeat(64), record)),
        ),
        { nowUnixSeconds: 1_100 },
      );
      expect(model.status).toBe("gap");
      expect(model.transcript).toEqual([]);
    }

    const firstIntent = intent("idempotency.issue31.conflict", {
      kind: "send_message",
      actionRef: "action.issue31.sarah.send",
      conversation,
      text: "one",
    });
    const secondIntent = intent("idempotency.issue31.conflict", {
      kind: "send_message",
      actionRef: "action.issue31.sarah.send",
      conversation,
      text: "two",
    });
    const model = projectIssue31OwnerPrivateReadModel(
      snapshot([
        privateEvent("1".repeat(64), firstIntent),
        privateEvent("2".repeat(64), secondIntent),
        privateEvent("3".repeat(64), firstIntent),
      ]),
      { nowUnixSeconds: 1_100 },
    );
    expect(model.status).toBe("gap");
    expect(model.reasonRef).toBe("reason.issue31.command_idempotency_conflict");
    expect(model.commands).toEqual([]);

    const coherentIntentId = "4".repeat(64);
    const coherentIntent = intent("idempotency.issue31.mismatched_result", {
      kind: "send_message",
      actionRef: "action.issue31.sarah.send",
      conversation,
      text: "source-bound",
    });
    const mismatchedResult = accepted(
      "f".repeat(64),
      coherentIntent.idempotencyRef,
      coherentIntent.arguments.actionRef,
      "e".repeat(64),
    );
    const mismatchedModel = projectIssue31OwnerPrivateReadModel(
      snapshot([
        privateEvent(coherentIntentId, coherentIntent),
        privateEvent("6".repeat(64), mismatchedResult),
      ]),
      { nowUnixSeconds: 1_100 },
    );
    expect(mismatchedModel.status).toBe("gap");
    expect(mismatchedModel.reasonRef).toBe("reason.issue31.command_idempotency_conflict");
    expect(mismatchedModel.commands[0]?.state).toBe("queued");
  });

  test("paginates transcript and searches decrypted memory only in the passed local model", () => {
    const records = Array.from({ length: 45 }, (_, index) => {
      const eventId = index.toString(16).padStart(64, "0");
      return privateEvent(
        eventId.split("").reverse().join(""),
        projection(eventId, {
          kind: "message",
          role: "owner",
          conversation,
          text: `message ${index}`,
        }),
      );
    });
    const memoryId = "f".repeat(64);
    records.push(
      privateEvent(
        "9".repeat(64),
        projection(
          memoryId,
          {
            kind: "engram",
            dTag: "a".repeat(64),
            plaintext: JSON.stringify({ slug: "mem/project", value: "Omega launch" }),
          },
          { sourceKind: 30_174 },
        ),
      ),
    );
    const model = projectIssue31OwnerPrivateReadModel(snapshot(records), {
      nowUnixSeconds: 1_100,
    });
    expect(model.transcript).toHaveLength(40);
    expect(model.transcriptTotal).toBe(45);
    expect(model.hasEarlierTranscript).toBe(true);
    expect(
      searchIssue31LocalMemory(model.memory, "launch").map((row) => row.sourceEventId),
    ).toEqual([memoryId]);
  });
});
