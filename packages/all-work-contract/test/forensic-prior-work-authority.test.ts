import { Effect, Layer, Ref } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  emptyForensicPriorWorkState,
  fileForensicPriorWorkStateStoreLayer,
  ForensicPriorWorkAuthority,
  ForensicPriorWorkAuthorityLive,
  ForensicPriorWorkAuthorityError,
  ForensicPriorWorkStateStore,
  initializeFileForensicPriorWorkState,
} from "../src/index.ts";

const allDispositions = [
  "confirmed",
  "dismissed",
  "rejected",
  "inconclusive",
  "expired",
  "superseded",
  "corrected",
  "duplicate",
  "retained",
] as const;

const submission = (overrides: Record<string, unknown> = {}) => ({
  workRef: "work:forensic:coldcard:1",
  repositoryRef: "repository:coldcard:firmware",
  revision: "bcc2c382a324690a2fcf972c0bac3b79bf923f7b",
  path: "shared/hmac.c",
  symbol: "rng_get_bytes",
  startLine: 10,
  endLine: 24,
  sourceWindowDigest: `sha256:${"a".repeat(64)}`,
  mechanismClass: "mechanism:entropy:unseeded-fallback",
  causalMechanism: "An unseeded fallback reaches wallet secret generation",
  affectedBehavior: "Wallet seeds can be generated from insufficient entropy",
  securityBoundary: "firmware entropy provider to seed generator",
  causalChainSummary: "provider guard -> unseeded fallback -> wallet seed",
  promptRefs: ["prompt:entropy:v1"],
  sourceRefs: ["source:coldcard:hmac"],
  evidenceRefs: ["evidence:coldcard:trace"],
  audience: {
    visibility: "organization",
    organizationRef: "organization:openagents",
    principalRef: null,
  },
  disposition: "confirmed",
  actorRef: "principal:reviewer:1",
  submittedAt: "2026-08-03T20:00:00Z",
  idempotencyRef: "idempotency:forensic:submit:1",
  ...overrides,
});

const query = (overrides: Record<string, unknown> = {}) => ({
  queryRef: "query:forensic:1",
  principalRef: "principal:reviewer:1",
  organizationRefs: ["organization:openagents"],
  includePublic: true,
  mode: "semantic",
  exactRef: null,
  text: "unseeded entropy fallback wallet seed",
  dispositionFilter: [...allDispositions],
  cursor: null,
  limit: 25,
  ...overrides,
});

const layer = () =>
  ForensicPriorWorkAuthorityLive.pipe(
    Layer.provide(
      Layer.effect(
        ForensicPriorWorkStateStore,
        Effect.gen(function* () {
          const ref = yield* Ref.make(emptyForensicPriorWorkState());
          return ForensicPriorWorkStateStore.of({
            load: Ref.get(ref),
            save: (expectedRevision, next) =>
              Ref.modify(ref, (current) =>
                current.revision === expectedRevision
                  ? ([true, next] as const)
                  : ([false, current] as const),
              ).pipe(
                Effect.flatMap((saved) =>
                  saved
                    ? Effect.void
                    : Effect.fail(
                        new ForensicPriorWorkAuthorityError({
                          reason: "revision_conflict",
                          detail: "stale test writer",
                        }),
                      ),
                ),
              ),
          });
        }),
      ),
    ),
  );

describe("All Work forensic prior-work authority", () => {
  it("merges one causal defect across focal files while preserving both occurrences", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* ForensicPriorWorkAuthority;
        const first = yield* authority.submit(submission());
        const second = yield* authority.submit(
          submission({
            workRef: "work:forensic:coldcard:2",
            path: "unix/random.c",
            symbol: "hardware_rng",
            startLine: 80,
            endLine: 96,
            sourceWindowDigest: `sha256:${"b".repeat(64)}`,
            submittedAt: "2026-08-03T20:00:01Z",
            idempotencyRef: "idempotency:forensic:submit:2",
          }),
        );
        const found = yield* authority.query(query());
        return { first, second, found };
      }).pipe(Effect.provide(layer())),
    );
    expect(result.second.rootCause.rootCauseRef).toBe(result.first.rootCause.rootCauseRef);
    expect(result.second.occurrences).toHaveLength(2);
    expect(result.second.workRefs).toEqual([
      "work:forensic:coldcard:1",
      "work:forensic:coldcard:2",
    ]);
    expect(result.second.dispositions.at(-1)?.disposition).toBe("duplicate");
    expect(result.second.relations.at(-1)?.targetWorkRef).toBe("work:forensic:coldcard:1");
    expect(result.found.matches).toHaveLength(1);
    expect(result.found.matches[0]?.record.occurrences).toHaveLength(2);
  });

  it("keeps source-window collisions separate when causal mechanisms differ", async () => {
    const records = await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* ForensicPriorWorkAuthority;
        const first = yield* authority.submit(submission());
        const second = yield* authority.submit(
          submission({
            workRef: "work:forensic:coldcard:collision",
            causalMechanism: "A bounds error truncates the entropy buffer",
            mechanismClass: "mechanism:entropy:buffer-truncation",
            affectedBehavior: "Entropy bytes are truncated before seed derivation",
            idempotencyRef: "idempotency:forensic:collision",
          }),
        );
        return [first, second];
      }).pipe(Effect.provide(layer())),
    );
    expect(records[0]?.occurrences[0]?.sourceWindowDigest).toBe(
      records[1]?.occurrences[0]?.sourceWindowDigest,
    );
    expect(records[0]?.rootCause.rootCauseRef).not.toBe(records[1]?.rootCause.rootCauseRef);
  });

  it("returns retained non-confirmed dispositions and exact stable Work lookup", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* ForensicPriorWorkAuthority;
        const record = yield* authority.submit(
          submission({ disposition: "inconclusive", idempotencyRef: "idempotency:inconclusive" }),
        );
        yield* authority.dispose({
          workRef: record.primaryWorkRef,
          disposition: "corrected",
          reason: "Later evidence corrected the retained interpretation",
          actorRef: "principal:reviewer:1",
          occurredAt: "2026-08-03T20:00:02Z",
          idempotencyRef: "idempotency:corrected",
        });
        const exact = yield* authority.query(
          query({
            queryRef: "query:forensic:exact",
            mode: "exact",
            exactRef: record.primaryWorkRef,
            text: null,
            dispositionFilter: ["corrected"],
          }),
        );
        return exact;
      }).pipe(Effect.provide(layer())),
    );
    expect(result.matches).toHaveLength(1);
    expect(result.matches[0]?.matchedWorkRefs).toEqual(["work:forensic:coldcard:1"]);
    expect(result.matches[0]?.record.dispositions.map((event) => event.disposition)).toEqual([
      "inconclusive",
      "corrected",
    ]);
    expect(result.receipt.authorizedPopulationComplete).toBe(true);
  });

  it("does not reveal private or organization Work to an unauthorized query", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* ForensicPriorWorkAuthority;
        yield* authority.submit(submission());
        return yield* authority.query(
          query({
            principalRef: "principal:outsider",
            organizationRefs: [],
            includePublic: false,
          }),
        );
      }).pipe(Effect.provide(layer())),
    );
    expect(result.matches).toEqual([]);
    expect(result.receipt.searchedAuthorizedCount).toBe(0);
    expect(result.receipt.returnedCount).toBe(0);
  });

  it("replays submission idempotently without changing first-identification time", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* ForensicPriorWorkAuthority;
        const first = yield* authority.submit(submission());
        const replay = yield* authority.submit(submission());
        return { first, replay };
      }).pipe(Effect.provide(layer())),
    );
    expect(result.replay).toEqual(result.first);
    expect(result.replay.firstIdentifiedAt).toBe("2026-08-03T20:00:00Z");
  });

  it("converges concurrent same-cause submissions without losing either occurrence", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const authority = yield* ForensicPriorWorkAuthority;
        yield* Effect.all(
          [
            authority.submit(submission()),
            authority.submit(
              submission({
                workRef: "work:forensic:coldcard:concurrent",
                path: "stm32/rng.c",
                symbol: "rng_sample",
                sourceWindowDigest: `sha256:${"c".repeat(64)}`,
                submittedAt: "2026-08-03T19:59:59Z",
                idempotencyRef: "idempotency:forensic:concurrent",
              }),
            ),
          ],
          { concurrency: "unbounded" },
        );
        return yield* authority.query(query());
      }).pipe(Effect.provide(layer())),
    );
    expect(result.matches).toHaveLength(1);
    expect(result.matches[0]?.record.occurrences).toHaveLength(2);
    expect(result.matches[0]?.record.workRefs).toContain("work:forensic:coldcard:concurrent");
    expect(result.matches[0]?.record.firstIdentifiedAt).toBe("2026-08-03T19:59:59Z");
  });

  it("restores exact lookup and append-only relations from the durable All Work store", async () => {
    const root = mkdtempSync(resolve(tmpdir(), "forensic-prior-work-"));
    try {
      const state = emptyForensicPriorWorkState();
      await Effect.runPromise(initializeFileForensicPriorWorkState(root, state));
      const fileLayer = () =>
        ForensicPriorWorkAuthorityLive.pipe(
          Layer.provide(fileForensicPriorWorkStateStoreLayer(root)),
        );
      await Effect.runPromise(
        Effect.gen(function* () {
          const authority = yield* ForensicPriorWorkAuthority;
          yield* authority.submit(submission());
          yield* authority.submit(
            submission({
              workRef: "work:forensic:coldcard:moved",
              path: "src/random.c",
              submittedAt: "2026-08-03T20:01:00Z",
              idempotencyRef: "idempotency:forensic:moved",
            }),
          );
          yield* authority.relate({
            fromWorkRef: "work:forensic:coldcard:moved",
            targetWorkRef: "work:forensic:coldcard:1",
            kind: "supersedes",
            confidence: "probable",
            reason: "Repository rename fixture preserves uncertainty",
            actorRef: "principal:reviewer:1",
            occurredAt: "2026-08-03T20:01:01Z",
            idempotencyRef: "idempotency:forensic:rename-relation",
          });
        }).pipe(Effect.provide(fileLayer())),
      );
      const restored = await Effect.runPromise(
        Effect.gen(function* () {
          const authority = yield* ForensicPriorWorkAuthority;
          return yield* authority.query(
            query({
              queryRef: "query:forensic:restart",
              mode: "exact",
              exactRef: "work:forensic:coldcard:moved",
              text: null,
            }),
          );
        }).pipe(Effect.provide(fileLayer())),
      );
      expect(restored.matches).toHaveLength(1);
      expect(restored.matches[0]?.record.relations.map((relation) => relation.kind)).toEqual([
        "duplicate",
        "supersedes",
      ]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("serializes concurrent writers that open independent durable store layers", async () => {
    const root = mkdtempSync(resolve(tmpdir(), "forensic-prior-work-concurrent-"));
    try {
      await Effect.runPromise(
        initializeFileForensicPriorWorkState(root, emptyForensicPriorWorkState()),
      );
      const submitWithFreshLayer = (input: unknown) =>
        Effect.gen(function* () {
          const authority = yield* ForensicPriorWorkAuthority;
          return yield* authority.submit(input);
        }).pipe(
          Effect.provide(
            ForensicPriorWorkAuthorityLive.pipe(
              Layer.provide(fileForensicPriorWorkStateStoreLayer(root)),
            ),
          ),
        );
      await Effect.runPromise(
        Effect.all(
          [
            submitWithFreshLayer(submission()),
            submitWithFreshLayer(
              submission({
                workRef: "work:forensic:coldcard:file-concurrent",
                path: "stm32/random.c",
                sourceWindowDigest: `sha256:${"d".repeat(64)}`,
                idempotencyRef: "idempotency:forensic:file-concurrent",
              }),
            ),
          ],
          { concurrency: "unbounded" },
        ),
      );
      const restored = await Effect.runPromise(
        Effect.gen(function* () {
          const authority = yield* ForensicPriorWorkAuthority;
          return yield* authority.query(query());
        }).pipe(
          Effect.provide(
            ForensicPriorWorkAuthorityLive.pipe(
              Layer.provide(fileForensicPriorWorkStateStoreLayer(root)),
            ),
          ),
        ),
      );
      expect(restored.matches[0]?.record.workRefs).toHaveLength(2);
      expect(restored.matches[0]?.record.occurrences).toHaveLength(2);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
