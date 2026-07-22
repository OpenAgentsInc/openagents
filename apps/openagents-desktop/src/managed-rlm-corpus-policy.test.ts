import { Effect, Stream } from "effect";
import {
  buildInlineCorpusInput,
  type RlmCorpusEntry,
  type RlmOrdinalRange,
  type RlmReadLimits,
} from "@openagentsinc/rlm";
import { describe, expect, test } from "vite-plus/test";

import {
  MANAGED_RLM_CORPUS_BINDING_SCHEMA_ID,
  ManagedRlmCorpusBinding,
  ManagedRlmCorpusStoreError,
  managedRlmCorpusInput,
  makeManagedRlmCorpusSource,
  type ManagedRlmCorpusObservation,
  type ManagedRlmCorpusStoreShape,
} from "./managed-rlm-corpus-policy.ts";

const binding = ManagedRlmCorpusBinding.make({
  schemaId: MANAGED_RLM_CORPUS_BINDING_SCHEMA_ID,
  sourceRef: "managed-source.owner-a.corpus-a.g7",
  ownerRef: "owner.a",
  corpusRef: "corpus.a",
  policyRef: "policy.owner-workroom.v1",
  scopeRef: "scope.owner-a.workroom-a",
  sourceGeneration: 7,
  maxEntriesPerRead: 2,
  maxCharsPerEntry: 8,
  maxScanEntries: 3,
});

const inline = buildInlineCorpusInput({
  corpusRef: binding.corpusRef,
  scopeRef: binding.scopeRef,
  entries: Array.from({ length: 5 }, (_, index) => ({
    entryRef: `entry.${index}`,
    scopeRef: binding.scopeRef,
    sourceKind: "managed_workroom_evidence",
    sourceAddress: {
      addressSchemaId: "openagents.managed_rlm.workroom_cursor.v1",
      encodedAddress: `event.${index}`,
    },
    text: `evidence-${index}-private-tail`,
    visibility: "private" as const,
    redactionClass: "private_ref" as const,
  })),
});

const activeObservation: ManagedRlmCorpusObservation = {
  sourceRef: binding.sourceRef,
  ownerRef: binding.ownerRef,
  corpusRef: binding.corpusRef,
  policyRef: binding.policyRef,
  scopeRef: binding.scopeRef,
  sourceGeneration: binding.sourceGeneration,
  state: "active",
};

const fixtureStore = (
  input?: Readonly<{
    observation?: () => ManagedRlmCorpusObservation;
    onResolve?: () => void;
    onRead?: (range: RlmOrdinalRange, limits: RlmReadLimits) => void;
  }>,
): ManagedRlmCorpusStoreShape => ({
  resolve: () =>
    Effect.sync(() => {
      input?.onResolve?.();
      return {
        observation: input?.observation?.() ?? activeObservation,
        manifest: inline.manifest,
      };
    }),
  read: ({ range, limits }) =>
    Effect.sync(() => {
      input?.onRead?.(range, limits);
      const entries = inline.entries
        .filter((entry) => entry.ordinal >= range.start && entry.ordinal <= range.endInclusive)
        .slice(0, limits.maxEntries)
        .map(
          (entry): RlmCorpusEntry => ({
            ...entry,
            ...(entry.text === undefined
              ? {}
              : { text: entry.text.slice(0, limits.maxCharsPerEntry) }),
          }),
        );
      return {
        observation: input?.observation?.() ?? activeObservation,
        entries,
      };
    }),
  validateSourceAddress: ({ address }) =>
    Effect.gen(function* () {
      const entry = inline.entries.find(
        (candidate) =>
          candidate.sourceAddress.addressSchemaId === address.addressSchemaId &&
          candidate.sourceAddress.encodedAddress === address.encodedAddress,
      );
      if (entry === undefined) {
        return yield* Effect.fail(
          new ManagedRlmCorpusStoreError({
            reason: "not_found",
            detailSafe: "test address is not in fixture",
          }),
        );
      }
      return {
        observation: input?.observation?.() ?? activeObservation,
        validated: {
          address,
          entryRef: entry.entryRef,
          ordinal: entry.ordinal,
        },
      };
    }),
});

describe("managed RLM corpus policy", () => {
  test("admits only the exact host-bound source ref", async () => {
    let resolveCalls = 0;
    const source = makeManagedRlmCorpusSource(
      binding,
      fixtureStore({ onResolve: () => resolveCalls++ }),
    );

    const handle = await Effect.runPromise(source.resolve(managedRlmCorpusInput(binding)));
    expect(handle.identity.corpusRef).toBe(binding.corpusRef);
    expect(resolveCalls).toBe(1);

    const error = await Effect.runPromise(
      source
        .resolve({
          _tag: "Source",
          sourceRef: {
            addressSchemaId: "openagents.managed_rlm.corpus_source.v1",
            encodedAddress: "managed-source.owner-b.corpus-b.g1",
          },
        })
        .pipe(Effect.flip),
    );
    expect(error.reason).toBe("unavailable");
    expect(resolveCalls).toBe(1);

    const inlineError = await Effect.runPromise(source.resolve(inline).pipe(Effect.flip));
    expect(inlineError.reason).toBe("unavailable");
    expect(resolveCalls).toBe(1);
  });

  test("refuses a cross-owner observation without exposing the foreign ref", async () => {
    const source = makeManagedRlmCorpusSource(
      binding,
      fixtureStore({
        observation: () => ({ ...activeObservation, ownerRef: "owner.b" }),
      }),
    );

    const error = await Effect.runPromise(
      source.resolve(managedRlmCorpusInput(binding)).pipe(Effect.flip),
    );
    expect(error.reason).toBe("unavailable");
    expect(error.detailSafe).not.toContain("owner.b");
  });

  test("fences a stale generation before returning a handle", async () => {
    const source = makeManagedRlmCorpusSource(
      binding,
      fixtureStore({
        observation: () => ({ ...activeObservation, sourceGeneration: 8 }),
      }),
    );

    const error = await Effect.runPromise(
      source.resolve(managedRlmCorpusInput(binding)).pipe(Effect.flip),
    );
    expect(error.reason).toBe("changed");
  });

  test("checks revocation again for each read", async () => {
    let state: ManagedRlmCorpusObservation["state"] = "active";
    const source = makeManagedRlmCorpusSource(
      binding,
      fixtureStore({
        observation: () => ({ ...activeObservation, state }),
      }),
    );
    const handle = await Effect.runPromise(source.resolve(managedRlmCorpusInput(binding)));

    state = "revoked";
    const error = await Effect.runPromise(
      handle
        .read({ start: 0, endInclusive: 0 }, { maxEntries: 1, maxCharsPerEntry: 100 })
        .pipe(Effect.flip),
    );
    expect(error.reason).toBe("unavailable");
    expect(error.detailSafe).toContain("revoked");
  });

  test("clamps reads and scans to the admitted host policy", async () => {
    const observedReads: Array<
      Readonly<{
        range: RlmOrdinalRange;
        limits: RlmReadLimits;
      }>
    > = [];
    const source = makeManagedRlmCorpusSource(
      binding,
      fixtureStore({
        onRead: (range, limits) => observedReads.push({ range, limits }),
      }),
    );
    const handle = await Effect.runPromise(source.resolve(managedRlmCorpusInput(binding)));

    const direct = await Effect.runPromise(
      handle.read({ start: 0, endInclusive: 99 }, { maxEntries: 999, maxCharsPerEntry: 99_999 }),
    );
    expect(direct).toHaveLength(2);
    expect(direct.every((entry) => (entry.text?.length ?? 0) <= 8)).toBe(true);
    expect(observedReads[0]?.limits).toEqual({ maxEntries: 2, maxCharsPerEntry: 8 });

    observedReads.length = 0;
    const scanned = await Effect.runPromise(
      handle.scan({ fromOrdinal: 0, maxEntries: 99 }).pipe(Stream.runCollect),
    );
    expect(scanned.map((entry) => entry.entryRef)).toEqual(["entry.0", "entry.1", "entry.2"]);
    expect(observedReads.map(({ limits }) => limits.maxEntries)).toEqual([2, 1]);
  });

  test("does not provide a raw-corpus materialization path", async () => {
    const source = makeManagedRlmCorpusSource(binding, fixtureStore());
    const handle = await Effect.runPromise(source.resolve(managedRlmCorpusInput(binding)));

    const error = await Effect.runPromise(handle.materializeAll().pipe(Effect.flip));
    expect(error.reason).toBe("unavailable");
    expect(error.detailSafe).toContain("does not permit raw corpus materialization");
  });
});
