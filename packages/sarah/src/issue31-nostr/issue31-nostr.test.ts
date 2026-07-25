import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

import { LocalKeySigner } from "nostr-effect/identity";
import { getEventHash } from "nostr-effect/pure";
import { describe, expect, test } from "vite-plus/test";

import {
  ISSUE31_COMMAND_SCHEMA,
  ISSUE31_COMMAND_SCHEMA_V2,
  ISSUE31_HOST_ANNOUNCEMENT_SCHEMA,
  ISSUE31_HOST_ANNOUNCEMENT_SCHEMA_V2,
  ISSUE31_PAIRING_SCHEMA,
  createIssue31PrivateGiftWrap,
  decodeIssue31CommandRecord,
  decodeIssue31CommandRecordV2,
  decodeIssue31HostAnnouncementV2,
  decodeIssue31HostAnnouncement,
  decodeIssue31OwnerProjectionRecord,
  decodeIssue31PairingRecord,
  foldIssue31Grant,
  reconcileIssue31Commands,
  unwrapIssue31PrivateGiftWrap,
  type Issue31CommandIntent,
  type Issue31CommandResult,
  type Issue31PairingEvent,
  type Issue31PairingRecord,
} from "./index.ts";

describe("Issue 31 command v2 and owner projection", () => {
  test("decodes canonical discovery, command, handling, and source projection fixtures", () => {
    const discovery = decodeIssue31HostAnnouncementV2(
      readFixture("openagents.omega.issue31.host_discovery.v2.canonical.json"),
    );
    const intent = decodeIssue31CommandRecordV2(
      readFixture("openagents.omega.issue31.command.v2.canonical-intent.json"),
    );
    const result = decodeIssue31CommandRecordV2(
      readFixture("openagents.omega.issue31.command.v2.canonical-result.json"),
    );
    const projection = decodeIssue31OwnerProjectionRecord(
      readFixture("openagents.omega.issue31.owner_projection.v1.canonical.json"),
    );
    expect(discovery.schema).toBe(ISSUE31_HOST_ANNOUNCEMENT_SCHEMA_V2);
    expect(discovery.protocols).toEqual([
      ISSUE31_PAIRING_SCHEMA,
      ISSUE31_COMMAND_SCHEMA,
      ISSUE31_COMMAND_SCHEMA_V2,
    ]);
    expect(intent.recordType).toBe("command_intent");
    expect(result).toMatchObject({ status: "accepted", handledAt: 1_784_937_610 });
    expect(projection).toMatchObject({
      sourceEventId: "b".repeat(64),
      sourceRole: "owner",
      sourceAuthorPublicKeyHex: projection.hostPublicKeyHex,
    });
  });

  // The read-state, reminder, authority-receipt, and engram projection bodies
  // are the omega#46 exits that had no shared fixture. The digests are pinned
  // identically in the Omega peer test
  // (`crates/omega_effectd/src/issue31_nostr.rs`), so a one-sided edit to the
  // shared bytes fails in both repositories rather than drifting silently.
  const OWNER_PROJECTION_BODY_FIXTURES = [
    [
      "canonical-read-state",
      "efd96dbe997e021c8e77300a802ab929b8c05981a1029c509b94d47410afc264",
      "read_state",
    ],
    [
      "canonical-reminder",
      "d21b2168d32d3c8e76294502b9a30a75a6f5f4f15c5195ac0c160fad15539fe3",
      "reminder",
    ],
    [
      "canonical-authority-receipt",
      "50d97118aec8931e624856246ae5e187bb6944f750052383f89472c4e9e27733",
      "authority_receipt",
    ],
    [
      "canonical-engram",
      "d499644feb77cb7d61b35fda9a4fbafe0b06fcc86e33a61985fbe36c1a819dae",
      "engram",
    ],
  ] as const;

  const OWNER_PROJECTION_NEGATIVE_FIXTURES = [
    ["negative-read-state-role", "1073644580d2c8d8768866c81a26cb8b044b13da4297383d54962ff949982baa"],
    [
      "negative-read-state-version",
      "9b134615eb992b2395c59dfc72abe8be3d472dd69c8dff73b7ec670757ebcfba",
    ],
    [
      "negative-reminder-pending-without-not-before",
      "e0f8a8c6a6f22c4b53326dbeb193eeda0832b8b8cbe0c2aae02ca7e1b70a1cec",
    ],
    [
      "negative-authority-receipt-terminal-without-outcome",
      "737bbaa8fece20bf9b898f4f76f5bf6d4369a1b0a487984302f0cf9a8bd9cc3b",
    ],
    ["negative-engram-slug", "3997fb6d470f20e4796f4765d2406e6e3f4a629f090b3c4ef26e77274cf7b6ed"],
  ] as const;

  test("decodes every shared owner projection body the Omega host emits", () => {
    for (const [name, digest, kind] of OWNER_PROJECTION_BODY_FIXTURES) {
      const fileName = `openagents.omega.issue31.owner_projection.v1.${name}.json`;
      expect(fixtureDigest(fileName), `${name} fixture bytes changed`).toBe(digest);
      const record = decodeIssue31OwnerProjectionRecord(readFixture(fileName));
      expect(record.projection.kind).toBe(kind);
      expect(record.schema).toBe("openagents.omega.issue31.owner_projection.v1");
    }
  });

  test("refuses every shared owner projection body the Omega host must not emit", () => {
    for (const [name, digest] of OWNER_PROJECTION_NEGATIVE_FIXTURES) {
      const fileName = `openagents.omega.issue31.owner_projection.v1.${name}.json`;
      expect(fixtureDigest(fileName), `${name} fixture bytes changed`).toBe(digest);
      expect(
        () => decodeIssue31OwnerProjectionRecord(readFixture(fileName)),
        `${name} must be refused`,
      ).toThrow();
    }
  });

  test("keeps discovery v1 exact and rejects v2 action or projection excess fields", () => {
    expect(
      decodeIssue31HostAnnouncement(
        readFixture("openagents.omega.issue31.host_discovery.v1.canonical.json"),
      ).schema,
    ).toBe(ISSUE31_HOST_ANNOUNCEMENT_SCHEMA);
    const intent = readFixture(
      "openagents.omega.issue31.command.v2.canonical-intent.json",
    ) as Record<string, unknown>;
    expect(() =>
      decodeIssue31CommandRecordV2({
        ...intent,
        arguments: {
          ...(intent["arguments"] as object),
          actionRef: "action.issue31.sarah.interrupt",
        },
      }),
    ).toThrow();
    const projection = readFixture(
      "openagents.omega.issue31.owner_projection.v1.canonical.json",
    ) as Record<string, unknown>;
    expect(() =>
      decodeIssue31OwnerProjectionRecord({ ...projection, ownerSecret: "no" }),
    ).toThrow();
  });

  test("rejects invalid reminder time and read context byte/control bounds", () => {
    const common = {
      schema: ISSUE31_COMMAND_SCHEMA_V2,
      recordType: "command_intent",
      hostRef: "omega.host.local",
      hostPublicKeyHex: "1".repeat(64),
      devicePublicKeyHex: "2".repeat(64),
      grantRef: "grant.omega.device_1",
      idempotencyRef: "idempotency.issue31.test_1",
      expectedGeneration: 1,
      issuedAt: 100,
      expiresAt: 200,
    } as const;
    expect(() =>
      decodeIssue31CommandRecordV2({
        ...common,
        arguments: {
          kind: "reminder_create",
          actionRef: "action.issue31.reminder.create",
          reminderId: "a".repeat(32),
          notBefore: 150,
          expiration: 150,
        },
      }),
    ).toThrow(/expiration/);
    for (const contextRef of ["bad\ncontext", "é".repeat(129)]) {
      expect(() =>
        decodeIssue31CommandRecordV2({
          ...common,
          arguments: {
            kind: "read_state_patch",
            actionRef: "action.issue31.read_state.advance",
            slotId: "owner-mobile",
            clientId: "openagents-mobile",
            contextRef,
            readAt: 150,
          },
        }),
      ).toThrow();
    }
  });

  test("unwraps a host-authored per-record projection only for the admitted device", async () => {
    const projection = decodeIssue31OwnerProjectionRecord({
      ...(readFixture("openagents.omega.issue31.owner_projection.v1.canonical.json") as object),
      hostPublicKeyHex,
      devicePublicKeyHex,
      sourceAuthorPublicKeyHex: hostPublicKeyHex,
    });
    const wrapped = await createIssue31PrivateGiftWrap({
      signer: hostSigner,
      recipientPublicKeyHex: devicePublicKeyHex,
      record: projection,
      randomSecretKey: () => new Uint8Array(ephemeralOne),
      createdAt: 1_784_937_610,
      sealCreatedAt: 1_784_937_600,
      wrapCreatedAt: 1_784_937_590,
    });
    const unwrapped = await unwrapIssue31PrivateGiftWrap({
      signer: deviceSigner,
      giftWrap: wrapped,
    });
    expect(unwrapped.record?.schema).toBe("openagents.omega.issue31.owner_projection.v1");
    await expect(
      unwrapIssue31PrivateGiftWrap({ signer: hostSigner, giftWrap: wrapped }),
    ).rejects.toThrow(/not addressed/);
  });
});

const deviceSecret = Uint8Array.from({ length: 32 }, (_, index) => index + 1);
const hostSecret = Uint8Array.from({ length: 32 }, (_, index) => index + 33);
const ephemeralOne = Uint8Array.from({ length: 32 }, (_, index) => index + 65);
const ephemeralTwo = Uint8Array.from({ length: 32 }, (_, index) => index + 97);
const deviceSigner = LocalKeySigner.fromPrivateKey(deviceSecret);
const hostSigner = LocalKeySigner.fromPrivateKey(hostSecret);
const devicePublicKeyHex = deviceSigner.publicKey;
const hostPublicKeyHex = hostSigner.publicKey;
const sarahPublicKeyHex = "3".repeat(64);

const fixturePath = (name: string): URL =>
  new URL(`../../fixtures/issue31-nostr/${name}`, import.meta.url);

const fixtureDigest = (name: string): string =>
  createHash("sha256").update(readFileSync(fixturePath(name))).digest("hex");

const readFixture = (name: string): unknown =>
  JSON.parse(
    readFileSync(new URL(`../../fixtures/issue31-nostr/${name}`, import.meta.url), "utf8"),
  );

const grant = (overrides: Partial<Issue31PairingRecord> = {}): Issue31PairingRecord =>
  decodeIssue31PairingRecord({
    schema: ISSUE31_PAIRING_SCHEMA,
    recordType: "scoped_grant",
    hostRef: "omega.host.local",
    hostPublicKeyHex,
    sarahPublicKeyHex,
    devicePublicKeyHex,
    issuedAt: 1_000,
    pairingResponseEventId: "a".repeat(64),
    grantRef: "grant.omega.device_1",
    generation: 1,
    scopes: ["observe_issue31", "send_message"],
    expiresAt: 2_000,
    ...overrides,
  });

const pairingChain = (): ReadonlyArray<Issue31PairingEvent> => [
  {
    eventId: "c".repeat(64),
    record: decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "pairing_request",
      hostRef: "omega.host.local",
      hostPublicKeyHex,
      devicePublicKeyHex,
      issuedAt: 800,
      pairingRequestRef: "pairing.request.device_1",
      requestedScopes: ["observe_issue31", "send_message"],
      expiresAt: 1_300,
    }),
  },
  {
    eventId: "b".repeat(64),
    record: decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "pairing_challenge",
      hostRef: "omega.host.local",
      hostPublicKeyHex,
      devicePublicKeyHex,
      issuedAt: 900,
      pairingChallengeRef: "pairing.challenge.device_1",
      pairingRequestEventId: "c".repeat(64),
      challenge: "d".repeat(64),
      expiresAt: 1_300,
    }),
  },
  {
    eventId: "a".repeat(64),
    record: decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "pairing_response",
      hostRef: "omega.host.local",
      hostPublicKeyHex,
      devicePublicKeyHex,
      issuedAt: 950,
      pairingResponseRef: "pairing.response.device_1",
      pairingChallengeEventId: "b".repeat(64),
      challenge: "d".repeat(64),
      expiresAt: 1_300,
    }),
  },
];

const intent = (overrides: Partial<Issue31CommandIntent> = {}): Issue31CommandIntent =>
  decodeIssue31CommandRecord({
    schema: ISSUE31_COMMAND_SCHEMA,
    recordType: "command_intent",
    hostRef: "omega.host.local",
    hostPublicKeyHex,
    devicePublicKeyHex,
    grantRef: "grant.omega.device_1",
    actionRef: "action.omega.full_auto.stop",
    idempotencyRef: "idempotency.omega.stop_1",
    expectedGeneration: 3,
    argumentsRef: "arguments.omega.none",
    issuedAt: 1_200,
    expiresAt: 1_500,
    ...overrides,
  }) as Issue31CommandIntent;

const result = (overrides: Partial<Issue31CommandResult> = {}): Issue31CommandResult =>
  decodeIssue31CommandRecord({
    schema: ISSUE31_COMMAND_SCHEMA,
    recordType: "command_result",
    hostRef: "omega.host.local",
    hostPublicKeyHex,
    devicePublicKeyHex,
    grantRef: "grant.omega.device_1",
    intentEventId: "b".repeat(64),
    actionRef: "action.omega.full_auto.stop",
    idempotencyRef: "idempotency.omega.stop_1",
    expectedGeneration: 3,
    status: "completed",
    outcomeRef: "outcome.omega.stop_1",
    completedAt: 1_300,
    ...overrides,
  }) as Issue31CommandResult;

describe("Issue 31 Nostr records", () => {
  test("keeps the canonical discovery, pairing, and command fixtures decodable", () => {
    const discovery = decodeIssue31HostAnnouncement(
      readFixture("openagents.omega.issue31.host_discovery.v1.canonical.json"),
    );
    const pairing = decodeIssue31PairingRecord(
      readFixture("openagents.omega.issue31.pairing.v1.canonical.json"),
    );
    const commandIntent = decodeIssue31CommandRecord(
      readFixture("openagents.omega.issue31.command.v1.canonical-intent.json"),
    );
    const commandResult = decodeIssue31CommandRecord(
      readFixture("openagents.omega.issue31.command.v1.canonical-result.json"),
    );

    expect(discovery.schema).toBe(ISSUE31_HOST_ANNOUNCEMENT_SCHEMA);
    expect(discovery.sarahPublicKeyHex).toBe(sarahPublicKeyHex);
    expect(pairing.recordType).toBe("scoped_grant");
    expect("sarahPublicKeyHex" in pairing && pairing.sarahPublicKeyHex).toBe(sarahPublicKeyHex);
    expect(commandIntent.recordType).toBe("command_intent");
    expect(commandResult.recordType).toBe("command_result");
    expect(
      reconcileIssue31Commands([
        { eventId: "4".repeat(64), record: commandIntent },
        { eventId: "5".repeat(64), record: commandResult },
      ])[0]?.result?.status,
    ).toBe("completed");
  });

  test("decodes a bounded NIP-89 Omega host discovery record", () => {
    const record = decodeIssue31HostAnnouncement({
      schema: ISSUE31_HOST_ANNOUNCEMENT_SCHEMA,
      hostRef: "omega.host.local",
      hostPublicKeyHex,
      sarahPublicKeyHex,
      displayName: "Local Omega",
      protocols: [ISSUE31_PAIRING_SCHEMA, ISSUE31_COMMAND_SCHEMA],
      relayUrls: ["wss://relay.example.com"],
      generation: 4,
      issuedAt: 1_000,
      expiresAt: 2_000,
    });
    expect(record.generation).toBe(4);
    expect(() =>
      decodeIssue31HostAnnouncement({
        ...record,
        sarahPublicKeyHex: hostPublicKeyHex,
      }),
    ).toThrow(/must be distinct/);
    expect(() =>
      decodeIssue31HostAnnouncement({
        ...record,
        relayUrls: ["wss://user:secret@relay.example.com"],
      }),
    ).toThrow(/unsafe relay URL/);
    expect(() =>
      decodeIssue31HostAnnouncement({
        ...record,
        relayUrls: ["wss://relay.example.com/#private"],
      }),
    ).toThrow(/unsafe relay URL/);
    expect(() =>
      decodeIssue31HostAnnouncement({
        ...record,
        relayUrls: ["wss://relay.example.com/?token=private"],
      }),
    ).toThrow(/unsafe relay URL/);
  });

  test("makes revocation terminal for one grant and requires a new grant ref to re-pair", () => {
    const renewal = decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "grant_renewal",
      hostRef: "omega.host.local",
      hostPublicKeyHex,
      sarahPublicKeyHex,
      devicePublicKeyHex,
      issuedAt: 1_100,
      grantRef: "grant.omega.device_1",
      previousGrantEventId: "1".repeat(64),
      priorGeneration: 1,
      generation: 2,
      scopes: ["observe_issue31", "send_message"],
      expiresAt: 2_100,
    });
    const revocation = decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "grant_revocation",
      hostRef: "omega.host.local",
      hostPublicKeyHex,
      sarahPublicKeyHex,
      devicePublicKeyHex,
      issuedAt: 1_400,
      grantRef: "grant.omega.device_1",
      generation: 1,
      reasonRef: "reason.omega.owner_revoked",
    });
    const folded = foldIssue31Grant(
      [
        { eventId: "2".repeat(64), record: revocation },
        { eventId: "1".repeat(64), record: renewal },
        { eventId: "2".repeat(64), record: revocation },
      ],
      "grant.omega.device_1",
    );
    expect(folded).toMatchObject({ status: "revoked", sourceEventId: "2".repeat(64) });
  });

  test("requires exact renewal lineage and fails closed on generation forks", () => {
    const grantRef = "grant.omega.device_1";
    const initial = grant();
    const renewal = decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "grant_renewal",
      hostRef: "omega.host.local",
      hostPublicKeyHex,
      sarahPublicKeyHex,
      devicePublicKeyHex,
      issuedAt: 1_100,
      grantRef: "grant.omega.device_1",
      previousGrantEventId: "1".repeat(64),
      priorGeneration: 1,
      generation: 2,
      scopes: ["observe_issue31", "send_message"],
      expiresAt: 2_100,
    });
    expect(
      foldIssue31Grant(
        [
          ...pairingChain(),
          { eventId: "1".repeat(64), record: initial },
          { eventId: "2".repeat(64), record: renewal },
        ],
        grantRef,
      ),
    ).toMatchObject({ status: "active", generation: 2, sourceEventId: "2".repeat(64) });
    expect(() =>
      foldIssue31Grant(
        [
          ...pairingChain(),
          { eventId: "1".repeat(64), record: initial },
          {
            eventId: "2".repeat(64),
            record: decodeIssue31PairingRecord({
              ...renewal,
              previousGrantEventId: "f".repeat(64),
            }),
          },
        ],
        grantRef,
      ),
    ).toThrow(/lineage/);
    expect(() =>
      foldIssue31Grant(
        [
          ...pairingChain(),
          { eventId: "1".repeat(64), record: initial },
          {
            eventId: "2".repeat(64),
            record: decodeIssue31PairingRecord({
              ...renewal,
              sarahPublicKeyHex: "4".repeat(64),
            }),
          },
        ],
        grantRef,
      ),
    ).toThrow(/identity fork/);
    expect(() =>
      foldIssue31Grant(
        [
          ...pairingChain(),
          { eventId: "1".repeat(64), record: initial },
          {
            eventId: "3".repeat(64),
            record: grant({ scopes: ["observe_issue31"] }),
          },
        ],
        grantRef,
      ),
    ).toThrow(/forks/);
    expect(() =>
      foldIssue31Grant([{ eventId: "1".repeat(64), record: initial }], grantRef),
    ).toThrow(/no pairing response/);
    expect(() =>
      decodeIssue31PairingRecord({
        ...initial,
        sarahPublicKeyHex: hostPublicKeyHex,
      }),
    ).toThrow(/must be distinct/);
  });

  test("keeps relay acknowledgement outside terminal command convergence", () => {
    const pending = reconcileIssue31Commands([{ eventId: "b".repeat(64), record: intent() }]);
    expect(pending[0]?.result).toBeNull();
    expect(
      reconcileIssue31Commands([
        { eventId: "b".repeat(64), record: intent() },
        { eventId: "c".repeat(64), record: result() },
      ])[0]?.result?.status,
    ).toBe("completed");
    expect(() =>
      reconcileIssue31Commands([
        { eventId: "b".repeat(64), record: intent() },
        { eventId: "c".repeat(64), record: result() },
        {
          eventId: "d".repeat(64),
          record: result({ status: "failed", outcomeRef: "outcome.omega.stop_failed" }),
        },
      ]),
    ).toThrow(/terminal result conflict/);
    expect(() =>
      reconcileIssue31Commands([
        { eventId: "b".repeat(64), record: intent() },
        {
          eventId: "e".repeat(64),
          record: intent({ actionRef: "action.omega.full_auto.pause" }),
        },
      ]),
    ).toThrow(/idempotency conflict/);
  });

  test("round-trips a signer-only NIP-17/44/59 record and deduplicates by rumor id", async () => {
    const first = await createIssue31PrivateGiftWrap({
      signer: hostSigner,
      recipientPublicKeyHex: devicePublicKeyHex,
      record: grant(),
      randomSecretKey: () => new Uint8Array(ephemeralOne),
      createdAt: 1_100,
      sealCreatedAt: 1_000,
      wrapCreatedAt: 900,
    });
    const second = await createIssue31PrivateGiftWrap({
      signer: hostSigner,
      recipientPublicKeyHex: devicePublicKeyHex,
      record: grant(),
      randomSecretKey: () => new Uint8Array(ephemeralTwo),
      createdAt: 1_100,
      sealCreatedAt: 1_000,
      wrapCreatedAt: 800,
    });
    const unwrappedFirst = await unwrapIssue31PrivateGiftWrap({
      signer: deviceSigner,
      giftWrap: first,
    });
    const unwrappedSecond = await unwrapIssue31PrivateGiftWrap({
      signer: deviceSigner,
      giftWrap: second,
    });
    expect(first.id).not.toBe(second.id);
    expect(unwrappedFirst.rumor.id).toBe(unwrappedSecond.rumor.id);
    expect(unwrappedFirst.record?.recordType).toBe("scoped_grant");
    await expect(
      unwrapIssue31PrivateGiftWrap({ signer: hostSigner, giftWrap: first }),
    ).rejects.toThrow(/not addressed/);
  });

  test("enforces the record-specific seal author", async () => {
    await expect(
      createIssue31PrivateGiftWrap({
        signer: deviceSigner,
        recipientPublicKeyHex: hostPublicKeyHex,
        record: grant(),
        randomSecretKey: () => new Uint8Array(ephemeralOne),
        createdAt: 1_100,
        sealCreatedAt: 1_000,
        wrapCreatedAt: 900,
      }),
    ).rejects.toThrow(/wrong signed seal author/);
    await expect(
      createIssue31PrivateGiftWrap({
        signer: hostSigner,
        recipientPublicKeyHex: devicePublicKeyHex,
        record: intent({
          hostPublicKeyHex,
          devicePublicKeyHex,
        }),
        randomSecretKey: () => new Uint8Array(ephemeralOne),
        createdAt: 1_200,
        sealCreatedAt: 1_100,
        wrapCreatedAt: 1_000,
      }),
    ).rejects.toThrow(/wrong signed seal author/);
  });

  test("rejects malformed content that claims an Issue 31 schema", async () => {
    const malformedRumorBase = {
      pubkey: hostPublicKeyHex,
      created_at: 1_100,
      kind: 14,
      tags: [["p", devicePublicKeyHex]],
      content: JSON.stringify({ schema: ISSUE31_PAIRING_SCHEMA, recordType: "scoped_grant" }),
    };
    const malformedRumor = {
      ...malformedRumorBase,
      id: getEventHash(malformedRumorBase),
    };
    const seal = await hostSigner.signEvent({
      kind: 13,
      created_at: 1_000,
      tags: [],
      content: await hostSigner.nip44Encrypt(devicePublicKeyHex, JSON.stringify(malformedRumor)),
    });
    const ephemeral = LocalKeySigner.fromPrivateKey(new Uint8Array(ephemeralOne));
    try {
      const giftWrap = await ephemeral.signEvent({
        kind: 1_059,
        created_at: 900,
        tags: [["p", devicePublicKeyHex]],
        content: await ephemeral.nip44Encrypt(devicePublicKeyHex, JSON.stringify(seal)),
      });
      await expect(
        unwrapIssue31PrivateGiftWrap({
          signer: deviceSigner,
          giftWrap,
          requireIssue31Record: false,
        }),
      ).rejects.toThrow();
    } finally {
      ephemeral.dispose();
    }
  });
});
