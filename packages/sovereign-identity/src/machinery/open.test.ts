import { Effect, Layer, Ref, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { IdentityRef } from "../contract/index.ts";
import type { CandidateDiagnostic } from "./discovery.ts";
import { inMemoryCandidateSource } from "./discovery.ts";
import { CreateRootError, IdentityRootCreator, createIdentityRoot, openIdentity } from "./open.ts";

const identityRef = S.decodeUnknownSync(IdentityRef)("test-ref");

const diagnostic = (over: Partial<CandidateDiagnostic>): CandidateDiagnostic => ({
  sourceLabel: "candidate",
  fsType: "regular_file",
  present: true,
  admissible: true,
  permissionMode: "600",
  sizeClass: "small",
  modifiedAtIso: "2026-07-20T00:00:00.000Z",
  blocker: null,
  ...over,
});

/**
 * A counting creator layer. It proves `openIdentity` never invokes creation, and
 * that `createIdentityRoot` does. It writes no real secret.
 */
const countingCreatorLayer = (counter: Ref.Ref<number>): Layer.Layer<IdentityRootCreator> =>
  Layer.succeed(
    IdentityRootCreator,
    IdentityRootCreator.of({
      create: (intent) =>
        Effect.gen(function* () {
          yield* Ref.update(counter, (value) => value + 1);
          if (intent.acknowledgeCreatesNewRoot !== true) {
            return yield* new CreateRootError({ reason: "create_failed" });
          }
          return { identityRef };
        }),
    }),
  );

describe("fail-closed openIdentity", () => {
  test("no candidate stops terminal at NoCandidateFound and never creates", async () => {
    const counter = await Effect.runPromise(Ref.make(0));
    const outcome = await Effect.runPromise(
      openIdentity(inMemoryCandidateSource([])).pipe(Effect.provide(countingCreatorLayer(counter))),
    );
    expect(outcome.state._tag).toBe("NoCandidateFound");
    expect(outcome.admissibleCount).toBe(0);
    // The creator was available in context but was never invoked by open.
    expect(await Effect.runPromise(Ref.get(counter))).toBe(0);
  });

  test("an admissible candidate reaches CandidateDiscovered, ready for reconciliation", async () => {
    const outcome = await Effect.runPromise(
      openIdentity(inMemoryCandidateSource([diagnostic({ sourceLabel: "primary" })])),
    );
    expect(outcome.state._tag).toBe("CandidateDiscovered");
    if (outcome.state._tag === "CandidateDiscovered") {
      expect(outcome.state.candidateCount).toBe(1);
    }
    expect(outcome.admissibleCount).toBe(1);
  });

  test("a symbolic-link-only candidate blocks with link_check_failed", async () => {
    const outcome = await Effect.runPromise(
      openIdentity(
        inMemoryCandidateSource([
          diagnostic({ fsType: "symbolic_link", admissible: false, blocker: "link_refused" }),
        ]),
      ),
    );
    expect(outcome.state._tag).toBe("Blocked");
    if (outcome.state._tag === "Blocked") {
      expect(outcome.state.blocker).toBe("link_check_failed");
    }
    expect(outcome.admissibleCount).toBe(0);
  });

  test("a weak-permission-only candidate blocks with permission_denied", async () => {
    const outcome = await Effect.runPromise(
      openIdentity(
        inMemoryCandidateSource([
          diagnostic({ admissible: false, blocker: "weak_permissions", permissionMode: "644" }),
        ]),
      ),
    );
    expect(outcome.state._tag).toBe("Blocked");
    if (outcome.state._tag === "Blocked") {
      expect(outcome.state.blocker).toBe("permission_denied");
    }
  });

  test("an admissible candidate wins even when a blocked candidate is also present", async () => {
    const outcome = await Effect.runPromise(
      openIdentity(
        inMemoryCandidateSource([
          diagnostic({ fsType: "symbolic_link", admissible: false, blocker: "link_refused" }),
          diagnostic({ sourceLabel: "primary" }),
        ]),
      ),
    );
    expect(outcome.state._tag).toBe("CandidateDiscovered");
    expect(outcome.admissibleCount).toBe(1);
  });
});

describe("the SEPARATE explicit createIdentityRoot operation", () => {
  test("an acknowledged intent creates a new root through the injected creator", async () => {
    const counter = await Effect.runPromise(Ref.make(0));
    const created = await Effect.runPromise(
      createIdentityRoot({ acknowledgeCreatesNewRoot: true }).pipe(
        Effect.provide(countingCreatorLayer(counter)),
      ),
    );
    expect(created.identityRef).toBe(identityRef);
    expect(await Effect.runPromise(Ref.get(counter))).toBe(1);
  });

  test("an unacknowledged intent is refused before the creator is reached", async () => {
    const counter = await Effect.runPromise(Ref.make(0));
    const error = await Effect.runPromise(
      createIdentityRoot({ acknowledgeCreatesNewRoot: false }).pipe(
        Effect.provide(countingCreatorLayer(counter)),
        Effect.flip,
      ),
    );
    expect(error).toBeInstanceOf(CreateRootError);
    expect(error.reason).toBe("intent_not_acknowledged");
    // The creator was never reached: acknowledgment is checked first.
    expect(await Effect.runPromise(Ref.get(counter))).toBe(0);
  });
});
