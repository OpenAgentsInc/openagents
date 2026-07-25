import { getEventHash, verifyEvent } from "nostr-effect/pure";
import { LocalKeySigner } from "nostr-effect/identity";

import {
  ISSUE31_PRIVATE_GIFT_WRAP_KIND,
  ISSUE31_PRIVATE_RUMOR_KIND,
  ISSUE31_PRIVATE_SEAL_KIND,
  decodeIssue31CommandRecord,
  decodeIssue31PairingRecord,
  type Issue31CommandRecord,
  type Issue31PairingRecord,
} from "./records.ts";

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
  record: Issue31PairingRecord | Issue31CommandRecord,
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

const validateRecord = (
  record: Issue31PairingRecord | Issue31CommandRecord,
): Issue31PairingRecord | Issue31CommandRecord =>
  record.schema === "openagents.omega.issue31.pairing.v1"
    ? decodeIssue31PairingRecord(record)
    : decodeIssue31CommandRecord(record);

export const createIssue31PrivateEnvelope = async (
  input: Readonly<{
    signer: Issue31NostrSigner;
    recipientPublicKeyHex: string;
    record: Issue31PairingRecord | Issue31CommandRecord;
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
    record: Issue31PairingRecord | Issue31CommandRecord | null;
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
  let record: Issue31PairingRecord | Issue31CommandRecord | null = null;
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
    }
  }
  if (record === null && input.requireIssue31Record !== false) {
    throw new Error("Issue 31 private rumor content is not an Issue 31 record.");
  }
  if (record !== null) assertRecordIdentity(record, seal.pubkey, recipientPublicKeyHex);
  return { rumor: privateRumor, record };
};
