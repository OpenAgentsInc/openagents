import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test";
import { afterEach, describe, expect, test } from "vite-plus/test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Effect } from "effect";

import type { PylonPortableControlSessionLifecycle } from "../src/node/control-sessions.js";
import {
  PylonOwnerLocalTargetStartupError,
  registerPylonOwnerLocalExecutionTarget,
} from "../src/portable-owner-local-target-startup.js";
import {
  makeDurablePylonPortablePhaseTargetResolver,
  openPylonPortablePhaseContextAdmissionStore,
} from "../src/portable-phase-context-admission.js";
import { makePylonPrivatePortablePhaseContextResolver } from "../src/portable-phase-production.js";
import { PylonPortableSessionOperationLedger } from "../src/portable-session-operation-ledger.js";

const roots: string[] = [];

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

const lifecycle = (bound: Array<unknown>): PylonPortableControlSessionLifecycle => ({
  bind: (input) => {
    bound.push(input);
  },
  recover: async () => {
    throw new Error("not used");
  },
  quiesce: async () => {
    throw new Error("not used");
  },
  checkpointSource: async () => {
    throw new Error("not used");
  },
  cleanup: async () => {
    throw new Error("not used");
  },
  stageDestination: async () => {
    throw new Error("not used");
  },
  activateDestination: async () => {
    throw new Error("not used");
  },
  abortDestination: async () => {
    throw new Error("not used");
  },
});

const registerBinding = (
  ledger: PylonPortableSessionOperationLedger,
  suffix: string,
  runtimeInstanceRef = "runtime.pylon.ide13.owner_local.before",
) =>
  Effect.gen(function* () {
    const sessionRef = `session.pylon.ide13.${suffix}`;
    const attachmentRef = `attachment.pylon.ide13.${suffix}.1`;
    yield* ledger.registerSession({
      sessionRef,
      attachmentRef,
      generation: 1,
      acceptingWork: true,
    });
    yield* ledger.persistControlBinding({
      sessionRef,
      attachmentRef,
      generation: 1,
      runtimeInstanceRef,
      agents: [
        {
          agentRef: `agent.pylon.ide13.${suffix}`,
          controlSessionRef: `control.pylon.ide13.${suffix}`,
          workspaceRef: `workspace.pylon.ide13.${suffix}`,
        },
      ],
    });
    return { sessionRef, attachmentRef };
  });

const startup = (
  ledger: PylonPortableSessionOperationLedger,
  sessionRef: string,
  bound: Array<unknown> = [],
) => {
  const registry = makePylonPrivatePortablePhaseContextResolver();
  return {
    bound,
    registry,
    run: () =>
      registerPylonOwnerLocalExecutionTarget({
        pylonRef: "pylon.ide13.owner_local",
        targetRef: "target.ide13.owner_local",
        sessionRef,
        ledger,
        lifecycle: lifecycle(bound),
        registry,
      }),
  };
};

describe("Pylon owner-local target startup", () => {
  test("registers the target from the only exact durable binding", () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const root = yield* Effect.promise(() =>
          mkdtemp(join(tmpdir(), "pylon-owner-local-target-admission-")),
        );
        roots.push(root);
        const database = new NodeTestDatabase(":memory:");
        const ledger = new PylonPortableSessionOperationLedger(database);
        const binding = yield* registerBinding(ledger, "exact");
        const composed = startup(ledger, binding.sessionRef);

        const target = yield* Effect.promise(composed.run);

        expect(target.targetRef).toBe("target.ide13.owner_local");
        expect(composed.registry.target(target.targetRef)).toBe(target);
        expect(composed.bound).toEqual([
          {
            sessionRef: binding.sessionRef,
            attachmentRef: binding.attachmentRef,
            generation: 1,
            agents: [
              {
                agentRef: "agent.pylon.ide13.exact",
                controlSessionRef: "control.pylon.ide13.exact",
              },
            ],
          },
        ]);
        const admission = yield* Effect.promise(() =>
          openPylonPortablePhaseContextAdmissionStore({
            databasePath: join(root, "context-admissions.sqlite"),
            pylonRef: "pylon.ide13.owner_local",
            targetRef: target.targetRef,
          }),
        );
        const request = {
          schema: "openagents.portable_phase_operation.v1" as const,
          operationRef: "operation.pylon.ide13.exact.quiesce",
          commandRef: "command.pylon.ide13.exact",
          commandExecutionClaimRef: "claim.pylon.ide13.exact",
          ownerRef: "owner.pylon.ide13.exact",
          sessionRef: binding.sessionRef,
          attachmentRef: binding.attachmentRef,
          attachmentGeneration: 1,
          targetRef: target.targetRef,
          pylonRef: "pylon.ide13.owner_local",
          kind: "quiesce" as const,
          checkpointRef: null,
          checkpointObjectRef: null,
          checkpointDigest: null,
          evidenceRefs: [],
          expiresAt: "2096-07-20T12:10:00.000Z",
        };
        admission.store.admit({
          schema: "openagents.pylon.portable_phase_context_admission.v1",
          request,
          payload: {
            kind: "quiesce",
            input: {
              operationRef: request.operationRef,
              sessionRef: request.sessionRef,
              attachmentRef: request.attachmentRef,
              generation: request.attachmentGeneration,
              graph: {
                rootAgentRef: "agent.pylon.ide13.exact",
                nodes: [
                  {
                    agentRef: "agent.pylon.ide13.exact",
                    threadRef: "thread.pylon.ide13.exact",
                    transcriptRef: "transcript.pylon.ide13.exact",
                    activityCursor: 0,
                    lifecycle: "running",
                    attachmentGeneration: 1,
                  },
                ],
              },
              threadCursors: [],
            },
          },
          recoverySemantics: "operation_ref_idempotent",
        });
        const resolver = makeDurablePylonPortablePhaseTargetResolver({
          store: admission.store,
          target: (targetRef) => composed.registry.target(targetRef),
        });
        expect(yield* Effect.promise(() => resolver.resolve(request))).toMatchObject({
          target,
          call: { kind: "quiesce" },
          operationRefSemantics: "operation_ref_idempotent",
        });
        admission.close();
        database.close();
      }),
    ));

  test("fails before registration when the binding is absent, ambiguous, or stale", () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const absentDatabase = new NodeTestDatabase(":memory:");
        const absentLedger = new PylonPortableSessionOperationLedger(absentDatabase);
        yield* Effect.promise(() =>
          expect(startup(absentLedger, "session.pylon.ide13.absent").run()).rejects.toEqual(
            new PylonOwnerLocalTargetStartupError({ reason: "binding_absent" }),
          ),
        );
        absentDatabase.close();

        const ambiguousDatabase = new NodeTestDatabase(":memory:");
        const ambiguousLedger = new PylonPortableSessionOperationLedger(ambiguousDatabase);
        const first = yield* registerBinding(ambiguousLedger, "ambiguous_a");
        yield* registerBinding(ambiguousLedger, "ambiguous_b");
        yield* Effect.promise(() =>
          expect(startup(ambiguousLedger, first.sessionRef).run()).rejects.toEqual(
            new PylonOwnerLocalTargetStartupError({ reason: "binding_ambiguous" }),
          ),
        );
        ambiguousDatabase.close();

        const staleDatabase = new NodeTestDatabase(":memory:");
        const staleLedger = new PylonPortableSessionOperationLedger(staleDatabase);
        const stale = yield* registerBinding(staleLedger, "stale");
        yield* staleLedger.quiesceGeneration({
          operationRef: "operation.pylon.ide13.stale.quiesce",
          sessionRef: stale.sessionRef,
          attachmentRef: stale.attachmentRef,
          generation: 1,
          evidenceRefs: ["evidence.pylon.ide13.stale.quiesced"],
        });
        yield* Effect.promise(() =>
          expect(startup(staleLedger, stale.sessionRef).run()).rejects.toEqual(
            new PylonOwnerLocalTargetStartupError({ reason: "binding_stale" }),
          ),
        );
        staleDatabase.close();
      }),
    ));

  test("reuses one exact recovered binding after restart", () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const root = yield* Effect.promise(() =>
          mkdtemp(join(tmpdir(), "pylon-owner-local-target-startup-")),
        );
        roots.push(root);
        const path = join(root, "portable.sqlite");
        const firstDatabase = new NodeTestDatabase(path, { create: true });
        const firstLedger = new PylonPortableSessionOperationLedger(firstDatabase);
        const binding = yield* registerBinding(firstLedger, "restart");
        firstDatabase.close();

        const restartedDatabase = new NodeTestDatabase(path);
        const restartedLedger = new PylonPortableSessionOperationLedger(restartedDatabase);
        yield* restartedLedger.recoverControlBinding({
          recoveryRef: "recovery.pylon.ide13.owner_local.restart",
          sessionRef: binding.sessionRef,
          attachmentRef: binding.attachmentRef,
          generation: 1,
          runtimeInstanceRef: "runtime.pylon.ide13.owner_local.after",
        });
        const composed = startup(restartedLedger, binding.sessionRef);

        const target = yield* Effect.promise(composed.run);

        expect(composed.registry.target(target.targetRef)).toBe(target);
        expect(composed.bound).toHaveLength(1);
        expect(yield* restartedLedger.readControlBinding(binding.sessionRef)).toMatchObject({
          state: "quiesced",
          runtimeInstanceRef: "runtime.pylon.ide13.owner_local.after",
        });
        restartedDatabase.close();
      }),
    ));
});
