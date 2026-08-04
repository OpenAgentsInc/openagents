import { Effect, Schema } from "effect";
import { verifyEvent, type Event } from "nostr-effect/pure";
import {
  CancelActionSchema,
  CloseOutcomeSchema,
  DescriptorStatusSchema,
  MKT_LIMITS,
  NamedIdentifierSchema,
  NostrEventSchema,
  OfferingStatusSchema,
  PRIVATE_MKT_KINDS,
  ProfileEnvelopeSchema,
  ProviderStatusSchema,
  PublicReceiptOutcomeSchema,
  PUBLIC_MKT_KINDS,
  QuoteTypeSchema,
  ReservationSchema,
  StatusStateSchema,
} from "./generated.js";

export const MktValidationCodeSchema = Schema.Literals([
  "duplicate_json_member",
  "envelope_mismatch",
  "event_too_large",
  "collection_limit",
  "invalid_event_shape",
  "invalid_event_signature",
  "invalid_kind",
  "invalid_identifier",
  "invalid_json",
  "invalid_reference",
  "tag_grammar",
  "unsupported_critical_member",
  "unsupported_profile",
  "unsupported_profile_version",
]);
export type MktValidationCode = typeof MktValidationCodeSchema.Type;

export class MktValidationError extends Schema.TaggedErrorClass<MktValidationError>()(
  "MktValidationError",
  { code: MktValidationCodeSchema, message: Schema.String },
) {}

export interface ProfileSupport {
  readonly id: string;
  readonly version: number;
  readonly criticalMembers?: readonly string[];
  readonly understoodMembers?: readonly string[];
}

export interface ValidatedMktRecord {
  readonly event: Event;
  readonly envelope: typeof ProfileEnvelopeSchema.Type;
  readonly raw: string;
}

const decodeEvent = Schema.decodeUnknownSync(NostrEventSchema);
const decodeEventEffect = Schema.decodeUnknownEffect(NostrEventSchema);
const decodeEnvelope = Schema.decodeUnknownSync(ProfileEnvelopeSchema);
const decodeNamedIdentifier = Schema.decodeUnknownSync(NamedIdentifierSchema);
const decodeProviderStatus = Schema.decodeUnknownSync(ProviderStatusSchema);
const decodeOfferingStatus = Schema.decodeUnknownSync(OfferingStatusSchema);
const decodeDescriptorStatus = Schema.decodeUnknownSync(DescriptorStatusSchema);
const decodePublicReceiptOutcome = Schema.decodeUnknownSync(PublicReceiptOutcomeSchema);
const decodeQuoteType = Schema.decodeUnknownSync(QuoteTypeSchema);
const decodeReservation = Schema.decodeUnknownSync(ReservationSchema);
const decodeStatusState = Schema.decodeUnknownSync(StatusStateSchema);
const decodeCancelAction = Schema.decodeUnknownSync(CancelActionSchema);
const decodeCloseOutcome = Schema.decodeUnknownSync(CloseOutcomeSchema);
const PRIVATE_KIND_SET = new Set<number>(PRIVATE_MKT_KINDS);
const PUBLIC_KIND_SET = new Set<number>(PUBLIC_MKT_KINDS);
const HEX_64 = /^[0-9a-f]{64}$/;
const POSITIVE_INTEGER = /^[1-9][0-9]*$/;
const DECIMAL_TIMESTAMP = /^(0|[1-9][0-9]*)$/;
const EVENT_MARKERS = new Set([
  "rfq",
  "quote",
  "order",
  "previous",
  "status",
  "cancel",
  "close",
  "evidence",
  "settlement",
]);
const COUNTERPARTY_ROLES = new Set(["requester", "provider"]);

function fail(code: MktValidationCode, message: string): never {
  throw new MktValidationError({ code, message });
}

function validationFailure(cause: unknown): MktValidationError {
  return cause instanceof MktValidationError
    ? cause
    : new MktValidationError({ code: "invalid_event_shape", message: String(cause) });
}

function mutableEvent(decoded: typeof NostrEventSchema.Type): Event {
  const tags: string[][] = decoded.tags.map((tag) => Array.from(tag));
  return { ...decoded, tags };
}

function utf8Bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function tagsNamed(event: Pick<Event, "tags">, name: string): string[][] {
  return event.tags.filter((tag) => tag[0] === name);
}

function exactlyOne(event: Pick<Event, "tags">, name: string): string[] {
  const matches = tagsNamed(event, name);
  if (matches.length !== 1) fail("tag_grammar", `expected exactly one ${name} tag`);
  return matches[0]!;
}

function exactlyOneMarker(event: Pick<Event, "tags">, name: string, marker: string): string[] {
  const matches = tagsNamed(event, name).filter((tag) => tag[3] === marker);
  if (matches.length !== 1) fail("tag_grammar", `expected exactly one ${marker} ${name} tag`);
  return matches[0]!;
}

function requireTagShape(tag: readonly string[], length: number, name: string): void {
  if (tag.length !== length) fail("tag_grammar", `${name} tag has the wrong shape`);
}

function requireNamedIdentifier(value: string, label: string): void {
  try {
    decodeNamedIdentifier(value);
  } catch {
    fail("invalid_identifier", `${label} is invalid`);
  }
}

function requireReferenceIdentifier(value: string, label: string): void {
  try {
    decodeNamedIdentifier(value);
  } catch {
    fail("invalid_reference", `${label} is invalid`);
  }
}

function requireHex(value: string, label: string): void {
  if (!HEX_64.test(value))
    fail("invalid_identifier", `${label} must be 64 lowercase hexadecimal characters`);
}

function requirePositive(value: string, label: string): number {
  if (!POSITIVE_INTEGER.test(value)) fail("tag_grammar", `${label} must be a positive integer`);
  const decoded = Number(value);
  if (!Number.isSafeInteger(decoded)) fail("tag_grammar", `${label} is too large`);
  return decoded;
}

function requireTimestamp(value: string, label: string): number {
  if (!DECIMAL_TIMESTAMP.test(value)) fail("tag_grammar", `${label} must be a decimal timestamp`);
  const decoded = Number(value);
  if (!Number.isSafeInteger(decoded)) fail("tag_grammar", `${label} is too large`);
  return decoded;
}

function requireEnum(decoder: (value: unknown) => unknown, value: string, label: string): void {
  try {
    decoder(value);
  } catch {
    fail("tag_grammar", `${label} is invalid`);
  }
}

function requireProfileTag(tag: readonly string[]): { id: string; version: number } {
  requireTagShape(tag, 3, "profile");
  requireNamedIdentifier(tag[1]!, "profile id");
  return { id: tag[1]!, version: requirePositive(tag[2]!, "profile version") };
}

function requireEventReference(tag: readonly string[], marker: string): void {
  if (tag.length !== 4 || tag[0] !== "e" || tag[3] !== marker) {
    fail("invalid_reference", `expected ${marker} event reference`);
  }
  if (!HEX_64.test(tag[1]!)) fail("invalid_reference", `${marker} event id is invalid`);
}

function requireCounterparty(event: Pick<Event, "tags">, role: "requester" | "provider"): void {
  const matches = tagsNamed(event, "p").filter((tag) => tag[3] === role);
  if (matches.length !== 1) fail("tag_grammar", `expected exactly one ${role} counterparty`);
}

function validateReferenceTags(event: Pick<Event, "tags" | "pubkey">): void {
  const references = event.tags.filter((tag) => tag[0] === "e");
  if (references.length > MKT_LIMITS.causal_or_evidence_references) {
    fail("collection_limit", "too many causal or evidence references");
  }
  for (const tag of references) {
    if (tag.length !== 4 || !EVENT_MARKERS.has(tag[3]!))
      fail("invalid_reference", "invalid event reference");
    if (!HEX_64.test(tag[1]!)) fail("invalid_reference", "event reference ID is invalid");
  }
  for (const tag of event.tags.filter(
    (candidate) => candidate[0] === "a" && candidate[3] === "offering",
  )) {
    if (tag.length !== 4) fail("invalid_reference", "invalid address reference");
    const match = /^39601:([0-9a-f]{64}):(.+)$/.exec(tag[1]!);
    if (match === null) fail("invalid_reference", "invalid offering address");
    requireReferenceIdentifier(match[2]!, "offering id");
  }
}

function validateCommonBounds(event: Pick<Event, "tags" | "pubkey">): void {
  if (event.tags.length > MKT_LIMITS.tags) fail("collection_limit", "too many tags");
  if (tagsNamed(event, "p").length > MKT_LIMITS.counterparties)
    fail("collection_limit", "too many counterparties");
  if (tagsNamed(event, "profile").length > MKT_LIMITS.profiles)
    fail("collection_limit", "too many profiles");
  const hints = event.tags.filter(
    (tag) => ((tag[0] === "p" || tag[0] === "e" || tag[0] === "a") && tag[2]) || tag[0] === "relay",
  );
  if (hints.length > MKT_LIMITS.relay_or_endpoint_hints)
    fail("collection_limit", "too many relay hints");
  validateReferenceTags(event);
}

function validatePublicTags(event: Event): void {
  const identifier = exactlyOne(event, "d");
  requireTagShape(identifier, 2, "d");
  if (event.kind === 39603) {
    if (identifier[1]!.length === 0) fail("invalid_identifier", "receipt d identifier is empty");
  } else {
    requireNamedIdentifier(identifier[1]!, "d identifier");
  }

  if (event.kind === 39600) {
    const status = exactlyOne(event, "status");
    requireTagShape(status, 2, "status");
    requireEnum(decodeProviderStatus, status[1]!, "provider status");
    const profiles = tagsNamed(event, "profile");
    if (profiles.length === 0) fail("tag_grammar", "provider profile requires a profile tag");
    const seen = new Set<string>();
    for (const tag of profiles) {
      const profile = requireProfileTag(tag);
      const key = `${profile.id}:${profile.version}`;
      if (seen.has(key)) fail("tag_grammar", "duplicate profile version");
      seen.add(key);
    }
    const published = exactlyOne(event, "published_at");
    requireTagShape(published, 2, "published_at");
    requireTimestamp(published[1]!, "published_at");
  } else if (event.kind === 39601) {
    const profile = exactlyOne(event, "profile");
    requireProfileTag(profile);
    const status = exactlyOne(event, "status");
    requireTagShape(status, 2, "status");
    requireEnum(decodeOfferingStatus, status[1]!, "offering status");
    const provider = exactlyOne(event, "provider");
    requireTagShape(provider, 2, "provider");
    const match = /^39600:([0-9a-f]{64}):(.+)$/.exec(provider[1]!);
    if (match === null || match[1] !== event.pubkey)
      fail("invalid_reference", "provider address is invalid");
    requireReferenceIdentifier(match[2]!, "provider id");
    const published = exactlyOne(event, "published_at");
    requireTagShape(published, 2, "published_at");
    requireTimestamp(published[1]!, "published_at");
  } else if (event.kind === 39602) {
    const version = exactlyOne(event, "version");
    requireTagShape(version, 2, "version");
    requirePositive(version[1]!, "version");
    const digest = exactlyOne(event, "x");
    requireTagShape(digest, 2, "x");
    requireHex(digest[1]!, "profile digest");
    const retrieval = exactlyOne(event, "r");
    requireTagShape(retrieval, 2, "r");
    try {
      const url = new URL(retrieval[1]!);
      if (
        (url.protocol !== "http:" && url.protocol !== "https:") ||
        url.host.length === 0 ||
        /\s/.test(retrieval[1]!)
      ) {
        fail("tag_grammar", "profile retrieval URL is invalid");
      }
    } catch {
      fail("tag_grammar", "profile retrieval URL is invalid");
    }
    const status = exactlyOne(event, "status");
    requireTagShape(status, 2, "status");
    requireEnum(decodeDescriptorStatus, status[1]!, "descriptor status");
  } else {
    const profile = exactlyOne(event, "profile");
    requireProfileTag(profile);
    const outcome = exactlyOne(event, "outcome");
    requireTagShape(outcome, 2, "outcome");
    requireEnum(decodePublicReceiptOutcome, outcome[1]!, "receipt outcome");
    const close = exactlyOne(event, "x");
    requireTagShape(close, 2, "x");
    if (!HEX_64.test(close[1]!)) fail("invalid_reference", "close event id is invalid");
    const role = exactlyOne(event, "role");
    requireTagShape(role, 2, "role");
    requireNamedIdentifier(role[1]!, "receipt role");
  }
}

function validatePrivateTags(
  event: Event,
  validateKindTags: boolean,
): { profile: string; version: number; session: string } {
  const identifier = exactlyOne(event, "d");
  requireTagShape(identifier, 2, "d");
  requireHex(identifier[1]!, "d identifier");
  const session = exactlyOne(event, "session");
  requireTagShape(session, 2, "session");
  requireHex(session[1]!, "session id");
  const profile = requireProfileTag(exactlyOne(event, "profile"));
  const counterparties = tagsNamed(event, "p");
  if (counterparties.length === 0) fail("tag_grammar", "private record requires a counterparty");
  let roleMarkedCounterparties = 0;
  for (const tag of counterparties) {
    if (tag.length !== 4 || !COUNTERPARTY_ROLES.has(tag[3]!)) continue;
    roleMarkedCounterparties += 1;
    requireHex(tag[1]!, "counterparty public key");
  }
  if (roleMarkedCounterparties === 0)
    fail("tag_grammar", "private record requires a role-marked counterparty");
  const alt = exactlyOne(event, "alt");
  requireTagShape(alt, 2, "alt");
  if (alt[1]!.length === 0) fail("tag_grammar", "alt text must not be empty");
  if (
    utf8Bytes(alt[1]!) > 128 ||
    Array.from(alt[1]!).some((character) => {
      const code = character.charCodeAt(0);
      return code < 0x20 || code === 0x7f;
    })
  ) {
    fail("tag_grammar", "alt text is invalid");
  }

  if (!validateKindTags) {
    return { profile: profile.id, version: profile.version, session: session[1]! };
  }

  if (event.kind === 39604) {
    requireCounterparty(event, "provider");
    const offering = exactlyOneMarker(event, "a", "offering");
    const match = /^39601:[0-9a-f]{64}:(.+)$/.exec(offering[1]!);
    if (match === null || offering[3] !== "offering")
      fail("invalid_reference", "offering reference is invalid");
    requireReferenceIdentifier(match[1]!, "offering id");
    const expiration = exactlyOne(event, "expiration");
    requireTagShape(expiration, 2, "expiration");
    requireTimestamp(expiration[1]!, "expiration");
  } else if (event.kind === 39605) {
    requireEventReference(exactlyOneMarker(event, "e", "rfq"), "rfq");
    requireCounterparty(event, "requester");
    const expiration = exactlyOne(event, "expiration");
    requireTagShape(expiration, 2, "expiration");
    requireTimestamp(expiration[1]!, "expiration");
    const quote = exactlyOne(event, "quote");
    requireTagShape(quote, 2, "quote");
    requireEnum(decodeQuoteType, quote[1]!, "quote type");
    const reservation = exactlyOne(event, "reservation");
    requireTagShape(reservation, 2, "reservation");
    requireEnum(decodeReservation, reservation[1]!, "reservation");
  } else if (event.kind === 39606) {
    requireEventReference(exactlyOneMarker(event, "e", "quote"), "quote");
    requireCounterparty(event, "provider");
  } else if (event.kind === 39607) {
    requireEventReference(exactlyOneMarker(event, "e", "order"), "order");
    const sequence = exactlyOne(event, "seq");
    requireTagShape(sequence, 2, "seq");
    const sequenceNumber = requireTimestamp(sequence[1]!, "seq");
    const state = exactlyOne(event, "state");
    requireTagShape(state, 2, "state");
    requireEnum(decodeStatusState, state[1]!, "status state");
    const previous = tagsNamed(event, "e").filter((tag) => tag[3] === "previous");
    if (sequenceNumber === 0 && previous.length !== 0)
      fail("tag_grammar", "sequence zero must not have previous");
    if (sequenceNumber > 0 && previous.length !== 1)
      fail("tag_grammar", "status requires previous");
    if (previous[0]) requireEventReference(previous[0], "previous");
  } else if (event.kind === 39608) {
    requireEventReference(exactlyOneMarker(event, "e", "order"), "order");
    const action = exactlyOne(event, "action");
    requireTagShape(action, 2, "action");
    requireEnum(decodeCancelAction, action[1]!, "cancel action");
    const reason = exactlyOne(event, "reason");
    requireTagShape(reason, 2, "reason");
    if (reason[1]!.length === 0) fail("tag_grammar", "cancel reason must not be empty");
  } else {
    requireEventReference(exactlyOneMarker(event, "e", "order"), "order");
    const outcome = exactlyOne(event, "outcome");
    requireTagShape(outcome, 2, "outcome");
    requireEnum(decodeCloseOutcome, outcome[1]!, "close outcome");
    const terminal = exactlyOne(event, "terminal_at");
    requireTagShape(terminal, 2, "terminal_at");
    requireTimestamp(terminal[1]!, "terminal_at");
  }
  return { profile: profile.id, version: profile.version, session: session[1]! };
}

export function parseJsonRejectingDuplicateMembers(raw: string): unknown {
  let index = 0;
  const whitespace = () => {
    while (/\s/.test(raw[index] ?? "")) index += 1;
  };
  const stringToken = (): string => {
    const start = index;
    if (raw[index] !== '"') fail("invalid_json", "expected JSON string");
    index += 1;
    while (index < raw.length) {
      const character = raw[index]!;
      if (character === '"') {
        index += 1;
        try {
          return JSON.parse(raw.slice(start, index)) as string;
        } catch {
          fail("invalid_json", "invalid JSON string");
        }
      }
      if (character === "\\") {
        index += 2;
      } else {
        if (character.charCodeAt(0) < 0x20)
          fail("invalid_json", "control character in JSON string");
        index += 1;
      }
    }
    fail("invalid_json", "unterminated JSON string");
  };
  const value = (): void => {
    whitespace();
    const character = raw[index];
    if (character === '"') {
      stringToken();
    } else if (character === "{") {
      index += 1;
      whitespace();
      const members = new Set<string>();
      if (raw[index] === "}") {
        index += 1;
        return;
      }
      while (true) {
        whitespace();
        const key = stringToken();
        if (members.has(key)) fail("duplicate_json_member", `duplicate JSON member: ${key}`);
        members.add(key);
        whitespace();
        if (raw[index] !== ":") fail("invalid_json", "expected colon");
        index += 1;
        value();
        whitespace();
        if (raw[index] === "}") {
          index += 1;
          return;
        }
        if (raw[index] !== ",") fail("invalid_json", "expected comma");
        index += 1;
      }
    } else if (character === "[") {
      index += 1;
      whitespace();
      if (raw[index] === "]") {
        index += 1;
        return;
      }
      while (true) {
        value();
        whitespace();
        if (raw[index] === "]") {
          index += 1;
          return;
        }
        if (raw[index] !== ",") fail("invalid_json", "expected comma");
        index += 1;
      }
    } else {
      const match =
        /^(?:true|false|null|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)/.exec(
          raw.slice(index),
        );
      if (match === null) fail("invalid_json", "invalid JSON value");
      index += match[0].length;
    }
  };
  value();
  whitespace();
  if (index !== raw.length) fail("invalid_json", "trailing bytes after JSON value");
  try {
    return JSON.parse(raw);
  } catch {
    fail("invalid_json", "invalid JSON");
  }
}

function decodeNostrEvent(raw: string): Event {
  const value = parseJsonRejectingDuplicateMembers(raw);
  try {
    return mutableEvent(decodeEvent(value));
  } catch {
    fail("invalid_event_shape", "signed event structure is invalid");
  }
}

function decodeContentEnvelope(content: string): typeof ProfileEnvelopeSchema.Type {
  const value = parseJsonRejectingDuplicateMembers(content);
  try {
    return decodeEnvelope(value);
  } catch {
    fail("envelope_mismatch", "content envelope is invalid");
  }
}

export function validatePublicHead(event: Event): Event {
  if (!PUBLIC_KIND_SET.has(event.kind)) fail("invalid_kind", "event is not a public MKT head");
  const maximum =
    event.kind === 39603 ? MKT_LIMITS.receipt_content_bytes : MKT_LIMITS.discovery_content_bytes;
  if (utf8Bytes(event.content) > maximum) fail("event_too_large", "public content is too large");
  const content = parseJsonRejectingDuplicateMembers(event.content);
  if (typeof content !== "object" || content === null || Array.isArray(content)) {
    fail("invalid_json", "public MKT content must be a JSON object");
  }
  validateCommonBounds(event);
  validatePublicTags(event);
  return event;
}

export function validateRawPrivateRecordBase(
  raw: string,
  validateKindTags = true,
): ValidatedMktRecord {
  if (utf8Bytes(raw) > MKT_LIMITS.private_signed_record_bytes)
    fail("event_too_large", "private signed record is too large");
  const event = decodeNostrEvent(raw);
  return validateDecodedPrivateRecord(raw, event, validateKindTags);
}

function validateDecodedPrivateRecord(
  raw: string,
  event: Event,
  validateKindTags: boolean,
): ValidatedMktRecord {
  if (!PRIVATE_KIND_SET.has(event.kind)) fail("invalid_kind", "event is not a private MKT record");
  if (!verifyEvent(event))
    fail("invalid_event_signature", "private MKT signature or ID is invalid");
  validateCommonBounds(event);
  const common = validatePrivateTags(event, validateKindTags);
  const envelope = decodeContentEnvelope(event.content);
  if (
    envelope.profile !== common.profile ||
    envelope.profile_version !== common.version ||
    envelope.session_id !== common.session
  ) {
    fail("envelope_mismatch", "content envelope does not match event tags");
  }
  return { event, envelope, raw };
}

export function validateRawPrivateRecord(
  raw: string,
  profiles: readonly ProfileSupport[],
  validateKindTags = true,
): ValidatedMktRecord {
  const validated = validateRawPrivateRecordBase(raw, validateKindTags);
  return validateProfileSupport(validated, profiles);
}

function validateProfileSupport(
  validated: ValidatedMktRecord,
  profiles: readonly ProfileSupport[],
): ValidatedMktRecord {
  const byId = profiles.filter((profile) => profile.id === validated.envelope.profile);
  if (byId.length === 0) fail("unsupported_profile", "profile is not supported");
  const support = byId.find((profile) => profile.version === validated.envelope.profile_version);
  if (support === undefined)
    fail("unsupported_profile_version", "profile version is not supported");
  const envelopeMembers = new Set(Object.keys(validated.envelope));
  for (const member of support.criticalMembers ?? []) {
    if (envelopeMembers.has(member) && !(support.understoodMembers ?? []).includes(member)) {
      fail("unsupported_critical_member", `critical profile member ${member} is not understood`);
    }
  }
  return validated;
}

export function validateRawPublicHead(raw: string): Event {
  const event = decodeNostrEvent(raw);
  if (!verifyEvent(event)) fail("invalid_event_signature", "public MKT signature or ID is invalid");
  return validatePublicHead(event);
}

export const decodePrivateBase = Effect.fn("NipMkt.decodePrivateBase")(function* (
  raw: string,
  validateKindTags = true,
) {
  if (utf8Bytes(raw) > MKT_LIMITS.private_signed_record_bytes) {
    return yield* new MktValidationError({
      code: "event_too_large",
      message: "private signed record is too large",
    });
  }
  const value = yield* Effect.try({
    try: () => parseJsonRejectingDuplicateMembers(raw),
    catch: validationFailure,
  });
  const decoded = yield* decodeEventEffect(value).pipe(
    Effect.mapError(
      () =>
        new MktValidationError({
          code: "invalid_event_shape",
          message: "signed event structure is invalid",
        }),
    ),
  );
  return yield* Effect.try({
    try: () => validateDecodedPrivateRecord(raw, mutableEvent(decoded), validateKindTags),
    catch: validationFailure,
  });
});

export const decodePrivateWithProfiles = Effect.fn("NipMkt.decodePrivateWithProfiles")(function* (
  raw: string,
  profiles: readonly ProfileSupport[],
  validateKindTags = true,
) {
  const validated = yield* decodePrivateBase(raw, validateKindTags);
  return yield* Effect.try({
    try: () => validateProfileSupport(validated, profiles),
    catch: validationFailure,
  });
});

export const decodePublicHead = Effect.fn("NipMkt.decodePublicHead")(function* (raw: string) {
  const value = yield* Effect.try({
    try: () => parseJsonRejectingDuplicateMembers(raw),
    catch: validationFailure,
  });
  const decoded = yield* decodeEventEffect(value).pipe(
    Effect.mapError(
      () =>
        new MktValidationError({
          code: "invalid_event_shape",
          message: "signed event structure is invalid",
        }),
    ),
  );
  return yield* Effect.try({
    try: () => {
      const event = mutableEvent(decoded);
      if (!verifyEvent(event))
        fail("invalid_event_signature", "public MKT signature or ID is invalid");
      return validatePublicHead(event);
    },
    catch: validationFailure,
  });
});

export const validateRawPrivateRecordEffect = decodePrivateWithProfiles;
export const validateRawPublicHeadEffect = decodePublicHead;
