import { readFileSync } from "node:fs";

import {
  ISSUE31_COMMAND_SCHEMA_V2,
  ISSUE31_PAIRING_SCHEMA,
  decodeIssue31CommandRecordV2,
  decodeIssue31OwnerProjectionRecord,
  decodeIssue31PairingRecord,
  decodeIssue31WithheldSourcesRecord,
  type Issue31CommandArguments,
  type Issue31CommandIntentV2,
  type Issue31OwnerProjectionRecord,
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

/**
 * omega#46 exits 3, 5, and 6 — read state, reminders, and authority receipts.
 *
 * These fields had no coverage at all, and the reason was structural: the
 * Omega host emits these projection bodies but the shape it emits was written
 * down nowhere both sides could check. The bodies below are read from the
 * fixtures the Omega producer emits and pins by digest
 * (`crates/omega_effectd/fixtures/`, asserted in
 * `crates/omega_effectd/src/issue31_nostr.rs`), so this is coverage of what the
 * host actually sends, not of a shape invented here.
 *
 * `expectedGeneration` is the one field adapted: the fixtures carry generation
 * 3 and the grant in this file is generation 1. Nothing inside `projection` is
 * touched.
 */
describe("Issue 31 owner-private read state, reminders, and receipts", () => {
  const hostFixture = (name: string): Record<string, unknown> =>
    JSON.parse(
      readFileSync(
        new URL(
          `../../../packages/sarah/fixtures/issue31-nostr/openagents.omega.issue31.owner_projection.v1.${name}.json`,
          import.meta.url,
        ),
        "utf8",
      ),
    ) as Record<string, unknown>;

  const hostProjection = (
    name: string,
    overrides: Record<string, unknown> = {},
  ): Issue31ConfirmedEvent => {
    const fixture = hostFixture(name);
    const record = decodeIssue31OwnerProjectionRecord({
      ...fixture,
      expectedGeneration: 1,
      ...overrides,
    });
    return privateEvent(record.sourceEventId, record);
  };

  test("projects the exact read state, reminder, receipt, and engram the host emits", () => {
    const model = projectIssue31OwnerPrivateReadModel(
      snapshot([
        hostProjection("canonical-read-state"),
        hostProjection("canonical-reminder"),
        hostProjection("canonical-authority-receipt"),
        hostProjection("canonical-engram"),
      ]),
      { nowUnixSeconds: 1_100 },
    );

    expect(model.status).toBe("ready");
    expect(model.readContexts).toEqual({ [conversation]: 1_784_937_608 });

    expect(model.reminders).toHaveLength(1);
    expect(model.reminders[0]).toMatchObject({
      reminderId: "0123456789abcdef0123456789abcdef",
      notBefore: 1_784_938_000,
      expiration: 1_785_024_000,
      content: { status: "pending", note: "Check the release evidence." },
      deepLink: `openagents://omega/workroom?room=owner_private&sourceEventId=${"d".repeat(64)}`,
    });

    expect(model.receipts).toHaveLength(1);
    expect(model.receipts[0]).toEqual({
      sourceEventId: "a".repeat(24) + "b".repeat(40),
      receiptRef: `receipt.issue31.${"a".repeat(24)}`,
      turnRef: "turn.issue31.release_evidence",
      authorityState: "refused",
      decisionRef: `decision.issue31.${"a".repeat(24)}`,
      authorityReasonRef: "reason.openagents.reserved_custody",
      // A refused decision says nothing about the target. It stays pending.
      targetState: "pending",
      outcomeRef: null,
      outcomeReasonRef: null,
      deepLink: `openagents://omega/workroom?room=owner_private&sourceEventId=${"a".repeat(24) + "b".repeat(40)}`,
    });
    expect(model.attentionDeepLinks).toContain(model.receipts[0]!.deepLink);
    // The reminder is not due at this clock, so it is not attention.
    expect(model.attentionDeepLinks).not.toContain(model.reminders[0]!.deepLink);

    expect(model.memory).toHaveLength(1);
    expect(model.memory[0]?.body).toEqual({
      slug: "mem/release_evidence",
      value: "The release candidate is notarized.",
    });
  });

  test("two clients agree on read state after replay in either order", () => {
    // Two read-state records from two clients, each ahead on one context.
    const first = hostProjection("canonical-read-state");
    const second = hostProjection("canonical-read-state", {
      sourceEventId: "1".repeat(64),
      sourceCreatedAt: 1_784_937_700,
      projectedAt: 1_784_937_701,
      projection: {
        kind: "read_state",
        dTag: "read-state:owner-private",
        plaintext: JSON.stringify({
          v: 1,
          client_id: "owner-phone",
          contexts: { [conversation]: 1_784_937_500, "sarah.feedcafefeedcafefeedcafe": 1_784_937_900 },
        }),
      },
    });

    const forward = projectIssue31OwnerPrivateReadModel(snapshot([first, second]), {
      nowUnixSeconds: 1_100,
    });
    const reverse = projectIssue31OwnerPrivateReadModel(snapshot([second, first]), {
      nowUnixSeconds: 1_100,
    });
    // A duplicate delivery is a replay, not a second opinion.
    const replayed = projectIssue31OwnerPrivateReadModel(
      snapshot([second, first, second, first]),
      { nowUnixSeconds: 1_100 },
    );

    const converged = {
      [conversation]: 1_784_937_608,
      "sarah.feedcafefeedcafefeedcafe": 1_784_937_900,
    };
    expect(forward.readContexts).toEqual(converged);
    expect(reverse.readContexts).toEqual(converged);
    expect(replayed.readContexts).toEqual(converged);
    // The merge never lowers a frontier: the second record's older value for
    // the shared context did not win.
    expect(forward.readContexts[conversation]).toBe(1_784_937_608);
  });

  test("reminder changes converge across clients in either delivery order", () => {
    const created = hostProjection("canonical-reminder");
    const completed = hostProjection("canonical-reminder", {
      sourceEventId: "2".repeat(64),
      sourceCreatedAt: 1_784_937_800,
      projectedAt: 1_784_937_801,
      projection: {
        kind: "reminder",
        reminderId: "0123456789abcdef0123456789abcdef",
        plaintext: JSON.stringify({ status: "done" }),
      },
    });

    const forward = projectIssue31OwnerPrivateReadModel(snapshot([created, completed]), {
      nowUnixSeconds: 1_100,
    });
    const reverse = projectIssue31OwnerPrivateReadModel(snapshot([completed, created]), {
      nowUnixSeconds: 1_100,
    });
    for (const model of [forward, reverse]) {
      expect(model.reminders).toHaveLength(1);
      expect(model.reminders[0]?.content.status).toBe("done");
      expect(model.reminders[0]?.sourceEventId).toBe("2".repeat(64));
      expect(model.reminders[0]?.notBefore).toBeNull();
    }
  });

  test("a reminder tie is broken the same way on every client", () => {
    // Same instant, two records. Without a total order the two clients would
    // show different reminders and "converge" would be a coin flip.
    const lower = hostProjection("canonical-reminder", {
      sourceEventId: "3".repeat(64),
      projection: {
        kind: "reminder",
        reminderId: "0123456789abcdef0123456789abcdef",
        plaintext: JSON.stringify({ status: "cancelled" }),
      },
    });
    const higher = hostProjection("canonical-reminder", {
      sourceEventId: "4".repeat(64),
      projection: {
        kind: "reminder",
        reminderId: "0123456789abcdef0123456789abcdef",
        plaintext: JSON.stringify({ status: "done" }),
      },
    });
    for (const records of [
      [lower, higher],
      [higher, lower],
    ]) {
      const model = projectIssue31OwnerPrivateReadModel(snapshot(records), {
        nowUnixSeconds: 1_100,
      });
      expect(model.reminders[0]?.sourceEventId).toBe("4".repeat(64));
      expect(model.reminders[0]?.content.status).toBe("done");
    }
  });

  test("an authority receipt bound to another grant is refused, not read", () => {
    // A receipt is authority evidence. One addressed to a different generation
    // is not this device's evidence, and it must not silently become a row.
    const foreign = hostProjection("canonical-authority-receipt", { expectedGeneration: 2 });
    const model = projectIssue31OwnerPrivateReadModel(snapshot([foreign]), {
      nowUnixSeconds: 1_100,
    });
    expect(model.receipts).toEqual([]);
    expect(model.status).toBe("gap");
    expect(model.reasonRef).toBe("reason.issue31.owner_projection_rejected");
    expect(model.rejectedProjectionCount).toBe(1);
  });
});

/**
 * omega#46 exit 4: "The owner can inspect every engram available to Sarah."
 *
 * Three paths removed a source from the owner's view with nothing the device
 * could see — a host quarantine counted only in `BootstrapResult`, the bounded
 * projection scan recorded only in the host's `last_gap_state`, and a
 * device-side read failure with no counter at all. Each gets its own test with
 * its own assertion, so no case is carried by a neighbour's evidence.
 */
describe("Issue 31 owner-private withheld sources", () => {
  const engramFixture = (): Record<string, unknown> =>
    JSON.parse(
      readFileSync(
        new URL(
          "../../../packages/sarah/fixtures/issue31-nostr/openagents.omega.issue31.owner_projection.v1.canonical-engram.json",
          import.meta.url,
        ),
        "utf8",
      ),
    ) as Record<string, unknown>;

  const engram = (): Issue31ConfirmedEvent => {
    const record = decodeIssue31OwnerProjectionRecord({
      ...engramFixture(),
      expectedGeneration: 1,
    });
    return privateEvent(record.sourceEventId, record);
  };

  const coverageFixture = (name: string): Record<string, unknown> =>
    JSON.parse(
      readFileSync(
        new URL(
          `../../../packages/sarah/fixtures/issue31-nostr/openagents.omega.issue31.withheld_sources.v1.${name}.json`,
          import.meta.url,
        ),
        "utf8",
      ),
    ) as Record<string, unknown>;

  const coverage = (
    canonicalRecordId: string,
    overrides: Record<string, unknown> = {},
    name = "canonical-complete",
  ): Issue31ConfirmedEvent =>
    privateEvent(
      canonicalRecordId,
      decodeIssue31WithheldSourcesRecord({
        ...coverageFixture(name),
        expectedGeneration: 1,
        ...overrides,
      }),
    );

  const quarantinedStatement = (canonicalRecordId: string, overrides = {}) =>
    coverage(
      canonicalRecordId,
      {
        withheld: [
          {
            cause: "quarantined",
            count: 3,
            exact: true,
            reasonRef: "reason.omega.invalid_projection_source",
          },
        ],
        coverage: "partial",
        ...overrides,
      },
      "canonical-partial",
    );

  test("a device holding no coverage statement reads unknown, never complete", () => {
    // The whole defect was that silence looked like completeness. A host that
    // has said nothing has not said the list is whole.
    const model = projectIssue31OwnerPrivateReadModel(snapshot([engram()]), {
      nowUnixSeconds: 1_100,
    });
    expect(model.memory).toHaveLength(1);
    expect(model.coverage).toBe("unknown");
    expect(model.withheld).toEqual([]);
  });

  test("only a host statement of completeness makes the list complete", () => {
    const model = projectIssue31OwnerPrivateReadModel(
      snapshot([engram(), coverage("5".repeat(64))]),
      {
        nowUnixSeconds: 1_100,
      },
    );
    expect(model.coverage).toBe("complete");
    expect(model.withheld).toEqual([]);
    expect(model.status).toBe("ready");
  });

  test("drop path one: a host quarantine reaches the device as an exact count with a reason", () => {
    const model = projectIssue31OwnerPrivateReadModel(
      snapshot([engram(), quarantinedStatement("5".repeat(64))]),
      { nowUnixSeconds: 1_100 },
    );
    expect(model.coverage).toBe("partial");
    expect(model.withheld).toEqual([
      {
        cause: "quarantined",
        count: 3,
        exact: true,
        reasonRef: "reason.omega.invalid_projection_source",
        observedBy: "host",
        deepLink: "openagents://omega/workroom?room=owner_private&withheld=quarantined",
      },
    ]);
    // Fail visible, not fail closed: the engram that did arrive still renders.
    expect(model.memory).toHaveLength(1);
    expect(model.status).toBe("gap");
    expect(model.reasonRef).toBe("reason.issue31.owner_sources_withheld");
    expect(model.attentionDeepLinks).toContain(
      "openagents://omega/workroom?room=owner_private&withheld=quarantined",
    );
  });

  test("drop path two: the projection scan bound reaches the device as a lower bound", () => {
    const model = projectIssue31OwnerPrivateReadModel(
      snapshot([
        engram(),
        coverage(
          "5".repeat(64),
          {
            withheld: [
              {
                cause: "scan_bound",
                count: 1,
                exact: false,
                reasonRef: "reason.omega.projection_scan_bound",
              },
            ],
            coverage: "partial",
          },
          "canonical-partial",
        ),
      ]),
      { nowUnixSeconds: 1_100 },
    );
    expect(model.coverage).toBe("partial");
    expect(model.withheld).toHaveLength(1);
    expect(model.withheld[0]).toMatchObject({ cause: "scan_bound", exact: false, count: 1 });
    // A host that stopped reading cannot say how many it did not read, and the
    // device must not render its lower bound as though it were exact.
    expect(model.withheld[0]?.exact).toBe(false);
    expect(model.memory).toHaveLength(1);
  });

  test("drop path three: an engram this device cannot read is counted, not dropped in silence", () => {
    // This models decoder/read-model drift. `decodeIssue31OwnerProjectionRecord`
    // refuses an unreadable engram body today, so the record is built directly:
    // the point is that if the two ever disagree — a relaxed decoder, a version
    // skew, a bound that moves on one side — the read model counts the drop
    // instead of shortening the list in silence.
    const drifted = {
      ...engramFixture(),
      expectedGeneration: 1,
      projection: {
        kind: "engram",
        dTag: "f".repeat(64),
        plaintext: JSON.stringify({ slug: "mem/unreadable", value: { not: "a string" } }),
      },
    } as unknown as Issue31OwnerProjectionRecord;
    const model = projectIssue31OwnerPrivateReadModel(
      snapshot([privateEvent("6".repeat(64), drifted), coverage("5".repeat(64))]),
      { nowUnixSeconds: 1_100 },
    );
    expect(model.memory).toEqual([]);
    expect(model.coverage).toBe("partial");
    expect(model.withheld).toEqual([
      {
        cause: "unreadable",
        count: 1,
        exact: true,
        reasonRef: "reason.issue31.owner_engram_unreadable",
        observedBy: "device",
        deepLink: "openagents://omega/workroom?room=owner_private&withheld=unreadable",
      },
    ]);
    // The host said complete and the device knows better. A device-observed
    // withholding must override a host statement, never be overridden by it.
    expect(model.status).toBe("gap");
  });

  test("a coverage statement bound to another generation is refused, not read", () => {
    const model = projectIssue31OwnerPrivateReadModel(
      snapshot([engram(), coverage("5".repeat(64), { expectedGeneration: 2 })]),
      { nowUnixSeconds: 1_100 },
    );
    expect(model.coverage).toBe("unknown");
    expect(model.status).toBe("gap");
    expect(model.reasonRef).toBe("reason.issue31.owner_projection_rejected");
    expect(model.rejectedProjectionCount).toBe(1);
  });

  test("the newest statement wins, so coverage can be restored as well as lost", () => {
    const withheldEarlier = quarantinedStatement("5".repeat(64), { observedAt: 1_784_937_651 });
    const completeLater = coverage("6".repeat(64), { observedAt: 1_784_937_999 });
    for (const records of [
      [withheldEarlier, completeLater],
      [completeLater, withheldEarlier],
    ]) {
      const model = projectIssue31OwnerPrivateReadModel(snapshot([engram(), ...records]), {
        nowUnixSeconds: 1_100,
      });
      expect(model.coverage).toBe("complete");
      expect(model.withheld).toEqual([]);
    }

    const stillWithheld = projectIssue31OwnerPrivateReadModel(
      snapshot([
        engram(),
        coverage("6".repeat(64), { observedAt: 1_784_937_000 }),
        quarantinedStatement("5".repeat(64), { observedAt: 1_784_937_651 }),
      ]),
      { nowUnixSeconds: 1_100 },
    );
    expect(stillWithheld.coverage).toBe("partial");
  });

  test("a tie on observation time never resolves toward claiming completeness", () => {
    // Two contradictory statements at the same second. An identifier tie-break
    // that could pick "complete" would hide a gap by accident.
    const tiedComplete = coverage("6".repeat(64), { observedAt: 1_784_937_651 });
    const tiedWithheld = quarantinedStatement("5".repeat(64), { observedAt: 1_784_937_651 });
    for (const records of [
      [tiedComplete, tiedWithheld],
      [tiedWithheld, tiedComplete],
    ]) {
      const model = projectIssue31OwnerPrivateReadModel(snapshot([engram(), ...records]), {
        nowUnixSeconds: 1_100,
      });
      expect(model.coverage).toBe("partial");
    }
  });
});

describe("issue 31 owner-private revocation reason", () => {
  const revocation = (): Issue31ConfirmedEvent =>
    privateEvent(
      "f".repeat(64),
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "grant_revocation",
        hostRef: "omega.host.local",
        hostPublicKeyHex,
        sarahPublicKeyHex,
        devicePublicKeyHex,
        issuedAt: 1_000,
        grantRef,
        generation: 1,
        reasonRef: "reason.omega.owner_revoked",
      }),
    );

  test("a revoked grant is reported as revoked, not as missing", () => {
    // omega#49: the room stopping at "ready" was the first defect and is fixed
    // in the runtime. This is the second half — falling through to the default
    // `active_grant_missing` describes a deliberate owner decision as an
    // absence, which is the same mistake as reporting an authentication
    // failure as a discovery one.
    const model = projectIssue31OwnerPrivateReadModel(snapshot([revocation()]), {
      nowUnixSeconds: 1_100,
    });
    expect(model.status).toBe("unavailable");
    expect(model.reasonRef).toBe("reason.issue31.owner_private.grant_revoked");
  });

  test("a device with no grant at all still reports the grant as missing", () => {
    // The revoked reason must not swallow the genuinely-absent case.
    const noGrant: Issue31NostrClientSnapshot = {
      ...snapshot([]),
      confirmedEvents: [],
    };
    const model = projectIssue31OwnerPrivateReadModel(noGrant, { nowUnixSeconds: 1_100 });
    expect(model.status).toBe("unavailable");
    expect(model.reasonRef).toBe("reason.issue31.owner_private.active_grant_missing");
  });

  test("another device's revocation does not revoke this one", () => {
    const otherDevice = "9".repeat(64);
    const strayRevocation = privateEvent(
      "e".repeat(64),
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "grant_revocation",
        hostRef: "omega.host.local",
        hostPublicKeyHex,
        sarahPublicKeyHex,
        devicePublicKeyHex: otherDevice,
        issuedAt: 1_000,
        grantRef: "grant.omega.device_2",
        generation: 1,
        reasonRef: "reason.omega.owner_revoked",
      }),
    );
    const model = projectIssue31OwnerPrivateReadModel(snapshot([strayRevocation]), {
      nowUnixSeconds: 1_100,
    });
    // This device's own grant is untouched, so the room stays available.
    expect(model.status).toBe("ready");
  });
});
