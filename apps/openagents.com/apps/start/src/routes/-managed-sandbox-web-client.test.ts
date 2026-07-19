import {
  decodeManagedSandboxSupervisionEnvelope,
  decodeManagedSandboxSupervisionOutcome,
} from "@openagentsinc/managed-sandbox-contract";
import { describe, expect, test } from "vite-plus/test";

import {
  buildWebManagedSandboxCommand,
  makeWebManagedSandboxController,
  WEB_MANAGED_SANDBOX_OUTBOX_KEY,
} from "./-managed-sandbox-web-client";

const projection = decodeManagedSandboxSupervisionEnvelope({
  observedAt: "2026-07-19T19:00:00.000Z",
  projections: [
    {
      schema: "openagents.managed_sandbox_supervision.v1",
      sandboxRef: "sandbox.web.fixture",
      workUnitRef: "work.web.fixture",
      attachmentRef: "attachment.web.fixture",
      attachmentGeneration: 2,
      resourceGeneration: 3,
      version: 7,
      target: {
        targetRef: "target.gcp.sbx",
        provider: "google_cloud",
        region: "us-central1",
        isolation: "gce_vm",
        custody: "openagents_managed_region",
      },
      state: { lifecycle: "running", runtime: "running", acceptingWork: true },
      timing: {
        createdAt: "2026-07-19T18:00:00.000Z",
        updatedAt: "2026-07-19T18:59:00.000Z",
        leaseExpiresAt: "2026-07-19T20:00:00.000Z",
        elapsedSeconds: 3600,
        idleSeconds: 60,
        leaseState: "active",
      },
      budget: {
        class: "bounded",
        currency: "USD",
        maxCostMicros: 10_000,
        observedCostMicros: null,
        state: "unreported",
      },
      runtime: {
        turnRef: "turn.web.fixture",
        status: "running",
        identity: {
          provider: "claude",
          modelRef: "model.claude-fable-5",
          harnessRef: "harness.claude.v1",
        },
        actorRef: "principal.sarah",
        startedAt: "2026-07-19T18:01:00.000Z",
        settledAt: null,
        terminalReasonRef: null,
      },
      lastStructuralEvent: {
        eventRef: "event.web.fixture.1",
        kind: "RuntimeStarted",
        sequence: 1,
        observedAt: "2026-07-19T18:01:00.000Z",
      },
      attention: { state: "none", reasonRef: null },
      cleanup: { state: "not_started", receiptRef: null },
      outcomes: {
        fileRefs: [],
        changeRefs: [],
        artifactRefs: [],
        evidenceRefs: [],
        receiptRefs: [],
      },
    },
  ],
}).projections[0]!;

const storage = (): Storage => {
  const rows = new Map<string, string>();
  return {
    getItem: (key) => rows.get(key) ?? null,
    setItem: (key, value) => {
      rows.set(key, value);
    },
    removeItem: (key) => {
      rows.delete(key);
    },
    clear: () => rows.clear(),
    key: (index) => [...rows.keys()][index] ?? null,
    get length() {
      return rows.size;
    },
  };
};

describe("authenticated web managed-sandbox controller", () => {
  test("builds only a typed exact-generation command", () => {
    expect(
      buildWebManagedSandboxCommand({
        projection,
        action: "interrupt",
        invocationRef: "click.fixture",
        issuedAt: "2026-07-19T19:00:00.000Z",
      }),
    ).toMatchObject({
      _tag: "Interrupt",
      surface: "web",
      expectedVersion: 7,
      expectedResourceGeneration: 3,
      turnRef: "turn.web.fixture",
    });
    expect(
      buildWebManagedSandboxCommand({
        projection: { ...projection, runtime: null },
        action: "interrupt",
        invocationRef: "click.fixture",
        issuedAt: "2026-07-19T19:00:00.000Z",
      }),
    ).toBeNull();
  });

  test("replays exact stored bytes once after an offline failure", async () => {
    const localStorage = storage();
    const bodies: string[] = [];
    let online = false;
    const controller = makeWebManagedSandboxController({
      storage: localStorage,
      randomId: () => "fixture",
      now: () => new Date("2026-07-19T19:00:00.000Z"),
      fetchImpl: async (_request, init) => {
        bodies.push(String(init?.body));
        if (!online) throw new Error("offline");
        const body = JSON.parse(String(init?.body)) as {
          command: { commandRef: string; idempotencyRef: string };
        };
        return Response.json(
          decodeManagedSandboxSupervisionOutcome({
            schema: "openagents.managed_sandbox_supervision_outcome.v1",
            commandRef: body.command.commandRef,
            idempotencyRef: body.command.idempotencyRef,
            state: "refused",
            reasonRef: "reason.stale_generation",
            receiptRefs: [],
            projection: { ...projection, resourceGeneration: 4 },
            observedAt: "2026-07-19T19:00:01.000Z",
          }),
          { status: 409 },
        );
      },
    });
    await expect(controller.request(projection, "stop")).resolves.toBeNull();
    expect(localStorage.getItem(WEB_MANAGED_SANDBOX_OUTBOX_KEY)).toContain(
      "command.web.sandbox.stop",
    );
    online = true;
    await expect(controller.flush()).resolves.toMatchObject([
      { state: "refused", reasonRef: "reason.stale_generation" },
    ]);
    expect(bodies[1]).toBe(bodies[0]);
    expect(controller.pending()).toEqual([]);
    await controller.flush();
    expect(bodies).toHaveLength(2);
  });
});
