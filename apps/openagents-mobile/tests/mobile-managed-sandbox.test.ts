import {
  decodeManagedSandboxSupervisionEnvelope,
  decodeManagedSandboxSupervisionOutcome,
  type ManagedSandboxSupervisionOutcome,
  type ManagedSandboxSupervisionProjection,
} from "@openagentsinc/managed-sandbox-contract";
import { describe, expect, test } from "vite-plus/test";

import {
  buildMobileManagedSandboxCommand,
  fetchMobileManagedSandboxes,
  makeMobileManagedSandboxController,
  type MobileManagedSandboxOutbox,
  type MobileManagedSandboxOutboxRecord,
} from "../src/managed-sandbox/mobile-managed-sandbox";
import { renderMobileManagedSandboxViews } from "../src/screens/mobile-managed-sandbox-view";

const contractId = "openagents_mobile.managed_sandbox.supervision.v1";

const projection = decodeManagedSandboxSupervisionEnvelope({
  observedAt: "2026-07-19T19:00:00.000Z",
  projections: [
    {
      schema: "openagents.managed_sandbox_supervision.v1",
      sandboxRef: "sandbox.mobile.fixture",
      workUnitRef: "work.mobile.fixture",
      attachmentRef: "attachment.mobile.fixture",
      attachmentGeneration: 4,
      resourceGeneration: 3,
      version: 12,
      target: {
        targetRef: "target.gcp.sbx.staging",
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
        elapsedSeconds: 3_600,
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
        turnRef: "turn.mobile.fixture",
        status: "running",
        identity: {
          provider: "codex",
          modelRef: "model.gpt-5.6-sol",
          harnessRef: "harness.codex.v1",
        },
        actorRef: "principal.sarah",
        startedAt: "2026-07-19T18:01:00.000Z",
        settledAt: null,
        terminalReasonRef: null,
      },
      lastStructuralEvent: {
        eventRef: "event.mobile.fixture.8",
        kind: "RuntimeToolCompleted",
        sequence: 8,
        observedAt: "2026-07-19T18:59:00.000Z",
      },
      attention: { state: "none", reasonRef: null },
      cleanup: { state: "not_started", receiptRef: null },
      outcomes: {
        fileRefs: ["file.safe.fixture"],
        changeRefs: ["change.safe.fixture"],
        artifactRefs: ["artifact.safe.fixture"],
        evidenceRefs: ["evidence.safe.fixture"],
        receiptRefs: ["receipt.safe.fixture"],
      },
    },
  ],
}).projections[0]!;

const outcome = (
  commandRef: string,
  idempotencyRef: string,
  overrides: Partial<ManagedSandboxSupervisionOutcome> = {},
): ManagedSandboxSupervisionOutcome =>
  decodeManagedSandboxSupervisionOutcome({
    schema: "openagents.managed_sandbox_supervision_outcome.v1",
    commandRef,
    idempotencyRef,
    state: "applied",
    reasonRef: null,
    receiptRefs: ["receipt.mobile.control.fixture"],
    projection,
    observedAt: "2026-07-19T19:00:01.000Z",
    ...overrides,
  });

const memoryOutbox = (): MobileManagedSandboxOutbox &
  Readonly<{
    rows: Map<string, MobileManagedSandboxOutboxRecord>;
  }> => {
  const rows = new Map<string, MobileManagedSandboxOutboxRecord>();
  return {
    rows,
    put: async (record) => {
      const existing = rows.get(record.command.commandRef);
      if (existing !== undefined && existing.bodyJson !== record.bodyJson) {
        throw new Error("conflicting bytes");
      }
      if (existing === undefined) rows.set(record.command.commandRef, record);
    },
    pending: async () => [...rows.values()].filter((row) => row.outcome === null),
    settle: async (commandRef, settled) => {
      const row = rows.get(commandRef);
      if (row !== undefined && row.outcome === null)
        rows.set(commandRef, { ...row, outcome: settled });
    },
  };
};

describe("mobile managed-sandbox supervision", () => {
  test("decodes create/progress/attention/budget/cleanup truth without private client capability", async () => {
    expect(contractId).toBe("openagents_mobile.managed_sandbox.supervision.v1");
    const response = await fetchMobileManagedSandboxes({
      baseUrl: "https://openagents.com",
      accessToken: "fixture-token",
      fetchImpl: async (request, init) => {
        expect(String(request)).toBe(
          "https://openagents.com/api/managed-sandboxes/mobile/supervision",
        );
        expect(init?.headers).toEqual({ authorization: "Bearer fixture-token" });
        return Response.json({
          projections: [
            projection,
            {
              ...projection,
              sandboxRef: "sandbox.mobile.recovery",
              state: { lifecycle: "recovery_required", runtime: "failed", acceptingWork: false },
              attention: { state: "recovery_required", reasonRef: "reason.teardown_failed" },
              cleanup: { state: "recovery_required", receiptRef: null },
            },
          ],
          observedAt: "2026-07-19T19:00:00.000Z",
        });
      },
    });
    expect(response).toMatchObject({
      state: "available",
      envelope: {
        projections: [
          { runtime: { actorRef: "principal.sarah" }, budget: { class: "bounded" } },
          { attention: { state: "recovery_required" }, cleanup: { state: "recovery_required" } },
        ],
      },
    });
    const bytes = JSON.stringify(response);
    expect(bytes).not.toMatch(
      /rawPath|runtimeOutput|providerCredential|privateKey|shell|pty|prompt/iu,
    );
  });

  test("builds only generation-fenced legal controls", () => {
    const interrupt = buildMobileManagedSandboxCommand({
      projection,
      action: "interrupt",
      invocationRef: "tap.fixture.1",
      issuedAt: "2026-07-19T19:00:00.000Z",
    });
    expect(interrupt).toMatchObject({
      state: "ready",
      command: {
        _tag: "Interrupt",
        surface: "mobile",
        expectedVersion: 12,
        expectedResourceGeneration: 3,
        turnRef: "turn.mobile.fixture",
      },
    });
    expect(
      buildMobileManagedSandboxCommand({
        projection: { ...projection, runtime: null },
        action: "interrupt",
        invocationRef: "tap.fixture.2",
        issuedAt: "2026-07-19T19:00:00.000Z",
      }),
    ).toEqual({ state: "rejected", reasonRef: "reason.action_unavailable" });
  });

  test("renders target, generation, runtime actor, timing, budget, structural event, outcomes, and cleanup", () => {
    const rendered = JSON.stringify(
      renderMobileManagedSandboxViews({
        snapshot: {
          state: "available",
          envelope: {
            projections: [projection],
            observedAt: "2026-07-19T19:00:00.000Z",
          },
        },
        pending: null,
        lastOutcome: null,
        deleteConfirmRef: projection.sandboxRef,
      }),
    );
    for (const value of [
      projection.workUnitRef,
      projection.target.region,
      "generation 3",
      "Sarah",
      "model.gpt-5.6-sol",
      "harness.codex.v1",
      "elapsed 1h 0m",
      "$0.01 cap",
      "RuntimeToolCompleted",
      "file.safe.fixture",
      "change.safe.fixture",
      "artifact.safe.fixture",
      "Cleanup not started",
      "Confirm delete",
      "zero-residue cleanup receipt",
    ])
      expect(rendered).toContain(value);
    expect(rendered).not.toMatch(
      /providerCredential|rawPath|runtimeOutput|privateKey|shell|pty|prompt/iu,
    );
  });

  test("replays one exact offline command and completes only from the durable outcome", async () => {
    const outbox = memoryOutbox();
    let online = false;
    const requests: string[] = [];
    const controller = makeMobileManagedSandboxController({
      baseUrl: "https://openagents.com",
      accessToken: () => "fixture-token",
      outbox,
      randomId: () => "fixture.3",
      now: () => new Date("2026-07-19T19:00:00.000Z"),
      fetchImpl: async (_request, init) => {
        requests.push(String(init?.body));
        if (!online) throw new Error("offline");
        const parsed = JSON.parse(String(init?.body)) as {
          command: { commandRef: string; idempotencyRef: string };
        };
        return Response.json(outcome(parsed.command.commandRef, parsed.command.idempotencyRef));
      },
    });
    await expect(controller.request(projection, "interrupt")).resolves.toEqual({
      state: "pending",
    });
    expect(outbox.rows.size).toBe(1);
    const firstBytes = requests[0];
    online = true;
    const settled = await controller.flush();
    expect(settled).toHaveLength(1);
    expect(requests[1]).toBe(firstBytes);
    expect(await controller.flush()).toEqual([]);
    expect(requests).toHaveLength(2);
  });

  test("settles revoke and stale-generation refusals instead of retrying them as success", async () => {
    for (const reasonRef of ["reason.permission_denied", "reason.stale_generation"] as const) {
      const outbox = memoryOutbox();
      const controller = makeMobileManagedSandboxController({
        baseUrl: "https://openagents.com",
        accessToken: () => "fixture-token",
        outbox,
        randomId: () => reasonRef.replaceAll(".", "_"),
        now: () => new Date("2026-07-19T19:00:00.000Z"),
        fetchImpl: async (_request, init) => {
          const parsed = JSON.parse(String(init?.body)) as {
            command: { commandRef: string; idempotencyRef: string };
          };
          return Response.json(
            outcome(parsed.command.commandRef, parsed.command.idempotencyRef, {
              state: "refused",
              reasonRef,
              projection:
                reasonRef === "reason.stale_generation"
                  ? ({
                      ...projection,
                      resourceGeneration: 4,
                    } as ManagedSandboxSupervisionProjection)
                  : null,
            }),
            { status: 409 },
          );
        },
      });
      const result = await controller.request(projection, "stop");
      expect(result).toMatchObject({
        state: "settled",
        outcome: { state: "refused", reasonRef },
      });
      expect(await outbox.pending()).toEqual([]);
    }
  });
});
