import {
  createSeal,
  createWrap,
  GIFT_WRAP_KIND,
  unwrapEventWithDetails,
  type GiftWrappedEvent,
  type Rumor,
  type WrapMaterial,
} from "nostr-effect/nip59";
import {
  EventId,
  EventKind,
  NostrEvent as NostrEventCodec,
  PublicKey,
  Tag,
  UnixTimestamp,
} from "nostr-effect/core";
import {
  finalizeEvent,
  getEventHash,
  getPublicKey,
  type Event,
  type EventTemplate,
} from "nostr-effect/pure";
import { Effect, Schema } from "effect";
import {
  MktValidationError,
  validatePublicHead,
  validateRawPrivateRecord,
  type ProfileSupport,
} from "./validation.js";

export const MktTransportCodeSchema = Schema.Literals([
  "invalid_private_record",
  "invalid_gift_wrap",
  "signer_mismatch",
  "wrapper_inner_created_at_mismatch",
  "wrapper_inner_kind_mismatch",
  "wrapper_inner_recipient_mismatch",
  "wrapper_inner_signer_mismatch",
  "wrapper_recipient_mismatch",
  "wrapper_transport_failure",
]);
export type MktTransportCode = typeof MktTransportCodeSchema.Type;

export class MktTransportError extends Schema.TaggedErrorClass<MktTransportError>()(
  "MktTransportError",
  { code: MktTransportCodeSchema, message: Schema.String },
) {}

export interface DeliveredPrivateRecord {
  readonly wrapId: string;
  readonly sealId: string;
  readonly rumorId: string;
  readonly receivedAt?: number;
  readonly verifiedProvenance: {
    readonly wrapId: string;
    readonly sealId: string;
    readonly rumorId: string;
  };
  readonly sourceProvenance: readonly string[];
  readonly raw: string;
  readonly event: Event;
}

export interface WrappedPrivateCopies {
  readonly counterparty: GiftWrappedEvent;
  readonly senderRecovery: GiftWrappedEvent;
}

export interface MktWrapMaterial extends WrapMaterial {
  readonly rumorIdentifier?: string;
  readonly validateKindTags?: boolean;
}

export interface MktUnwrapOptions {
  readonly validateKindTags?: boolean;
  readonly receivedAt?: number;
  readonly sourceProvenance?: readonly string[];
}

export type WrapperBindingCode =
  | "wrapper_inner_signer_mismatch"
  | "wrapper_inner_kind_mismatch"
  | "wrapper_inner_recipient_mismatch";

export function wrapperBindingError(input: {
  readonly sealPubkey: string;
  readonly rumorPubkey: string;
  readonly rumorKind: number;
  readonly innerPubkey: string;
  readonly innerKind: number;
  readonly recipient: string;
  readonly innerCounterparties: readonly {
    readonly pubkey: string;
    readonly role: string;
  }[];
}): WrapperBindingCode | undefined {
  if (input.sealPubkey !== input.rumorPubkey || input.innerPubkey !== input.rumorPubkey) {
    return "wrapper_inner_signer_mismatch";
  }
  if (input.innerKind !== input.rumorKind) return "wrapper_inner_kind_mismatch";
  if (
    input.innerPubkey !== input.recipient &&
    !input.innerCounterparties.some(
      (counterparty) =>
        counterparty.pubkey === input.recipient &&
        (counterparty.role === "requester" || counterparty.role === "provider"),
    )
  )
    return "wrapper_inner_recipient_mismatch";
  return undefined;
}

function requireRumorBindings(tags: readonly (readonly string[])[], recipient: string): void {
  const recipients = tags.filter((tag) => tag[0] === "p");
  const identifiers = tags.filter((tag) => tag[0] === "d");
  if (recipients.length !== 1 || recipients[0]?.length !== 2 || recipients[0]?.[1] !== recipient) {
    throw new Error("MKT rumor must contain exactly one recipient tag");
  }
  if (
    identifiers.length !== 1 ||
    identifiers[0]?.length !== 2 ||
    !/^[0-9a-f]{64}$/.test(identifiers[0]?.[1] ?? "")
  ) {
    throw new Error("MKT rumor must contain exactly one unique d tag");
  }
}

const RumorSchema = Schema.Struct({
  id: EventId,
  pubkey: PublicKey,
  created_at: UnixTimestamp,
  kind: EventKind,
  tags: Schema.Array(Tag),
  content: Schema.String,
});
const decodeRumor = Schema.decodeUnknownSync(RumorSchema);
const decodeNostrEventEffect = Schema.decodeUnknownEffect(NostrEventCodec);

export function serializeSignedEvent(event: Event): string {
  return JSON.stringify({
    id: event.id,
    pubkey: event.pubkey,
    created_at: event.created_at,
    kind: event.kind,
    tags: event.tags,
    content: event.content,
    sig: event.sig,
  });
}

export function buildSignedPublicHead(template: EventTemplate, privateKey: Uint8Array): Event {
  const event = finalizeEvent(
    { ...template, tags: template.tags.map((tag) => [...tag]) },
    privateKey,
  );
  return validatePublicHead(event);
}

function transportFailure(cause: unknown): MktValidationError | MktTransportError {
  return cause instanceof MktValidationError || cause instanceof MktTransportError
    ? cause
    : new MktTransportError({ code: "wrapper_transport_failure", message: String(cause) });
}

function wrapPrivateRecordSync(
  rawSignedEvent: string,
  senderPrivateKey: Uint8Array,
  recipientPublicKey: string,
  profiles: readonly ProfileSupport[],
  material: MktWrapMaterial = {},
): GiftWrappedEvent {
  const validated = validateRawPrivateRecord(
    rawSignedEvent,
    profiles,
    material.validateKindTags ?? true,
  );
  const senderPublicKey = getPublicKey(senderPrivateKey);
  if (validated.event.pubkey !== senderPublicKey) {
    throw new MktTransportError({
      code: "signer_mismatch",
      message: "private MKT signer does not match wrapping sender",
    });
  }
  const rumorIdentifier =
    material.rumorIdentifier ??
    Array.from(globalThis.crypto.getRandomValues(new Uint8Array(32)), (byte) =>
      byte.toString(16).padStart(2, "0"),
    ).join("");
  if (!/^[0-9a-f]{64}$/.test(rumorIdentifier)) {
    throw new MktTransportError({
      code: "invalid_private_record",
      message: "rumor identifier must be 64 lowercase hexadecimal characters",
    });
  }
  const rumorFields = {
    pubkey: senderPublicKey,
    created_at: validated.event.created_at,
    kind: validated.event.kind,
    tags: [
      ["p", recipientPublicKey],
      ["d", rumorIdentifier],
    ],
    content: rawSignedEvent,
  };
  const rumor: Rumor = decodeRumor({
    id: getEventHash(rumorFields),
    ...rumorFields,
  });
  const seal = createSeal(rumor, senderPrivateKey, recipientPublicKey, material);
  const canonicalSeal = {
    id: seal.id,
    pubkey: seal.pubkey,
    created_at: seal.created_at,
    kind: seal.kind,
    tags: seal.tags,
    content: seal.content,
    sig: seal.sig,
  };
  return createWrap(canonicalSeal, recipientPublicKey, material);
}

export const wrapPrivateRecordCopies = Effect.fn("NipMkt.wrapPrivateRecordCopies")(function* (
  rawSignedEvent: string,
  senderPrivateKey: Uint8Array,
  counterpartyPublicKey: string,
  profiles: readonly ProfileSupport[],
  counterpartyMaterial: MktWrapMaterial = {},
  recoveryMaterial: MktWrapMaterial = {},
) {
  const senderPublicKey = getPublicKey(senderPrivateKey);
  const counterparty = yield* wrapPrivateRecord(
    rawSignedEvent,
    senderPrivateKey,
    counterpartyPublicKey,
    profiles,
    counterpartyMaterial,
  );
  const senderRecovery = yield* wrapPrivateRecord(
    rawSignedEvent,
    senderPrivateKey,
    senderPublicKey,
    profiles,
    recoveryMaterial,
  );
  return { counterparty, senderRecovery };
});

function unwrapPrivateRecordSync(
  wrap: GiftWrappedEvent,
  recipientPrivateKey: Uint8Array,
  profiles: readonly ProfileSupport[],
  options: MktUnwrapOptions = {},
): DeliveredPrivateRecord {
  const recipientPublicKey = getPublicKey(recipientPrivateKey);
  const details = unwrapEventWithDetails(wrap, recipientPrivateKey);
  const rumor = details.rumor;
  requireRumorBindings(rumor.tags, recipientPublicKey);
  const validated = validateRawPrivateRecord(
    rumor.content,
    profiles,
    options.validateKindTags ?? true,
  );
  const bindingError = wrapperBindingError({
    sealPubkey: details.seal.pubkey,
    rumorPubkey: rumor.pubkey,
    rumorKind: rumor.kind,
    innerPubkey: validated.event.pubkey,
    innerKind: validated.event.kind,
    recipient: recipientPublicKey,
    innerCounterparties: validated.event.tags
      .filter((tag) => tag.length === 4 && tag[0] === "p")
      .map((tag) => ({ pubkey: tag[1] ?? "", role: tag[3] ?? "" })),
  });
  if (bindingError !== undefined) {
    throw new MktTransportError({ code: bindingError, message: bindingError.replaceAll("_", " ") });
  }
  if (validated.event.created_at !== rumor.created_at) {
    throw new MktTransportError({
      code: "wrapper_inner_created_at_mismatch",
      message: "wrapper inner timestamp does not match the rumor",
    });
  }
  const delivery = {
    wrapId: details.wrapId,
    sealId: details.sealId,
    rumorId: details.rumorId,
    verifiedProvenance: {
      wrapId: details.wrapId,
      sealId: details.sealId,
      rumorId: details.rumorId,
    },
    sourceProvenance: options.sourceProvenance ?? [],
    raw: rumor.content,
    event: validated.event,
  };
  return options.receivedAt === undefined
    ? delivery
    : { ...delivery, receivedAt: options.receivedAt };
}

export const signPublicHead = Effect.fn("NipMkt.signPublicHead")(
  (template: EventTemplate, privateKey: Uint8Array) =>
    Effect.try({
      try: () => buildSignedPublicHead(template, privateKey),
      catch: transportFailure,
    }),
);

export const wrapPrivateRecord = Effect.fn("NipMkt.wrapPrivateRecord")(
  (
    rawSignedEvent: string,
    senderPrivateKey: Uint8Array,
    recipientPublicKey: string,
    profiles: readonly ProfileSupport[],
    material: MktWrapMaterial = {},
  ) =>
    Effect.try({
      try: () =>
        wrapPrivateRecordSync(
          rawSignedEvent,
          senderPrivateKey,
          recipientPublicKey,
          profiles,
          material,
        ),
      catch: transportFailure,
    }),
);

export const unwrapPrivateRecord = Effect.fn("NipMkt.unwrapPrivateRecord")(function* (
  wrap: unknown,
  recipientPrivateKey: Uint8Array,
  profiles: readonly ProfileSupport[],
  options: MktUnwrapOptions = {},
) {
  const decoded = yield* decodeNostrEventEffect(wrap).pipe(
    Effect.mapError(
      (cause) =>
        new MktTransportError({
          code: "invalid_gift_wrap",
          message: `gift wrap structure is invalid: ${String(cause)}`,
        }),
    ),
  );
  if (decoded.kind !== GIFT_WRAP_KIND) {
    return yield* new MktTransportError({
      code: "invalid_gift_wrap",
      message: "gift wrap must use kind 1059",
    });
  }
  const giftWrap: GiftWrappedEvent = { ...decoded, kind: GIFT_WRAP_KIND };
  return yield* Effect.try({
    try: () => unwrapPrivateRecordSync(giftWrap, recipientPrivateKey, profiles, options),
    catch: transportFailure,
  });
});
