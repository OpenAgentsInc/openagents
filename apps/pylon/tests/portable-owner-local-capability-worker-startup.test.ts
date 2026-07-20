import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test";
import { afterEach, describe, expect, test, vi } from "vite-plus/test";
import { mkdtemp, rm } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { Effect } from "effect";

import {
  openPylonOwnerLocalCapabilityWorkerStartup,
  PylonOwnerLocalCapabilityWorkerStartupError,
} from "../src/portable-owner-local-capability-worker-startup.js";
import { PylonPortableSessionOperationLedger } from "../src/portable-session-operation-ledger.js";

const roots: string[] = [];

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

const seed = async (suffix: string) => {
  const root = await mkdtemp(join(tmpdir(), `pylon-capability-startup-${suffix}-`));
  roots.push(root);
  const database = new NodeTestDatabase(join(root, "portable.sqlite"), { create: true });
  const ledger = new PylonPortableSessionOperationLedger(database);
  const sessionRef = `session.ide13.capability-startup.${suffix}`;
  const attachmentRef = `attachment.ide13.capability-startup.${suffix}.1`;
  await Effect.runPromise(
    ledger.registerSession({
      sessionRef,
      attachmentRef,
      generation: 1,
      acceptingWork: true,
    }),
  );
  const stored = await Effect.runPromise(
    ledger.persistControlBinding({
      sessionRef,
      attachmentRef,
      generation: 1,
      runtimeInstanceRef: `runtime.ide13.capability-startup.${suffix}`,
      agents: [
        {
          agentRef: `agent.ide13.capability-startup.${suffix}`,
          controlSessionRef: `control.ide13.capability-startup.${suffix}`,
          workspaceRef: `workspace.ide13.capability-startup.${suffix}`,
        },
      ],
    }),
  );
  return {
    root,
    database,
    ledger,
    binding: stored.binding,
    sessionRef,
  };
};

describe("owner-local capability worker startup", () => {
  test("does not start by default without the exact current target binding", async () => {
    const fixture = await seed("default");
    let passes = 0;
    let workers = 0;
    const startup = openPylonOwnerLocalCapabilityWorkerStartup({
      agentToken: "agent-token-fixture-0123456789",
      baseUrl: "http://127.0.0.1:8787",
      pylonHome: join(fixture.root, "isolated-pylon-home"),
      pylonRef: "pylon.ide13.capability-startup.default",
      targetRef: "target.ide13.capability-startup.default",
      sessionRef: fixture.sessionRef,
      workerInstanceRef: "worker.ide13.capability-startup.default",
      binding: fixture.binding,
      ledger: fixture.ledger,
      authorityStore: { authorizesCapability: () => true },
      targetBindingIsCurrent: () => false,
      pollIntervalMs: 10,
      bindingCheckIntervalMs: 10,
      dependencies: {
        makeWorker: () => {
          workers += 1;
          return {
            runPass: async () => {
              passes += 1;
              return 0;
            },
          };
        },
      },
    });

    await expect(startup.reconcile()).resolves.toMatchObject({
      state: "waiting_binding",
      active: false,
      material: "excluded",
    });
    expect(passes).toBe(0);
    expect(workers).toBe(0);
    await startup.close();
    fixture.database.close();
  });

  test("starts once, keeps a stable restart identity, and stops on binding loss", async () => {
    const fixture = await seed("restart");
    let bindingCurrent = true;
    let firstPasses = 0;
    let workers = 0;
    const open = (onPass: () => void, binding = fixture.binding) =>
      openPylonOwnerLocalCapabilityWorkerStartup({
        agentToken: "agent-token-fixture-0123456789",
        baseUrl: "http://127.0.0.1:8787",
        pylonHome: join(fixture.root, "isolated-pylon-home"),
        pylonRef: "pylon.ide13.capability-startup.restart",
        targetRef: "target.ide13.capability-startup.restart",
        sessionRef: fixture.sessionRef,
        workerInstanceRef: "worker.ide13.capability-startup.restart",
        binding,
        ledger: fixture.ledger,
        authorityStore: { authorizesCapability: () => true },
        targetBindingIsCurrent: () => bindingCurrent,
        pollIntervalMs: 10,
        bindingCheckIntervalMs: 10,
        dependencies: {
          makeWorker: () => {
            workers += 1;
            return {
              runPass: async () => {
                onPass();
                return 0;
              },
            };
          },
        },
      });

    const first = open(() => {
      firstPasses += 1;
    });
    const running = await first.reconcile();
    expect(running).toMatchObject({ state: "running", active: true });
    await first.reconcile();
    expect(workers).toBe(1);
    await vi.waitFor(() => expect(firstPasses).toBeGreaterThan(0));
    const workerInstanceRef = first.status().workerInstanceRef;
    await first.close();

    const recovered = await Effect.runPromise(fixture.ledger.recoverControlBinding({
      recoveryRef: "recovery.ide13.capability-startup.restart",
      sessionRef: fixture.sessionRef,
      attachmentRef: fixture.binding.attachmentRef,
      generation: fixture.binding.generation,
      runtimeInstanceRef: "runtime.ide13.capability-startup.restart.after",
    }));
    let restartedPasses = 0;
    const restarted = open(() => {
      restartedPasses += 1;
    }, recovered.binding);
    expect((await restarted.reconcile()).workerInstanceRef).toBe(workerInstanceRef);
    expect(workers).toBe(2);
    await vi.waitFor(() => expect(restartedPasses).toBeGreaterThan(0));

    bindingCurrent = false;
    await vi.waitFor(() => expect(restarted.status()).toMatchObject({
      state: "binding_lost",
      active: false,
    }));
    await restarted.close();
    fixture.database.close();
  });

  test("rejects the default Codex home before it creates a worker", async () => {
    const fixture = await seed("codex-home");
    expect(() =>
      openPylonOwnerLocalCapabilityWorkerStartup({
        agentToken: "agent-token-fixture-0123456789",
        baseUrl: "http://127.0.0.1:8787",
        pylonHome: join(homedir(), ".codex"),
        pylonRef: "pylon.ide13.capability-startup.codex-home",
        targetRef: "target.ide13.capability-startup.codex-home",
        sessionRef: fixture.sessionRef,
        workerInstanceRef: "worker.ide13.capability-startup.codex-home",
        binding: fixture.binding,
        ledger: fixture.ledger,
        authorityStore: { authorizesCapability: () => true },
        targetBindingIsCurrent: () => true,
      }),
    ).toThrow(PylonOwnerLocalCapabilityWorkerStartupError);
    fixture.database.close();
  });
});
