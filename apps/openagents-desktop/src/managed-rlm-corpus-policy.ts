/**
 * RLM-08 (#9144) — fail-closed policy boundary for a managed corpus source.
 *
 * The authenticated host binds owner, corpus, policy, and generation before it
 * gives the generic RLM engine a source ref. The adapter reads bounded pages
 * from the source of record. It does not materialize or persist a second raw
 * corpus copy.
 */

import { Context, Effect, Layer, Option, Schema, Stream } from "effect";
import {
  RlmCorpusError,
  RlmCorpusSource,
  type RlmCorpusEntry,
  type RlmCorpusHandle,
  type RlmCorpusInput,
  type RlmCorpusManifest,
  type RlmCorpusSourceShape,
  type RlmOrdinalRange,
  type RlmReadLimits,
  type RlmSourceAddress,
  type RlmValidatedSourceAddress,
} from "@openagentsinc/rlm";

export const MANAGED_RLM_CORPUS_BINDING_SCHEMA_ID =
  "openagents.managed_rlm.corpus_binding.v1" as const;

export const MANAGED_RLM_CORPUS_ADDRESS_SCHEMA_ID =
  "openagents.managed_rlm.corpus_source.v1" as const;

const ManagedRlmRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256));

const ManagedRlmPositiveInteger = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(1),
);

/**
 * Host-owned admission for one managed source. `sourceRef` is an opaque
 * capability reference. It does not contain raw corpus data or credentials.
 */
export const ManagedRlmCorpusBinding = Schema.Struct({
  schemaId: Schema.Literal(MANAGED_RLM_CORPUS_BINDING_SCHEMA_ID),
  sourceRef: ManagedRlmRef,
  ownerRef: ManagedRlmRef,
  corpusRef: ManagedRlmRef,
  policyRef: ManagedRlmRef,
  scopeRef: ManagedRlmRef,
  sourceGeneration: ManagedRlmPositiveInteger,
  maxEntriesPerRead: ManagedRlmPositiveInteger.pipe(Schema.check(Schema.isLessThanOrEqualTo(256))),
  maxCharsPerEntry: ManagedRlmPositiveInteger.pipe(
    Schema.check(Schema.isLessThanOrEqualTo(16_384)),
  ),
  maxScanEntries: ManagedRlmPositiveInteger.pipe(Schema.check(Schema.isLessThanOrEqualTo(4_096))),
});

export interface ManagedRlmCorpusBinding extends Schema.Schema.Type<
  typeof ManagedRlmCorpusBinding
> {}

/** Current authority facts returned with every managed store operation. */
export const ManagedRlmCorpusObservation = Schema.Struct({
  sourceRef: ManagedRlmRef,
  ownerRef: ManagedRlmRef,
  corpusRef: ManagedRlmRef,
  policyRef: ManagedRlmRef,
  scopeRef: ManagedRlmRef,
  sourceGeneration: ManagedRlmPositiveInteger,
  state: Schema.Literals(["active", "revoked"]),
});

export interface ManagedRlmCorpusObservation extends Schema.Schema.Type<
  typeof ManagedRlmCorpusObservation
> {}

export class ManagedRlmCorpusStoreError extends Schema.TaggedErrorClass<ManagedRlmCorpusStoreError>()(
  "ManagedRlmCorpus.StoreError",
  {
    reason: Schema.Literals([
      "unavailable",
      "not_found",
      "authority_not_granted",
      "revoked",
      "stale_generation",
      "invalid_response",
    ]),
    detailSafe: Schema.optionalKey(
      Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(512)),
    ),
  },
) {}

export interface ManagedRlmCorpusResolution {
  readonly observation: ManagedRlmCorpusObservation;
  readonly manifest: RlmCorpusManifest;
}

export interface ManagedRlmCorpusReadResult {
  readonly observation: ManagedRlmCorpusObservation;
  readonly entries: ReadonlyArray<RlmCorpusEntry>;
}

export interface ManagedRlmCorpusAddressResult {
  readonly observation: ManagedRlmCorpusObservation;
  readonly validated: RlmValidatedSourceAddress;
}

/**
 * The store is the source-of-record adapter. It must make each read and address
 * validation atomic with the returned authority observation.
 */
export interface ManagedRlmCorpusStoreShape {
  readonly resolve: (
    binding: ManagedRlmCorpusBinding,
  ) => Effect.Effect<ManagedRlmCorpusResolution, ManagedRlmCorpusStoreError>;
  readonly read: (
    input: Readonly<{
      binding: ManagedRlmCorpusBinding;
      range: RlmOrdinalRange;
      limits: RlmReadLimits;
    }>,
  ) => Effect.Effect<ManagedRlmCorpusReadResult, ManagedRlmCorpusStoreError>;
  readonly validateSourceAddress: (
    input: Readonly<{
      binding: ManagedRlmCorpusBinding;
      address: RlmSourceAddress;
    }>,
  ) => Effect.Effect<ManagedRlmCorpusAddressResult, ManagedRlmCorpusStoreError>;
}

export class ManagedRlmCorpusStore extends Context.Service<
  ManagedRlmCorpusStore,
  ManagedRlmCorpusStoreShape
>()("@openagentsinc/openagents-desktop/ManagedRlmCorpusStore") {}

/** Build the generic source input for one host-admitted managed binding. */
export const managedRlmCorpusInput = (
  binding: ManagedRlmCorpusBinding,
): Extract<RlmCorpusInput, { readonly _tag: "Source" }> => ({
  _tag: "Source",
  sourceRef: {
    addressSchemaId: MANAGED_RLM_CORPUS_ADDRESS_SCHEMA_ID,
    encodedAddress: binding.sourceRef,
  },
});

const corpusError = (reason: RlmCorpusError["reason"], detailSafe: string): RlmCorpusError =>
  new RlmCorpusError({ reason, detailSafe });

const mapStoreError = (error: ManagedRlmCorpusStoreError): RlmCorpusError =>
  corpusError(
    error.reason === "stale_generation" ? "changed" : "unavailable",
    error.reason === "stale_generation"
      ? "managed corpus generation changed"
      : "managed corpus is unavailable under the current authority",
  );

const verifyObservation = (
  binding: ManagedRlmCorpusBinding,
  observation: ManagedRlmCorpusObservation,
): Effect.Effect<ManagedRlmCorpusObservation, RlmCorpusError> => {
  if (
    observation.sourceRef !== binding.sourceRef ||
    observation.ownerRef !== binding.ownerRef ||
    observation.corpusRef !== binding.corpusRef ||
    observation.policyRef !== binding.policyRef ||
    observation.scopeRef !== binding.scopeRef
  ) {
    return Effect.fail(
      corpusError(
        "unavailable",
        "managed corpus authority did not match the admitted source scope",
      ),
    );
  }
  if (observation.sourceGeneration !== binding.sourceGeneration) {
    return Effect.fail(corpusError("changed", "managed corpus generation changed"));
  }
  if (observation.state !== "active") {
    return Effect.fail(corpusError("unavailable", "managed corpus access has been revoked"));
  }
  return Effect.succeed(observation);
};

const verifyManifest = (
  binding: ManagedRlmCorpusBinding,
  manifest: RlmCorpusManifest,
): Effect.Effect<RlmCorpusManifest, RlmCorpusError> =>
  manifest.corpusRef === binding.corpusRef && manifest.scopeRef === binding.scopeRef
    ? Effect.succeed(manifest)
    : Effect.fail(
        corpusError(
          "unavailable",
          "managed corpus manifest did not match the admitted source scope",
        ),
      );

const verifyEntries = (
  binding: ManagedRlmCorpusBinding,
  range: RlmOrdinalRange,
  limits: RlmReadLimits,
  entries: ReadonlyArray<RlmCorpusEntry>,
): Effect.Effect<ReadonlyArray<RlmCorpusEntry>, RlmCorpusError> => {
  let priorOrdinal = range.start - 1;
  if (entries.length > limits.maxEntries) {
    return Effect.fail(
      corpusError("unavailable", "managed corpus store exceeded the admitted read limit"),
    );
  }
  for (const entry of entries) {
    if (
      entry.scopeRef !== binding.scopeRef ||
      entry.ordinal < range.start ||
      entry.ordinal > range.endInclusive ||
      entry.ordinal <= priorOrdinal ||
      (entry.text?.length ?? 0) > limits.maxCharsPerEntry
    ) {
      return Effect.fail(
        corpusError(
          "unavailable",
          "managed corpus store returned data outside the admitted read scope",
        ),
      );
    }
    priorOrdinal = entry.ordinal;
  }
  return Effect.succeed(entries);
};

const boundedLimits = (binding: ManagedRlmCorpusBinding, limits: RlmReadLimits): RlmReadLimits => ({
  maxEntries: Math.min(limits.maxEntries, binding.maxEntriesPerRead),
  maxCharsPerEntry: Math.min(limits.maxCharsPerEntry, binding.maxCharsPerEntry),
});

/**
 * Make an RLM corpus source over the host-owned managed store. The generic RLM
 * engine receives only this standard source interface.
 */
export const makeManagedRlmCorpusSource = (
  binding: ManagedRlmCorpusBinding,
  store: ManagedRlmCorpusStoreShape,
): RlmCorpusSourceShape => {
  const read = Effect.fn("ManagedRlmCorpusSource.read")(function* (
    range: RlmOrdinalRange,
    requestedLimits: RlmReadLimits,
  ) {
    if (range.start > range.endInclusive) {
      return yield* Effect.fail(corpusError("invalid_range", "start > endInclusive"));
    }
    const limits = boundedLimits(binding, requestedLimits);
    const result = yield* store
      .read({ binding, range, limits })
      .pipe(Effect.mapError(mapStoreError));
    yield* verifyObservation(binding, result.observation);
    return yield* verifyEntries(binding, range, limits, result.entries);
  });

  const resolve = Effect.fn("ManagedRlmCorpusSource.resolve")(function* (input: RlmCorpusInput) {
    if (
      input._tag !== "Source" ||
      input.sourceRef.addressSchemaId !== MANAGED_RLM_CORPUS_ADDRESS_SCHEMA_ID ||
      input.sourceRef.encodedAddress !== binding.sourceRef
    ) {
      return yield* Effect.fail(
        corpusError("unavailable", "managed corpus source ref was not admitted by the host"),
      );
    }

    const resolution = yield* store.resolve(binding).pipe(Effect.mapError(mapStoreError));
    yield* verifyObservation(binding, resolution.observation);
    const manifest = yield* verifyManifest(binding, resolution.manifest);

    const handle: RlmCorpusHandle = {
      identity: {
        schemaId: "openagents.ai.rlm_corpus.v1",
        corpusRef: manifest.corpusRef,
        contentDigest: manifest.contentDigest,
        manifestDigest: manifest.manifestDigest,
      },
      manifest,
      read,
      scan: (request) => {
        const maximum = Math.min(request.maxEntries, binding.maxScanEntries);
        if (maximum === 0) return Stream.empty;
        return Stream.paginate(
          { nextOrdinal: request.fromOrdinal ?? 0, remaining: maximum },
          ({ nextOrdinal, remaining }) => {
            const pageSize = Math.min(remaining, binding.maxEntriesPerRead);
            return read(
              { start: nextOrdinal, endInclusive: nextOrdinal + pageSize - 1 },
              {
                maxEntries: pageSize,
                maxCharsPerEntry: binding.maxCharsPerEntry,
              },
            ).pipe(
              Effect.map((entries) => {
                const last = entries.at(-1);
                const nextRemaining = remaining - entries.length;
                const next =
                  last === undefined || nextRemaining === 0
                    ? Option.none<Readonly<{ nextOrdinal: number; remaining: number }>>()
                    : Option.some({
                        nextOrdinal: last.ordinal + 1,
                        remaining: nextRemaining,
                      });
                return [entries, next] as const;
              }),
            );
          },
        );
      },
      validateSourceAddress: (address) =>
        store.validateSourceAddress({ binding, address }).pipe(
          Effect.mapError(mapStoreError),
          Effect.flatMap((result) =>
            verifyObservation(binding, result.observation).pipe(
              Effect.flatMap(() =>
                result.validated.address.addressSchemaId === address.addressSchemaId &&
                result.validated.address.encodedAddress === address.encodedAddress
                  ? Effect.succeed(result.validated)
                  : Effect.fail(
                      corpusError(
                        "invalid_address",
                        "managed corpus store changed the source address",
                      ),
                    ),
              ),
            ),
          ),
        ),
      materializeAll: () =>
        Effect.fail(
          corpusError("unavailable", "managed corpus does not permit raw corpus materialization"),
        ),
    };
    return handle;
  });

  return RlmCorpusSource.of({ resolve });
};

/** Layer for the standard engine service with an explicit managed-store need. */
export const managedRlmCorpusSourceLayer = (
  binding: ManagedRlmCorpusBinding,
): Layer.Layer<RlmCorpusSource, never, ManagedRlmCorpusStore> =>
  Layer.effect(
    RlmCorpusSource,
    Effect.gen(function* () {
      const store = yield* ManagedRlmCorpusStore;
      return makeManagedRlmCorpusSource(binding, store);
    }),
  );
