/**
 * RLM-08 (#9144) — ProductSpec workroom source-of-record adapter.
 *
 * The adapter projects current owner-scoped run and evidence facts on demand.
 * It does not persist a second corpus or retain raw workroom text.
 */

import { Effect, Schema } from "effect";
import {
  computeContentDigest,
  computeManifestDigest,
  type RlmCorpusEntry,
  type RlmCorpusManifest,
  type RlmOrdinalRange,
  type RlmReadLimits,
  type RlmSourceAddress,
} from "@openagentsinc/rlm";

import {
  ManagedRlmCorpusBinding,
  ManagedRlmCorpusStoreError,
  type ManagedRlmCorpusObservation,
  type ManagedRlmCorpusStoreShape,
} from "./managed-rlm-corpus-policy.ts";
import type { ProductSpecRun } from "./product-spec-workroom-contract.ts";
import type { ProductSpecWorkroom } from "./product-spec-workroom.ts";

export const MANAGED_RLM_WORKROOM_GRANT_SCHEMA_ID =
  "openagents.managed_rlm.workroom_grant.v1" as const;
export const MANAGED_RLM_WORKROOM_ADDRESS_SCHEMA_ID =
  "openagents.managed_rlm.workroom_cursor.v1" as const;
export const MANAGED_RLM_WORKROOM_MAX_ENTRIES = 10_000 as const;

const WorkroomTimestamp = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
);
const ProductSpecDigest = Schema.String.check(Schema.isPattern(/^sha256:[a-f0-9]{64}$/));

/** Host authority for one exact ProductSpec workroom run generation. */
export const ManagedRlmWorkroomGrant = Schema.Struct({
  schemaId: Schema.Literal(MANAGED_RLM_WORKROOM_GRANT_SCHEMA_ID),
  sourceRef: ManagedRlmCorpusBinding.fields.sourceRef,
  ownerRef: ManagedRlmCorpusBinding.fields.ownerRef,
  corpusRef: ManagedRlmCorpusBinding.fields.corpusRef,
  policyRef: ManagedRlmCorpusBinding.fields.policyRef,
  scopeRef: ManagedRlmCorpusBinding.fields.scopeRef,
  sourceGeneration: ManagedRlmCorpusBinding.fields.sourceGeneration,
  runRef: ManagedRlmCorpusBinding.fields.sourceRef,
  workContextRef: ManagedRlmCorpusBinding.fields.scopeRef,
  runUpdatedAt: WorkroomTimestamp,
  specDigest: ProductSpecDigest,
  specRevision: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  state: Schema.Literals(["active", "revoked"]),
});

export interface ManagedRlmWorkroomGrant extends Schema.Schema.Type<
  typeof ManagedRlmWorkroomGrant
> {}

const decodeManagedRlmWorkroomGrant = Schema.decodeUnknownEffect(ManagedRlmWorkroomGrant);

export interface ManagedRlmWorkroomRunSource {
  readonly run: ProductSpecWorkroom["run"];
}

export interface ManagedRlmWorkroomStoreOptions {
  readonly workroom: ManagedRlmWorkroomRunSource;
  /** Read the current host grant. The adapter decodes every observation. */
  readonly observeGrant: () => Effect.Effect<unknown, ManagedRlmCorpusStoreError>;
}

interface CurrentWorkroomCorpus {
  readonly observation: ManagedRlmCorpusObservation;
  readonly manifest: RlmCorpusManifest;
  readonly entries: ReadonlyArray<RlmCorpusEntry>;
}

const storeError = (
  reason: ManagedRlmCorpusStoreError["reason"],
  detailSafe: string,
): ManagedRlmCorpusStoreError => new ManagedRlmCorpusStoreError({ reason, detailSafe });

const grantMatchesBinding = (
  grant: ManagedRlmWorkroomGrant,
  binding: ManagedRlmCorpusBinding,
): boolean =>
  grant.sourceRef === binding.sourceRef &&
  grant.ownerRef === binding.ownerRef &&
  grant.corpusRef === binding.corpusRef &&
  grant.policyRef === binding.policyRef &&
  grant.scopeRef === binding.scopeRef;

const sameGrant = (left: ManagedRlmWorkroomGrant, right: ManagedRlmWorkroomGrant): boolean =>
  left.sourceRef === right.sourceRef &&
  left.ownerRef === right.ownerRef &&
  left.corpusRef === right.corpusRef &&
  left.policyRef === right.policyRef &&
  left.scopeRef === right.scopeRef &&
  left.sourceGeneration === right.sourceGeneration &&
  left.runRef === right.runRef &&
  left.workContextRef === right.workContextRef &&
  left.runUpdatedAt === right.runUpdatedAt &&
  left.specDigest === right.specDigest &&
  left.specRevision === right.specRevision &&
  left.state === right.state;

const observationOf = (grant: ManagedRlmWorkroomGrant): ManagedRlmCorpusObservation => ({
  sourceRef: grant.sourceRef,
  ownerRef: grant.ownerRef,
  corpusRef: grant.corpusRef,
  policyRef: grant.policyRef,
  scopeRef: grant.scopeRef,
  sourceGeneration: grant.sourceGeneration,
  state: grant.state,
});

const encodeAddress = (runRef: string, packetRef: string): RlmSourceAddress => ({
  addressSchemaId: MANAGED_RLM_WORKROOM_ADDRESS_SCHEMA_ID,
  encodedAddress: JSON.stringify([runRef, packetRef]),
});

const decodeAddress = (
  address: RlmSourceAddress,
): Readonly<{ runRef: string; packetRef: string }> | null => {
  if (address.addressSchemaId !== MANAGED_RLM_WORKROOM_ADDRESS_SCHEMA_ID) return null;
  try {
    const value: unknown = JSON.parse(address.encodedAddress);
    if (
      !Array.isArray(value) ||
      value.length !== 2 ||
      typeof value[0] !== "string" ||
      typeof value[1] !== "string"
    )
      return null;
    return { runRef: value[0], packetRef: value[1] };
  } catch {
    return null;
  }
};

const packetText = (packet: ProductSpecRun["plan"]["packets"][number]): string =>
  JSON.stringify({
    packetRef: packet.packetRef,
    title: packet.title,
    state: packet.state,
    criterionIds: packet.criterionIds,
    criterionRefs: packet.criterionRefs,
    dependencyRefs: packet.dependencyRefs,
    evidenceRefs: packet.evidenceRefs,
    evidenceReceipts: packet.evidenceReceipts,
    verifierRefs: packet.verifierRefs,
    verificationReceipts: packet.verificationReceipts,
    ownerDisposition: packet.ownerDisposition,
    blockedReason: packet.blockedReason,
  });

const entriesOf = (run: ProductSpecRun, scopeRef: string): ReadonlyArray<RlmCorpusEntry> =>
  run.plan.packets.map((packet, ordinal) => ({
    ordinal,
    entryRef: `${run.runRef}:${packet.packetRef}`,
    scopeRef,
    sourceKind: "managed_workroom_evidence",
    sourcePlane: "evidence_pack" as const,
    sourceAddress: encodeAddress(run.runRef, packet.packetRef),
    text: packetText(packet),
    visibility: "private",
    redactionClass: "private_ref",
    observedAt: run.updatedAt,
  }));

const encodedBytesOf = (entries: ReadonlyArray<RlmCorpusEntry>): number =>
  new TextEncoder().encode(entries.map((entry) => JSON.stringify(entry)).join("\n")).length;

const manifestOf = (
  binding: ManagedRlmCorpusBinding,
  run: ProductSpecRun,
  entries: ReadonlyArray<RlmCorpusEntry>,
): RlmCorpusManifest => {
  const ordering = {
    rule: "source_declared" as const,
    note: "ProductSpec work packet order from the current owner-scoped run.",
  };
  const coverage = {
    note: "Current owner-scoped ProductSpec work packet and evidence facts.",
    entryCount: entries.length,
    encodedBytes: encodedBytesOf(entries),
    exclusions: [
      {
        reason: "packet_without_evidence",
        count: run.plan.packets.filter(({ evidenceRefs }) => evidenceRefs.length === 0).length,
      },
      {
        reason: "packet_not_verified",
        count: run.plan.packets.filter(({ state }) => state !== "verified").length,
      },
    ],
  };
  const contentDigest = computeContentDigest({
    scopeRef: binding.scopeRef,
    ordering,
    entries,
  });
  const manifestPolicy = {
    includeVisibilities: ["private" as const],
    includeRedactionClasses: ["private_ref" as const],
  };
  const manifestDigest = computeManifestDigest({
    contentDigest,
    coverage,
    policy: manifestPolicy,
    scopeRef: binding.scopeRef,
    ordering,
  });
  return {
    schemaId: "openagents.ai.rlm_corpus.v2",
    corpusRef: binding.corpusRef,
    contentDigest,
    manifestDigest,
    ordering,
    coverage,
    policy: manifestPolicy,
    scopeRef: binding.scopeRef,
    builtAt: run.updatedAt,
  };
};

/** Build a managed RLM store over the current ProductSpec workroom run. */
export const makeManagedRlmWorkroomStore = (
  options: ManagedRlmWorkroomStoreOptions,
): ManagedRlmCorpusStoreShape => {
  const observe = Effect.fn("ManagedRlmWorkroomStore.observe")(function* (
    binding: ManagedRlmCorpusBinding,
  ) {
    const grant = yield* options.observeGrant().pipe(
      Effect.flatMap(decodeManagedRlmWorkroomGrant),
      Effect.mapError((error) =>
        error instanceof ManagedRlmCorpusStoreError
          ? error
          : storeError("invalid_response", "managed workroom grant was invalid"),
      ),
    );
    if (!grantMatchesBinding(grant, binding)) {
      return yield* Effect.fail(
        storeError("authority_not_granted", "managed workroom grant did not match the binding"),
      );
    }
    if (grant.sourceGeneration !== binding.sourceGeneration) {
      return yield* Effect.fail(
        storeError("stale_generation", "managed workroom source generation changed"),
      );
    }
    if (grant.state !== "active") {
      return yield* Effect.fail(storeError("revoked", "managed workroom grant was revoked"));
    }
    return grant;
  });

  const readRun = Effect.fn("ManagedRlmWorkroomStore.readRun")(function* (
    grant: ManagedRlmWorkroomGrant,
  ) {
    const result = yield* Effect.try({
      try: () => options.workroom.run(grant.runRef),
      catch: () => storeError("unavailable", "managed workroom run could not be read"),
    });
    if (!result.ok) {
      return yield* Effect.fail(
        storeError(
          result.reason === "not_found" ? "not_found" : "unavailable",
          "managed workroom run was unavailable",
        ),
      );
    }
    const run = result.value;
    if (
      run.runRef !== grant.runRef ||
      run.workContextRef !== grant.workContextRef ||
      run.updatedAt !== grant.runUpdatedAt ||
      run.spec.digest !== grant.specDigest ||
      run.spec.revision !== grant.specRevision
    ) {
      return yield* Effect.fail(
        storeError("stale_generation", "managed workroom run changed after grant admission"),
      );
    }
    if (run.plan.packets.length > MANAGED_RLM_WORKROOM_MAX_ENTRIES) {
      return yield* Effect.fail(
        storeError("invalid_response", "managed workroom run exceeded the entry ceiling"),
      );
    }
    return run;
  });

  const current = Effect.fn("ManagedRlmWorkroomStore.current")(function* (
    binding: ManagedRlmCorpusBinding,
  ) {
    const before = yield* observe(binding);
    const run = yield* readRun(before);
    const after = yield* observe(binding);
    if (!sameGrant(before, after)) {
      return yield* Effect.fail(
        storeError("stale_generation", "managed workroom grant changed during the read"),
      );
    }
    const entries = entriesOf(run, binding.scopeRef);
    return {
      observation: observationOf(after),
      manifest: manifestOf(binding, run, entries),
      entries,
    } satisfies CurrentWorkroomCorpus;
  });

  const resolve = Effect.fn("ManagedRlmWorkroomStore.resolve")(function* (
    binding: ManagedRlmCorpusBinding,
  ) {
    const corpus = yield* current(binding);
    return { observation: corpus.observation, manifest: corpus.manifest };
  });

  const read = Effect.fn("ManagedRlmWorkroomStore.read")(function* (input: {
    readonly binding: ManagedRlmCorpusBinding;
    readonly range: RlmOrdinalRange;
    readonly limits: RlmReadLimits;
  }) {
    if (
      !Number.isInteger(input.range.start) ||
      !Number.isInteger(input.range.endInclusive) ||
      input.range.start < 0 ||
      input.range.start > input.range.endInclusive ||
      !Number.isInteger(input.limits.maxEntries) ||
      !Number.isInteger(input.limits.maxCharsPerEntry) ||
      input.limits.maxEntries < 0 ||
      input.limits.maxCharsPerEntry < 0
    ) {
      return yield* Effect.fail(
        storeError("invalid_response", "managed workroom read bounds were invalid"),
      );
    }
    const corpus = yield* current(input.binding);
    const entries = corpus.entries
      .filter(({ ordinal }) => ordinal >= input.range.start && ordinal <= input.range.endInclusive)
      .slice(0, input.limits.maxEntries)
      .map((entry) =>
        entry.text === undefined
          ? entry
          : Object.assign({}, entry, {
              text: entry.text.slice(0, input.limits.maxCharsPerEntry),
            }),
      );
    return { observation: corpus.observation, entries };
  });

  const validateSourceAddress = Effect.fn("ManagedRlmWorkroomStore.validateSourceAddress")(
    function* (input: {
      readonly binding: ManagedRlmCorpusBinding;
      readonly address: RlmSourceAddress;
    }) {
      const decoded = decodeAddress(input.address);
      if (decoded === null) {
        return yield* Effect.fail(
          storeError("not_found", "managed workroom source address was not found"),
        );
      }
      const corpus = yield* current(input.binding);
      const entry = corpus.entries.find(
        ({ sourceAddress }) =>
          sourceAddress.addressSchemaId === input.address.addressSchemaId &&
          sourceAddress.encodedAddress === input.address.encodedAddress,
      );
      if (entry === undefined) {
        return yield* Effect.fail(
          storeError("not_found", "managed workroom source address was not found"),
        );
      }
      return {
        observation: corpus.observation,
        validated: {
          address: entry.sourceAddress,
          sourcePlane: "evidence_pack" as const,
          entryRef: entry.entryRef,
          ordinal: entry.ordinal,
          origin: {
            sourcePlane: "evidence_pack" as const,
            sourceKind: entry.sourceKind,
            sourceAddress: entry.sourceAddress,
            corpusRef: corpus.manifest.corpusRef,
            contentDigest: corpus.manifest.contentDigest,
            entryRef: entry.entryRef,
          },
        },
      };
    },
  );

  return { resolve, read, validateSourceAddress };
};
