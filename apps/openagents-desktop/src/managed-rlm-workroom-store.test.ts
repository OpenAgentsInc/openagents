import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";
import { ProductSpecRunSchema } from "./product-spec-workroom-contract.ts";
import {
  MANAGED_RLM_CORPUS_BINDING_SCHEMA_ID,
  ManagedRlmCorpusBinding,
  managedRlmCorpusInput,
  makeManagedRlmCorpusSource,
} from "./managed-rlm-corpus-policy.ts";
import {
  MANAGED_RLM_WORKROOM_ADDRESS_SCHEMA_ID,
  MANAGED_RLM_WORKROOM_GRANT_SCHEMA_ID,
  ManagedRlmWorkroomGrant,
  makeManagedRlmWorkroomStore,
  type ManagedRlmWorkroomRunSource,
} from "./managed-rlm-workroom-store.ts";

const digest = (character: string): string => `sha256:${character.repeat(64)}`;
const observedAt = "2026-07-22T01:00:00.000Z";
const workContextRef = "scope.owner-a.workroom-a";

const spec = {
  specRef: "spec.workroom.a",
  relativePath: "docs/workroom-a.product-spec.md",
  revision: 3,
  digest: digest("a"),
} as const;

const run = ProductSpecRunSchema.make({
  runRef: "product.run.owner-a.1",
  spec,
  workContextRef,
  plan: {
    planRef: "product.plan.owner-a.1",
    spec,
    workContextRef,
    state: "accepted",
    packets: [
      {
        packetRef: "packet.research.1",
        title: "Collect exact evidence refs",
        criterionIds: ["CW-AC-01"],
        criterionRefs: ["The workroom retains exact evidence refs."],
        dependencyRefs: [],
        allocation: "child",
        state: "evidence_present",
        evidenceRefs: ["evidence.workroom.1"],
        evidenceReceipts: [
          {
            receiptRef: "receipt.evidence.workroom.1",
            evidenceRef: "evidence.workroom.1",
            kind: "receipt",
            producerRef: "agent.child.1",
            spec,
            criterionIds: ["CW-AC-01"],
            producedAt: observedAt,
          },
        ],
        evidenceProducerRef: "agent.child.1",
        verifierRefs: [],
        verificationReceipts: [],
        ownerDisposition: null,
        activeLease: null,
      },
      {
        packetRef: "packet.verify.1",
        title: "Verify the retained evidence",
        criterionIds: ["CW-AC-02"],
        criterionRefs: ["An independent verifier checks the evidence."],
        dependencyRefs: ["packet.research.1"],
        allocation: "root",
        state: "verified",
        evidenceRefs: ["evidence.workroom.2"],
        evidenceReceipts: [
          {
            receiptRef: "receipt.evidence.workroom.2",
            evidenceRef: "evidence.workroom.2",
            kind: "test_run",
            producerRef: "agent.root.1",
            spec,
            criterionIds: ["CW-AC-02"],
            producedAt: observedAt,
          },
        ],
        evidenceProducerRef: "agent.root.1",
        verifierRefs: ["reviewer.independent.1"],
        verificationReceipts: [
          {
            receiptRef: "receipt.verification.workroom.2",
            evidenceReceiptRefs: ["receipt.evidence.workroom.2"],
            outputRef: "output.verification.workroom.2",
            verifierRef: "reviewer.independent.1",
            spec,
            criterionIds: ["CW-AC-02"],
            verdict: "passed",
            verifiedAt: observedAt,
          },
        ],
        ownerDisposition: null,
        activeLease: null,
      },
    ],
    deferredCriterionIds: [],
    proposedAt: observedAt,
    acceptedAt: observedAt,
  },
  createdAt: observedAt,
  updatedAt: observedAt,
});

const binding = ManagedRlmCorpusBinding.make({
  schemaId: MANAGED_RLM_CORPUS_BINDING_SCHEMA_ID,
  sourceRef: "managed-source.owner-a.workroom-a.g7",
  ownerRef: "owner.a",
  corpusRef: "corpus.workroom.a",
  policyRef: "policy.owner-workroom.v1",
  scopeRef: workContextRef,
  sourceGeneration: 7,
  maxEntriesPerRead: 1,
  maxCharsPerEntry: 80,
  maxScanEntries: 2,
});

const activeGrant = ManagedRlmWorkroomGrant.make({
  schemaId: MANAGED_RLM_WORKROOM_GRANT_SCHEMA_ID,
  sourceRef: binding.sourceRef,
  ownerRef: binding.ownerRef,
  corpusRef: binding.corpusRef,
  policyRef: binding.policyRef,
  scopeRef: binding.scopeRef,
  sourceGeneration: binding.sourceGeneration,
  runRef: run.runRef,
  workContextRef: run.workContextRef,
  runUpdatedAt: run.updatedAt,
  specDigest: run.spec.digest,
  specRevision: run.spec.revision,
  state: "active",
});

const workroom = (value = run): ManagedRlmWorkroomRunSource => ({
  run: (runRef) =>
    runRef === value.runRef
      ? { ok: true, value }
      : { ok: false, reason: "not_found", message: "The run does not exist." },
});

// eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests -- Vite Plus does not expose an Effect-aware test API; this helper is the suite runtime boundary.
const runEffect = <A, E>(effect: Effect.Effect<A, E>): Promise<A> => Effect.runPromise(effect);

const failureOf = <A, E>(effect: Effect.Effect<A, E>): Promise<E | null> =>
  runEffect(
    effect.pipe(
      Effect.match({
        onFailure: (error) => error,
        onSuccess: () => null,
      }),
    ),
  );

describe("managed RLM ProductSpec workroom store", () => {
  test("resolves the current workroom, clamps reads, and validates exact citation addresses", async () => {
    const store = makeManagedRlmWorkroomStore({
      workroom: workroom(),
      observeGrant: () => Effect.succeed(activeGrant),
    });
    const source = makeManagedRlmCorpusSource(binding, store);
    const handle = await runEffect(source.resolve(managedRlmCorpusInput(binding)));

    expect(handle.manifest.coverage.entryCount).toBe(2);
    expect(handle.manifest.coverage.exclusions).toEqual([
      { reason: "packet_without_evidence", count: 0 },
      { reason: "packet_not_verified", count: 1 },
    ]);
    const entries = await runEffect(
      handle.read({ start: 0, endInclusive: 99 }, { maxEntries: 99, maxCharsPerEntry: 10_000 }),
    );
    expect(entries).toHaveLength(1);
    expect(entries[0]?.text?.length).toBeLessThanOrEqual(binding.maxCharsPerEntry);
    expect(entries[0]?.sourceAddress.addressSchemaId).toBe(MANAGED_RLM_WORKROOM_ADDRESS_SCHEMA_ID);
    const validated = await runEffect(handle.validateSourceAddress(entries[0]!.sourceAddress));
    expect(validated.entryRef).toBe(entries[0]?.entryRef);
    expect("materializeAll" in store).toBe(false);
  });

  test("refuses a cross-owner binding without exposing the foreign owner ref", async () => {
    const store = makeManagedRlmWorkroomStore({
      workroom: workroom(),
      observeGrant: () => Effect.succeed(activeGrant),
    });
    const error = await failureOf(
      store.resolve(
        ManagedRlmCorpusBinding.make({
          ...binding,
          ownerRef: "owner.b",
        }),
      ),
    );

    expect(error?.reason).toBe("authority_not_granted");
    expect(error?.detailSafe).not.toContain("owner.a");
  });

  test("refuses revoked and stale-generation grants on the next operation", async () => {
    let grant = activeGrant;
    const store = makeManagedRlmWorkroomStore({
      workroom: workroom(),
      observeGrant: () => Effect.succeed(grant),
    });

    expect((await runEffect(store.resolve(binding))).manifest.corpusRef).toBe(binding.corpusRef);
    grant = ManagedRlmWorkroomGrant.make({ ...activeGrant, state: "revoked" });
    expect((await failureOf(store.resolve(binding)))?.reason).toBe("revoked");
    grant = ManagedRlmWorkroomGrant.make({
      ...activeGrant,
      state: "active",
      sourceGeneration: activeGrant.sourceGeneration + 1,
    });
    expect((await failureOf(store.resolve(binding)))?.reason).toBe("stale_generation");
  });

  test("detects authority changes during a read and stale workroom bytes", async () => {
    let observations = 0;
    const changingStore = makeManagedRlmWorkroomStore({
      workroom: workroom(),
      observeGrant: () =>
        Effect.succeed(
          observations++ === 0
            ? activeGrant
            : ManagedRlmWorkroomGrant.make({ ...activeGrant, state: "revoked" }),
        ),
    });
    expect((await failureOf(changingStore.resolve(binding)))?.reason).toBe("revoked");

    const staleRun = ProductSpecRunSchema.make({
      ...run,
      updatedAt: "2026-07-22T01:01:00.000Z",
    });
    const staleStore = makeManagedRlmWorkroomStore({
      workroom: workroom(staleRun),
      observeGrant: () => Effect.succeed(activeGrant),
    });
    expect((await failureOf(staleStore.resolve(binding)))?.reason).toBe("stale_generation");
  });

  test("rejects malformed and foreign workroom cursor addresses", async () => {
    const store = makeManagedRlmWorkroomStore({
      workroom: workroom(),
      observeGrant: () => Effect.succeed(activeGrant),
    });
    const malformed = await failureOf(
      store.validateSourceAddress({
        binding,
        address: {
          addressSchemaId: MANAGED_RLM_WORKROOM_ADDRESS_SCHEMA_ID,
          encodedAddress: "not-json",
        },
      }),
    );
    const foreign = await failureOf(
      store.validateSourceAddress({
        binding,
        address: {
          addressSchemaId: MANAGED_RLM_WORKROOM_ADDRESS_SCHEMA_ID,
          encodedAddress: JSON.stringify([run.runRef, "packet.foreign"]),
        },
      }),
    );

    expect(malformed?.reason).toBe("not_found");
    expect(foreign?.reason).toBe("not_found");
  });
});
