import { describe, expect, it } from "@effect/vitest";
import { Effect, Layer } from "effect";

import {
  emptyWorkCutoverAuthorityState,
  inMemoryWorkCutoverStateStoreLayer,
  WorkCutoverAuthority,
  WorkCutoverAuthorityLive,
} from "../src/work-cutover-authority.ts";

const sourceDigest = "a".repeat(64);
const principalRef = "principal:omega:local-owner";
const initial = () =>
  emptyWorkCutoverAuthorityState({
    organizationRef: "organization:openagents",
    authorizedPrincipalRefs: [principalRef],
    sourceDigest,
    sourceCursor: "cursor:planning:40",
  });

const request = (
  expectedRevision: number,
  expectedGeneration: number,
  id: string,
  command: unknown,
) => ({
  intentRef: `intent:cutover:${id}`,
  idempotencyKey: `work-cutover-${id}`,
  expectedRevision,
  expectedGeneration,
  effectivePrincipalRef: principalRef,
  organizationRef: "organization:openagents",
  capabilityRef: "capability:work-cutover:write",
  occurredAt: "2026-08-03T12:00:00Z",
  githubWriteCount: 0,
  command,
});

const layer = () =>
  WorkCutoverAuthorityLive.pipe(Layer.provide(inMemoryWorkCutoverStateStoreLayer(initial())));

describe("WorkCutoverAuthority", () => {
  it.effect("activates explicitly and refuses rollback with a native history gap", () =>
    Effect.gen(function* () {
      const authority = yield* WorkCutoverAuthority;
      const activated = yield* authority.execute(
        request(1, 1, "activate", {
          command: "activate_native",
          sourceDigest,
          reconciledCursor: "cursor:planning:40",
          receiptRef: "receipt:cutover:1",
        }),
      );
      expect(activated.state).toMatchObject({
        writer: "native_omega",
        generation: 2,
        nativeHighWatermark: "cursor:planning:40",
      });
      const written = yield* authority.execute(
        request(2, 2, "write", {
          command: "record_native_write",
          eventCursor: "cursor:planning:41",
        }),
      );
      expect(written.state.nativeHighWatermark).toBe("cursor:planning:41");
      const gap = yield* Effect.flip(
        authority.execute(
          request(3, 2, "gap", {
            command: "rollback_legacy",
            reconciledNativeCursor: "cursor:planning:40",
            receiptRef: "receipt:rollback:gap",
          }),
        ),
      );
      expect(gap.reason).toBe("native_history_gap");
      const rolledBack = yield* authority.execute(
        request(3, 2, "rollback", {
          command: "rollback_legacy",
          reconciledNativeCursor: "cursor:planning:41",
          receiptRef: "receipt:rollback:1",
        }),
      );
      expect(rolledBack.state).toMatchObject({ writer: "legacy_github", generation: 3 });
      expect(rolledBack.receipt.githubWriteCount).toBe(0);
    }).pipe(Effect.provide(layer())),
  );

  it.effect("fences stale generations and unauthorized principals before mutation", () =>
    Effect.gen(function* () {
      const authority = yield* WorkCutoverAuthority;
      const stale = yield* Effect.flip(
        authority.execute(
          request(1, 0, "stale", {
            command: "bind_shadow",
            sourceDigest: "b".repeat(64),
            sourceCursor: "cursor:planning:41",
          }),
        ),
      );
      expect(stale.reason).toBe("stale_generation");
      const forbiddenInput = {
        ...request(1, 1, "forbidden", {
          command: "bind_shadow",
          sourceDigest: "b".repeat(64),
          sourceCursor: "cursor:planning:41",
        }),
        effectivePrincipalRef: "principal:omega:not-owner",
      };
      const forbidden = yield* Effect.flip(authority.execute(forbiddenInput));
      expect(forbidden.reason).toBe("forbidden");
      const state = yield* authority.read({});
      expect(state.state).toMatchObject({ revision: 1, writer: "legacy_github" });
    }).pipe(Effect.provide(layer())),
  );
});
