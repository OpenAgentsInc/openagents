import { Effect, Schema } from "effect";
import {
  OfferingStatusSchema,
  ProviderStatusSchema,
  decodePublicHead,
  parseJsonRejectingDuplicateMembers,
} from "@openagentsinc/nip-mkt";
import type { DiscoveredOffering, OfferingSide } from "@openagentsinc/mkt-swp-pair";

export const MKT_DISCOVERY_SUBSCRIPTION_ID = "openagents-swap-heads-v1";
export const MKT_DISCOVERY_LIMIT = 128;
export const MKT_DISCOVERY_MAX_HEADS_PER_KIND = 64;
export const MKT_DISCOVERY_MAX_FRAME_BYTES = 512 * 1024;

export const mktDiscoveryRequest = (subscriptionId = MKT_DISCOVERY_SUBSCRIPTION_ID): string =>
  JSON.stringify(["REQ", subscriptionId, { kinds: [39600, 39601], limit: MKT_DISCOVERY_LIMIT }]);

export const mktDiscoveryClose = (subscriptionId = MKT_DISCOVERY_SUBSCRIPTION_ID): string =>
  JSON.stringify(["CLOSE", subscriptionId]);

export class MktDiscoveryError extends Schema.TaggedErrorClass<MktDiscoveryError>()(
  "MktSwp.MktDiscoveryError",
  {
    code: Schema.Literals([
      "invalid_frame",
      "invalid_event",
      "unexpected_kind",
      "head_bound",
      "auth_required",
      "subscription_closed",
      "projection_failed",
    ]),
    detail: Schema.String,
  },
) {}

export interface MktDiscoveryHead {
  readonly address: string;
  readonly distinct: string;
  readonly status: string;
  readonly publishedAtSeconds: number;
  readonly observedAtSeconds: number;
  readonly event: Effect.Success<ReturnType<typeof decodePublicHead>>;
  readonly content: Readonly<Record<string, Schema.Json>>;
}

export interface MktDiscoverySnapshot {
  readonly relayUrl: string;
  readonly subscriptionId: string;
  readonly completedAtSeconds: number;
  readonly providers: readonly MktDiscoveryHead[];
  readonly offerings: readonly MktDiscoveryHead[];
  readonly rejectedFrames: number;
}

export type MktDiscoveryFrame =
  | { readonly type: "head"; readonly kind: 39600 | 39601; readonly address: string }
  | { readonly type: "eose" }
  | { readonly type: "notice"; readonly detail: string }
  | { readonly type: "ignored" };

type MutableSnapshot = {
  providers: Map<string, MktDiscoveryHead>;
  offerings: Map<string, MktDiscoveryHead>;
};

const newSnapshot = (): MutableSnapshot => ({
  providers: new Map(),
  offerings: new Map(),
});

const isJson = Schema.is(Schema.Json);

const byAddress = (left: MktDiscoveryHead, right: MktDiscoveryHead): number =>
  left.address.localeCompare(right.address);

const failure = (code: MktDiscoveryError["code"], detail: string): MktDiscoveryError =>
  new MktDiscoveryError({ code, detail });

type ValidatedPublicHead = Effect.Success<ReturnType<typeof decodePublicHead>>;

const requiredTag = (event: ValidatedPublicHead, name: string): string => {
  const matches = event.tags.filter((tag) => tag[0] === name);
  const value = matches[0]?.[1];
  if (matches.length !== 1 || value === undefined || value === "") {
    throw failure("invalid_event", `event must contain exactly one ${name} tag`);
  }
  return value;
};

const jsonObject = (input: unknown, label: string): Readonly<Record<string, Schema.Json>> => {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw failure("invalid_event", `${label} must be a JSON object`);
  }
  const entries = Object.entries(input);
  if (!entries.every(([, value]) => isJson(value))) {
    throw failure("invalid_event", `${label} contains a non-JSON value`);
  }
  return Object.fromEntries(entries);
};

const headFromEvent = (event: ValidatedPublicHead, observedAtSeconds: number): MktDiscoveryHead => {
  const distinct = requiredTag(event, "d");
  const publishedAt = requiredTag(event, "published_at");
  if (!/^(0|[1-9][0-9]*)$/.test(publishedAt)) {
    throw failure("invalid_event", "published_at must be a decimal timestamp");
  }
  const publishedAtSeconds = Number(publishedAt);
  if (!Number.isSafeInteger(publishedAtSeconds)) {
    throw failure("invalid_event", "published_at exceeds the safe timestamp range");
  }
  const content = jsonObject(
    parseJsonRejectingDuplicateMembers(event.content),
    "discovery content",
  );
  return {
    address: `${event.kind}:${event.pubkey}:${distinct}`,
    distinct,
    status: requiredTag(event, "status"),
    publishedAtSeconds,
    observedAtSeconds,
    event,
    content,
  };
};

const shouldReplace = (current: MktDiscoveryHead, incoming: MktDiscoveryHead): boolean =>
  incoming.event.created_at > current.event.created_at ||
  (incoming.event.created_at === current.event.created_at && incoming.event.id < current.event.id);

export class MktDiscoveryBook {
  private active = newSnapshot();
  private pending = newSnapshot();
  private hasActiveSnapshot = false;
  private loadingSnapshot = true;
  private rejectedFrames = 0;

  beginSnapshot(): void {
    this.pending = newSnapshot();
    this.loadingSnapshot = true;
    this.rejectedFrames = 0;
  }

  ingestText(
    text: string,
    observedAtSeconds: number,
    subscriptionId = MKT_DISCOVERY_SUBSCRIPTION_ID,
  ): Effect.Effect<MktDiscoveryFrame, MktDiscoveryError> {
    const book = this;
    return Effect.gen(function* () {
      const parsed = yield* Effect.try({
        try: () => {
          if (!Number.isSafeInteger(observedAtSeconds) || observedAtSeconds < 0) {
            throw failure(
              "invalid_frame",
              "relay observation time must be a non-negative safe integer",
            );
          }
          if (new TextEncoder().encode(text).byteLength > MKT_DISCOVERY_MAX_FRAME_BYTES) {
            throw failure(
              "invalid_frame",
              `relay frame exceeds ${MKT_DISCOVERY_MAX_FRAME_BYTES} bytes`,
            );
          }
          const value = parseJsonRejectingDuplicateMembers(text);
          if (!Array.isArray(value) || typeof value[0] !== "string") {
            throw failure("invalid_frame", "relay frame must be a typed JSON array");
          }
          return value;
        },
        catch: (cause) =>
          cause instanceof MktDiscoveryError ? cause : failure("invalid_frame", String(cause)),
      });
      const type = parsed[0];
      if (type === "AUTH") {
        return yield* failure("auth_required", "discovery relay requested NIP-42 authentication");
      }
      if (type === "EVENT") {
        if (parsed.length !== 3 || parsed[1] !== subscriptionId) {
          return { type: "ignored" } as const;
        }
        const event = yield* decodePublicHead(JSON.stringify(parsed[2])).pipe(
          Effect.mapError((cause) => failure("invalid_event", String(cause))),
        );
        return yield* Effect.try({
          try: () => {
            if (event.kind !== 39600 && event.kind !== 39601) {
              throw failure("unexpected_kind", "relay returned an unrequested event kind");
            }
            if (
              event.created_at > observedAtSeconds + 900 ||
              Number(requiredTag(event, "published_at")) > observedAtSeconds + 900
            ) {
              throw failure("invalid_event", "discovery timestamp exceeds the future-skew bound");
            }
            const head = headFromEvent(event, observedAtSeconds);
            const target = book.loadingSnapshot ? book.pending : book.active;
            const heads = event.kind === 39600 ? target.providers : target.offerings;
            const current = heads.get(head.address);
            if (current !== undefined && !shouldReplace(current, head)) {
              return { type: "ignored" } as const;
            }
            if (current === undefined && heads.size >= MKT_DISCOVERY_MAX_HEADS_PER_KIND) {
              throw failure(
                "head_bound",
                `verified ${event.kind} head bound ${MKT_DISCOVERY_MAX_HEADS_PER_KIND} reached`,
              );
            }
            heads.set(head.address, head);
            return {
              type: "head",
              kind: event.kind,
              address: head.address,
            } as const;
          },
          catch: (cause) =>
            cause instanceof MktDiscoveryError ? cause : failure("invalid_event", String(cause)),
        });
      }
      if (type === "EOSE" && parsed[1] === subscriptionId) {
        if (parsed.length !== 2) {
          return yield* failure("invalid_frame", "EOSE frame has invalid arity");
        }
        if (!book.loadingSnapshot) {
          return { type: "ignored" } as const;
        }
        book.active = book.pending;
        book.pending = newSnapshot();
        book.hasActiveSnapshot = true;
        book.loadingSnapshot = false;
        return { type: "eose" } as const;
      }
      if (type === "CLOSED" && parsed[1] === subscriptionId) {
        return yield* failure(
          "subscription_closed",
          typeof parsed[2] === "string" ? parsed[2] : "relay closed discovery subscription",
        );
      }
      if (type === "NOTICE") {
        return {
          type: "notice",
          detail: typeof parsed[1] === "string" ? parsed[1] : "relay notice",
        } as const;
      }
      return { type: "ignored" } as const;
    }).pipe(
      Effect.tapError(() =>
        Effect.sync(() => {
          this.rejectedFrames += 1;
        }),
      ),
    );
  }

  snapshot(
    relayUrl: string,
    completedAtSeconds: number,
    subscriptionId = MKT_DISCOVERY_SUBSCRIPTION_ID,
  ): Effect.Effect<MktDiscoverySnapshot, MktDiscoveryError> {
    if (!this.hasActiveSnapshot) {
      return Effect.fail(failure("invalid_frame", "discovery snapshot is incomplete before EOSE"));
    }
    return Effect.succeed({
      relayUrl,
      subscriptionId,
      completedAtSeconds,
      // The package targets ES2022, so sort a fresh array instead of using ES2023 toSorted.
      providers: [...this.active.providers.values()].sort(byAddress),
      // The package targets ES2022, so sort a fresh array instead of using ES2023 toSorted.
      offerings: [...this.active.offerings.values()].sort(byAddress),
      rejectedFrames: this.rejectedFrames,
    });
  }
}

const AvailabilitySchema = Schema.Literals(["available", "limited", "unavailable"]);
const OfferingSideSchema = Schema.Struct({
  input_asset_id: Schema.NonEmptyString,
  output_asset_id: Schema.NonEmptyString,
  min: Schema.String.check(Schema.isPattern(/^(0|[1-9][0-9]*)$/)),
  max: Schema.String.check(Schema.isPattern(/^(0|[1-9][0-9]*)$/)),
  fee_bps: Schema.String.check(Schema.isPattern(/^(0|[1-9][0-9]*)$/)),
});
const OfferingContentSchema = Schema.StructWithRest(
  Schema.Struct({
    mkt_swp: Schema.StructWithRest(
      Schema.Struct({
        availability: AvailabilitySchema,
        sides: Schema.Array(OfferingSideSchema),
      }),
      [Schema.Record(Schema.String, Schema.Json)],
    ),
  }),
  [Schema.Record(Schema.String, Schema.Json)],
);

const decodeOfferingContent = Schema.decodeUnknownSync(OfferingContentSchema);
const decodeOfferingStatus = Schema.decodeUnknownSync(OfferingStatusSchema);
const decodeProviderStatus = Schema.decodeUnknownSync(ProviderStatusSchema);

export const projectDiscoveredOfferings = (
  snapshot: MktDiscoverySnapshot,
): Effect.Effect<readonly DiscoveredOffering[], MktDiscoveryError> =>
  Effect.try({
    try: () => {
      const providers = new Map(snapshot.providers.map((head) => [head.address, head]));
      return snapshot.offerings.map((offering): DiscoveredOffering => {
        const providerAddress = requiredTag(offering.event, "provider");
        const provider = providers.get(providerAddress);
        if (provider === undefined) {
          throw failure(
            "projection_failed",
            `offering ${offering.address} references an absent provider`,
          );
        }
        const content = decodeOfferingContent(offering.content);
        return {
          offeringAddress: offering.address,
          providerAddress,
          offeringStatus: decodeOfferingStatus(offering.status),
          providerStatus: decodeProviderStatus(provider.status),
          availability: content.mkt_swp.availability,
          publishedAtSeconds: offering.publishedAtSeconds,
          observedAtSeconds: offering.observedAtSeconds,
          sides: content.mkt_swp.sides.map(
            (side): OfferingSide => ({
              inputAssetId: side.input_asset_id,
              outputAssetId: side.output_asset_id,
              min: side.min,
              max: side.max,
              feeBps: side.fee_bps,
            }),
          ),
        };
      });
    },
    catch: (cause) =>
      cause instanceof MktDiscoveryError ? cause : failure("projection_failed", String(cause)),
  });
