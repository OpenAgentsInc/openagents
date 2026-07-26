import { getEventHash, verifyEvent } from "nostr-effect/pure";
import { LocalKeySigner } from "nostr-effect/identity";

import {
  ISSUE31_PRIVATE_GIFT_WRAP_KIND,
  ISSUE31_PRIVATE_RUMOR_KIND,
  ISSUE31_PRIVATE_SEAL_KIND,
  decodeIssue31CommandRecord,
  decodeIssue31CommandRecordV2,
  decodeIssue31PairingRecord,
  type Issue31CommandRecord,
  type Issue31CommandRecordV2,
  type Issue31PairingRecord,
} from "./records.ts";
import {
  ISSUE31_OWNER_PROJECTION_SCHEMA,
  decodeIssue31OwnerProjectionRecord,
  type Issue31OwnerProjectionRecord,
} from "./owner-projection.ts";
import {
  ISSUE31_WITHHELD_SOURCES_SCHEMA,
  decodeIssue31WithheldSourcesRecord,
  type Issue31WithheldSourcesRecord,
} from "./withheld-sources.ts";
import {
  ISSUE31_ADJUNCT_DELIVERY_KEYS,
  ISSUE31_FULL_AUTO_ADJUNCT_RECORD_TYPE,
  ISSUE31_FULL_AUTO_ADJUNCT_SCHEMA,
  ISSUE31_HOST_ADJUNCT_RECORD_TYPE,
  ISSUE31_HOST_ADJUNCT_SCHEMA,
  decodeIssue31FullAutoAdjunct,
  decodeIssue31HostAdjunct,
  type Issue31FullAutoAdjunct,
  type Issue31HostAdjunct,
} from "../issue31-workroom/index.ts";

/**
 * The identity every delivered owner-private record must state (omega#49).
 *
 * `assertRecordIdentity` below is the reason this exists: a record that names
 * neither the host that signed it nor the device it was sent to cannot be
 * checked against the signed seal author and the gift wrap recipient, and a
 * record that skips that check is one a paired device would accept from a host
 * it never paired with.
 */
export type Issue31AdjunctDelivery<RecordType extends string> = Readonly<{
  recordType: RecordType;
  hostPublicKeyHex: string;
  devicePublicKeyHex: string;
  grantRef: string;
  expectedGeneration: number;
}>;

/** An omega#47 `host.v1` snapshot delivered to one admitted device. */
export type Issue31DeliveredHostAdjunct = Issue31HostAdjunct &
  Issue31AdjunctDelivery<typeof ISSUE31_HOST_ADJUNCT_RECORD_TYPE>;

/** An omega#47 `fullauto.v1` detail projection delivered to one device. */
export type Issue31DeliveredFullAutoAdjunct = Issue31FullAutoAdjunct &
  Issue31AdjunctDelivery<typeof ISSUE31_FULL_AUTO_ADJUNCT_RECORD_TYPE>;

export type Issue31PrivateRecord =
  | Issue31PairingRecord
  | Issue31CommandRecord
  | Issue31CommandRecordV2
  | Issue31OwnerProjectionRecord
  | Issue31WithheldSourcesRecord
  | Issue31DeliveredHostAdjunct
  | Issue31DeliveredFullAutoAdjunct;

const assertDeliveryStated = (
  adjunct: Readonly<Record<string, unknown>>,
  label: string,
): void => {
  for (const key of ISSUE31_ADJUNCT_DELIVERY_KEYS) {
    if (adjunct[key] === undefined) {
      throw new Error(`Issue 31 ${label} was delivered without stating its ${key}.`);
    }
  }
  // Delivering a snapshot of some *other* host under this host's signature is
  // the one substitution the seal author cannot rule out on its own, because
  // the seal proves who signed, not which host the body describes. The two
  // statements are made in one record so a reader can compare them.
  if (adjunct["hostPublicKeyHex"] === adjunct["devicePublicKeyHex"]) {
    throw new Error(`Issue 31 ${label} names one key as both host and device.`);
  }
};

/**
 * An omega#47 `host.v1` snapshot that has been addressed to a device.
 *
 * The undelivered form — the same snapshot with no delivery binding — stays
 * decodable by `decodeIssue31HostAdjunct` for the host's own panels. Only the
 * delivered form is admitted as an owner-private record, because only the
 * delivered form can be checked against the envelope that carried it.
 */
export const decodeIssue31DeliveredHostAdjunct = (
  value: unknown,
): Issue31DeliveredHostAdjunct => {
  const adjunct = decodeIssue31HostAdjunct(value);
  assertDeliveryStated(adjunct, "host adjunct");
  return adjunct as Issue31DeliveredHostAdjunct;
};

/** An omega#47 `fullauto.v1` detail projection addressed to a device. */
export const decodeIssue31DeliveredFullAutoAdjunct = (
  value: unknown,
): Issue31DeliveredFullAutoAdjunct => {
  const adjunct = decodeIssue31FullAutoAdjunct(value);
  assertDeliveryStated(adjunct, "Full Auto adjunct");
  return adjunct as Issue31DeliveredFullAutoAdjunct;
};

/** True when a private record is one of the two delivered omega#47 adjuncts. */
export const isIssue31DeliveredAdjunct = (
  record: Issue31PrivateRecord,
): record is Issue31DeliveredHostAdjunct | Issue31DeliveredFullAutoAdjunct =>
  record.schema === ISSUE31_HOST_ADJUNCT_SCHEMA ||
  record.schema === ISSUE31_FULL_AUTO_ADJUNCT_SCHEMA;

const HEX_64 = /^[0-9a-f]{64}$/;
const HEX_128 = /^[0-9a-f]{128}$/;
const NIP59_MAX_TIMESTAMP_SKEW_SECONDS = 2 * 24 * 60 * 60;

export const issue31PrivateEnvelopeTimestamps = (
  createdAt: number,
  randomUint32: () => number,
): Readonly<{ sealCreatedAt: number; wrapCreatedAt: number }> => {
  if (!Number.isSafeInteger(createdAt) || createdAt < 0) {
    throw new Error("Issue 31 private rumor timestamp is invalid.");
  }
  const skew = (): number => {
    const value = randomUint32();
    if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
      throw new Error("Issue 31 private timestamp randomness is invalid.");
    }
    return value % (Math.min(createdAt, NIP59_MAX_TIMESTAMP_SKEW_SECONDS) + 1);
  };
  return {
    sealCreatedAt: createdAt - skew(),
    wrapCreatedAt: createdAt - skew(),
  };
};

export interface Issue31NostrSigner {
  readonly getPublicKey: () => Promise<string>;
  readonly signEvent: (
    event: Readonly<{
      kind: number;
      content: string;
      tags: ReadonlyArray<ReadonlyArray<string>>;
      created_at?: number;
    }>,
  ) => Promise<Issue31SignedNostrEvent>;
  readonly nip44Encrypt: (recipientPublicKeyHex: string, plaintext: string) => Promise<string>;
  readonly nip44Decrypt: (senderPublicKeyHex: string, ciphertext: string) => Promise<string>;
}

export interface Issue31SignedNostrEvent {
  readonly id: string;
  readonly pubkey: string;
  readonly created_at: number;
  readonly kind: number;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: string;
  readonly sig: string;
}

export interface Issue31PrivateRumor {
  readonly id: string;
  readonly pubkey: string;
  readonly created_at: number;
  readonly kind: typeof ISSUE31_PRIVATE_RUMOR_KIND;
  readonly tags: ReadonlyArray<ReadonlyArray<string>>;
  readonly content: string;
}

const assertPublicKey = (value: string, label: string): void => {
  if (!HEX_64.test(value)) throw new Error(`${label} must be a lowercase 64-hex public key.`);
};

const assertSignedEvent = (
  value: unknown,
  expectedKind: number,
  label: string,
): Issue31SignedNostrEvent => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} is not an event.`);
  }
  const event = value as Readonly<Record<string, unknown>>;
  if (
    !HEX_64.test(typeof event["id"] === "string" ? event["id"] : "") ||
    !HEX_64.test(typeof event["pubkey"] === "string" ? event["pubkey"] : "") ||
    !HEX_128.test(typeof event["sig"] === "string" ? event["sig"] : "") ||
    !Number.isSafeInteger(event["created_at"]) ||
    event["kind"] !== expectedKind ||
    !Array.isArray(event["tags"]) ||
    typeof event["content"] !== "string"
  ) {
    throw new Error(`${label} has an invalid shape.`);
  }
  const signed = event as unknown as Issue31SignedNostrEvent;
  if (
    !verifyEvent({
      ...signed,
      tags: signed.tags.map((tag) => [...tag]),
    })
  )
    throw new Error(`${label} signature is invalid.`);
  return signed;
};

const assertRecordIdentity = (
  record: Issue31PrivateRecord,
  senderPublicKeyHex: string,
  recipientPublicKeyHex: string,
): void => {
  const deviceAuthored =
    record.recordType === "pairing_request" ||
    record.recordType === "pairing_response" ||
    record.recordType === "command_intent";
  const expectedSenderPublicKeyHex = deviceAuthored
    ? record.devicePublicKeyHex
    : record.hostPublicKeyHex;
  if (expectedSenderPublicKeyHex !== senderPublicKeyHex) {
    throw new Error(`Issue 31 ${record.recordType} has the wrong signed seal author.`);
  }
  const expectedRecipient =
    senderPublicKeyHex === record.hostPublicKeyHex
      ? record.devicePublicKeyHex
      : record.hostPublicKeyHex;
  if (expectedRecipient !== recipientPublicKeyHex) {
    throw new Error("Issue 31 private record recipient does not match the local signer.");
  }
};

const validateRecord = (record: Issue31PrivateRecord): Issue31PrivateRecord =>
  record.schema === "openagents.omega.issue31.pairing.v1"
    ? decodeIssue31PairingRecord(record)
    : record.schema === "openagents.omega.issue31.command.v1"
      ? decodeIssue31CommandRecord(record)
      : record.schema === "openagents.omega.issue31.command.v2"
        ? decodeIssue31CommandRecordV2(record)
        : record.schema === ISSUE31_WITHHELD_SOURCES_SCHEMA
          ? decodeIssue31WithheldSourcesRecord(record)
          : record.schema === ISSUE31_HOST_ADJUNCT_SCHEMA
            ? decodeIssue31DeliveredHostAdjunct(record)
            : record.schema === ISSUE31_FULL_AUTO_ADJUNCT_SCHEMA
              ? decodeIssue31DeliveredFullAutoAdjunct(record)
              : decodeIssue31OwnerProjectionRecord(record);

export const createIssue31PrivateEnvelope = async (
  input: Readonly<{
    signer: Issue31NostrSigner;
    recipientPublicKeyHex: string;
    record: Issue31PrivateRecord;
    randomSecretKey: () => Uint8Array;
    createdAt: number;
    sealCreatedAt: number;
    wrapCreatedAt: number;
  }>,
): Promise<
  Readonly<{
    rumor: Issue31PrivateRumor;
    giftWrap: Issue31SignedNostrEvent;
  }>
> => {
  assertPublicKey(input.recipientPublicKeyHex, "Issue 31 private recipient");
  const record = validateRecord(input.record);
  const senderPublicKeyHex = await input.signer.getPublicKey();
  assertPublicKey(senderPublicKeyHex, "Issue 31 private sender");
  assertRecordIdentity(record, senderPublicKeyHex, input.recipientPublicKeyHex);
  for (const [label, timestamp] of [
    ["seal", input.sealCreatedAt],
    ["wrap", input.wrapCreatedAt],
  ] as const) {
    if (
      !Number.isSafeInteger(timestamp) ||
      timestamp < Math.max(0, input.createdAt - NIP59_MAX_TIMESTAMP_SKEW_SECONDS) ||
      timestamp > input.createdAt
    ) {
      throw new Error(`Issue 31 private ${label} timestamp is outside the NIP-59 window.`);
    }
  }

  const rumorBase = {
    pubkey: senderPublicKeyHex,
    created_at: input.createdAt,
    kind: ISSUE31_PRIVATE_RUMOR_KIND,
    tags: [["p", input.recipientPublicKeyHex]],
    content: JSON.stringify(record),
  };
  const rumor: Issue31PrivateRumor = { ...rumorBase, id: getEventHash(rumorBase) };
  const sealedContent = await input.signer.nip44Encrypt(
    input.recipientPublicKeyHex,
    JSON.stringify(rumor),
  );
  const seal = await input.signer.signEvent({
    kind: ISSUE31_PRIVATE_SEAL_KIND,
    created_at: input.sealCreatedAt,
    tags: [],
    content: sealedContent,
  });

  const secretKey = input.randomSecretKey();
  let ephemeral: LocalKeySigner | null = null;
  try {
    ephemeral = LocalKeySigner.fromPrivateKey(secretKey);
    secretKey.fill(0);
    const wrapContent = await ephemeral.nip44Encrypt(
      input.recipientPublicKeyHex,
      JSON.stringify(seal),
    );
    const giftWrap = await ephemeral.signEvent({
      kind: ISSUE31_PRIVATE_GIFT_WRAP_KIND,
      created_at: input.wrapCreatedAt,
      tags: [["p", input.recipientPublicKeyHex]],
      content: wrapContent,
    });
    return { rumor, giftWrap };
  } finally {
    secretKey.fill(0);
    ephemeral?.dispose();
  }
};

export const createIssue31PrivateGiftWrap = async (
  input: Parameters<typeof createIssue31PrivateEnvelope>[0],
): Promise<Issue31SignedNostrEvent> => (await createIssue31PrivateEnvelope(input)).giftWrap;

export const unwrapIssue31PrivateGiftWrap = async (
  input: Readonly<{
    signer: Issue31NostrSigner;
    giftWrap: unknown;
    requireIssue31Record?: boolean;
  }>,
): Promise<
  Readonly<{
    rumor: Issue31PrivateRumor;
    record: Issue31PrivateRecord | null;
  }>
> => {
  const giftWrap = assertSignedEvent(
    input.giftWrap,
    ISSUE31_PRIVATE_GIFT_WRAP_KIND,
    "Issue 31 gift wrap",
  );
  const recipientPublicKeyHex = await input.signer.getPublicKey();
  assertPublicKey(recipientPublicKeyHex, "Issue 31 local recipient");
  if (!giftWrap.tags.some((tag) => tag[0] === "p" && tag[1] === recipientPublicKeyHex)) {
    throw new Error("Issue 31 gift wrap is not addressed to the local signer.");
  }
  const sealJson = await input.signer.nip44Decrypt(giftWrap.pubkey, giftWrap.content);
  const seal = assertSignedEvent(
    JSON.parse(sealJson) as unknown,
    ISSUE31_PRIVATE_SEAL_KIND,
    "Issue 31 seal",
  );
  const rumorJson = await input.signer.nip44Decrypt(seal.pubkey, seal.content);
  const rumorValue = JSON.parse(rumorJson) as unknown;
  if (rumorValue === null || typeof rumorValue !== "object" || Array.isArray(rumorValue)) {
    throw new Error("Issue 31 rumor is not an event.");
  }
  const rumor = rumorValue as Readonly<Record<string, unknown>>;
  if (
    rumor["kind"] !== ISSUE31_PRIVATE_RUMOR_KIND ||
    rumor["pubkey"] !== seal.pubkey ||
    !HEX_64.test(typeof rumor["id"] === "string" ? rumor["id"] : "") ||
    !Number.isSafeInteger(rumor["created_at"]) ||
    !Array.isArray(rumor["tags"]) ||
    typeof rumor["content"] !== "string"
  ) {
    throw new Error("Issue 31 rumor has an invalid shape.");
  }
  const privateRumor = rumor as unknown as Issue31PrivateRumor;
  if (
    getEventHash({
      ...privateRumor,
      tags: privateRumor.tags.map((tag) => [...tag]),
    }) !== privateRumor.id
  ) {
    throw new Error("Issue 31 rumor identifier is invalid.");
  }
  if (!privateRumor.tags.some((tag) => tag[0] === "p" && tag[1] === recipientPublicKeyHex)) {
    throw new Error("Issue 31 rumor is not addressed to the local signer.");
  }
  let record: Issue31PrivateRecord | null = null;
  let contentValue: unknown;
  try {
    contentValue = JSON.parse(privateRumor.content) as unknown;
  } catch {
    contentValue = null;
  }
  if (contentValue !== null && typeof contentValue === "object" && !Array.isArray(contentValue)) {
    const schema = (contentValue as Readonly<Record<string, unknown>>)["schema"];
    if (schema === "openagents.omega.issue31.pairing.v1") {
      record = decodeIssue31PairingRecord(contentValue);
    } else if (schema === "openagents.omega.issue31.command.v1") {
      record = decodeIssue31CommandRecord(contentValue);
    } else if (schema === "openagents.omega.issue31.command.v2") {
      record = decodeIssue31CommandRecordV2(contentValue);
    } else if (schema === ISSUE31_OWNER_PROJECTION_SCHEMA) {
      record = decodeIssue31OwnerProjectionRecord(contentValue);
    } else if (schema === ISSUE31_WITHHELD_SOURCES_SCHEMA) {
      record = decodeIssue31WithheldSourcesRecord(contentValue);
    } else if (schema === ISSUE31_HOST_ADJUNCT_SCHEMA) {
      record = decodeIssue31DeliveredHostAdjunct(contentValue);
    } else if (schema === ISSUE31_FULL_AUTO_ADJUNCT_SCHEMA) {
      record = decodeIssue31DeliveredFullAutoAdjunct(contentValue);
    }
  }
  if (record === null && input.requireIssue31Record !== false) {
    throw new Error("Issue 31 private rumor content is not an Issue 31 record.");
  }
  if (record !== null) assertRecordIdentity(record, seal.pubkey, recipientPublicKeyHex);
  return { rumor: privateRumor, record };
};
