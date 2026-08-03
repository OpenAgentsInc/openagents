import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  decodeRepositoryClaimExecuteResult,
  decodeRepositoryClaimReadResult,
  decodeProtocolInitializeResult,
  decodePlanningGraphReadResult,
  decodeWorkIndexReadResult,
  decodeWorkSnapshotReadResult,
} from "@openagentsinc/all-work-contract";
import { describe, expect, test } from "vite-plus/test";

import { createOmegaEffectdService } from "../service.ts";
import { openFullAutoRegistry } from "../engine/full-auto-registry.ts";
import { openFullAutoRunRegistry } from "../engine/full-auto-run-registry.ts";
import { resolveFullAutoRegistryPath, resolveFullAutoRunsPath } from "../paths.ts";
import { createOmegaEffectdFramedServer } from "./server.ts";
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA } from "./framed.ts";
import { makeOmegaEffectdTestHost } from "./test-host.ts";

const withRoot = async (fn: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-effectd-framed-"));
  try {
    await fn(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const request = (id: string, generation: number, method: string, params?: unknown) =>
  JSON.stringify({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "request",
    id,
    generation,
    method,
    ...(params === undefined ? {} : { params }),
  });

describe("omega-effectd framed protocol", () => {
  test("initialize, health, generation fence, and disk recovery", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );

      const init = await server.handleLine(request("1", 0, "initialize", { generation: 7 }));
      expect(init?.ok).toBe(true);
      expect((init?.result as { generation: number }).generation).toBe(7);

      const health = await server.handleLine(request("2", 7, "health"));
      expect(health?.ok).toBe(true);
      expect((health?.result as { status: string }).status).toBe("running");

      const stale = await server.handleLine(request("3", 6, "health"));
      expect(stale?.ok).toBe(false);
      expect(stale?.error?.code).toBe("stale_generation");

      const runsPath = resolveFullAutoRunsPath({ dataRoot: root });
      const registry = openFullAutoRunRegistry(runsPath);
      const created = registry.createDraft({
        title: "Recovery proof",
        objective: "Prove durable truth survives restart.",
        doneCondition: "Runs list still shows this runRef.",
        objectiveSource: "user",
        workspaceRef: "workspace.omega.supervised",
      });
      expect(created.runRef).toMatch(/^run\.full-auto\./);

      // New process-equivalent: new service + framed server on same data root.
      const serviceB = createOmegaEffectdService({ paths: { dataRoot: root } });
      const serverB = createOmegaEffectdFramedServer(
        serviceB,
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );
      await serverB.handleLine(request("10", 0, "initialize", { generation: 8 }));
      const listed = await serverB.handleLine(request("11", 8, "list_runs"));
      expect(listed?.ok).toBe(true);
      const runs = (listed?.result as { runs: Array<{ runRef: string; title: string }> }).runs;
      expect(
        runs.some((run) => run.runRef === created.runRef && run.title === "Recovery proof"),
      ).toBe(true);
      expect(JSON.stringify(listed)).not.toContain("Prove durable truth");
    });
  });

  test("negotiates typed All Work v2 reads while explicit v1 remains rollback-only", async () => {
    await withRoot(async (root) => {
      const run = openFullAutoRunRegistry(resolveFullAutoRunsPath({ dataRoot: root })).createDraft({
        title: "All Work process row",
        objective: "This private objective must not enter the Work Index.",
        doneCondition: "Typed process reads pass.",
        objectiveSource: "user",
        threadRef: "thread:all-work:1",
      });
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );
      const initialized = await server.handleLine(
        request("init-v2", 0, "initialize", {
          generation: 1,
          allWork: {
            supportedVersions: ["omega-effectd.v2", "omega-effectd.v1"],
            requestedCapabilities: ["work.index.read", "work.snapshot.read", "planning.graph.read"],
          },
        }),
      );
      expect(initialized?.ok).toBe(true);
      const allWork = decodeProtocolInitializeResult(
        (initialized?.result as { allWork: unknown }).allWork,
      );
      expect(allWork.selectedVersion).toBe("omega-effectd.v2");
      expect(allWork.capabilities).toEqual([
        "work.index.read",
        "work.snapshot.read",
        "planning.graph.read",
      ]);

      const planning = await server.handleLine(request("planning", 1, "planning.graph.read", {}));
      expect(planning?.ok).toBe(true);
      const planningResult = decodePlanningGraphReadResult(planning?.result);
      expect(planningResult.graph.work).toHaveLength(28);
      expect(
        planningResult.graph.work.filter((work) => work.summary.state === "completed"),
      ).toHaveLength(6);
      expect(planningResult.graph.sourceCoordinates).toHaveLength(28);
      expect(planningResult.graph.releaseScopeLinks).toHaveLength(10);

      const indexed = await server.handleLine(request("index", 1, "work.index.read", {}));
      expect(indexed?.ok).toBe(true);
      const indexResult = decodeWorkIndexReadResult(indexed?.result);
      expect(indexResult.items).toHaveLength(1);
      expect(indexResult.items[0]?.workRef).toBe(`work:${run.runRef}`);
      expect(JSON.stringify(indexResult)).not.toContain("private objective");

      const snapshot = await server.handleLine(
        request("snapshot", 1, "work.snapshot.read", { workRef: `work:${run.runRef}` }),
      );
      expect(snapshot?.ok).toBe(true);
      expect(decodeWorkSnapshotReadResult(snapshot?.result).snapshot.threadRefs).toEqual([
        "thread:all-work:1",
      ]);

      const legacyRoot = path.join(root, "legacy");
      const legacy = createOmegaEffectdFramedServer(
        createOmegaEffectdService({ paths: { dataRoot: legacyRoot } }),
        { dataRoot: legacyRoot },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );
      const legacyInit = await legacy.handleLine(
        request("init-v1", 0, "initialize", { generation: 1 }),
      );
      expect(
        decodeProtocolInitializeResult((legacyInit?.result as { allWork: unknown }).allWork)
          .selectedVersion,
      ).toBe("omega-effectd.v1");
      const refused = await legacy.handleLine(request("legacy-index", 1, "work.index.read", {}));
      expect(refused?.ok).toBe(false);
      expect(refused?.error?.code).toBe("incompatible_version");
    });
  });

  test("executes and restarts a native Repository Work Claim through the generated processor", async () => {
    await withRoot(async (root) => {
      const open = () =>
        createOmegaEffectdFramedServer(
          createOmegaEffectdService({ paths: { dataRoot: root } }),
          { dataRoot: root },
          { hostRequestHandler: makeOmegaEffectdTestHost() },
        );
      const server = open();
      const init = await server.handleLine(
        request("claim-init", 0, "initialize", {
          generation: 1,
          allWork: {
            supportedVersions: ["omega-effectd.v2"],
            requestedCapabilities: ["repository.claim.read", "repository.claim.execute"],
          },
        }),
      );
      expect(init?.ok).toBe(true);
      const execute = async (id: string, expectedRevision: number, command: unknown) => {
        const response = await server.handleLine(
          request(id, 1, "repository.claim.execute", {
            requestRef: `claim-request:${id}`,
            idempotencyKey: `repository-claim-${id}`,
            expectedRevision,
            effectivePrincipalRef: "principal:omega:local-owner",
            capabilityRef: "capability:repository-claim:write",
            occurredAt: "2026-08-03T08:00:00Z",
            command,
          }),
        );
        expect(response?.ok).toBe(true);
        return decodeRepositoryClaimExecuteResult(response?.result);
      };
      await execute("packet", 0, {
        command: "create_packet",
        packetRef: "work-packet:omega-224",
        workRef: "work:github:openagentsinc-omega:224",
        repositoryRef: "repository:omega",
        title: "Implement native Repository Work Claims",
        scope: "Generated-client claim journey.",
        ownedPaths: ["crates/omega_work_index"],
        hotFiles: [],
        hotContracts: ["all-work generated contract"],
        verification: "Run the focused claim journey.",
      });
      const claimed = await execute("claim", 1, {
        command: "claim_packet",
        packetRef: "work-packet:omega-224",
        claimRef: "repository-claim:omega-224",
      });
      expect(claimed.ledger.claims[0]).toMatchObject({ state: "claimed", generation: 1 });
      expect(claimed.receipt.githubWriteCount).toBe(0);

      const restarted = open();
      await restarted.handleLine(
        request("restart-init", 0, "initialize", {
          generation: 2,
          allWork: {
            supportedVersions: ["omega-effectd.v2"],
            requestedCapabilities: ["repository.claim.read"],
          },
        }),
      );
      const read = await restarted.handleLine(
        request("claim-read", 2, "repository.claim.read", {}),
      );
      expect(read?.ok).toBe(true);
      const ledger = decodeRepositoryClaimReadResult(read?.result).ledger;
      expect(ledger.claims[0]?.claimRef).toBe("repository-claim:omega-224");
      expect(JSON.stringify(ledger)).not.toContain("githubWrite");
    });
  });

  test("start, get_run detail, pause, resume, and stop for FA-03 launcher", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const dispatchMessages: string[] = [];
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: makeOmegaEffectdTestHost((hostRequest) => {
            if (hostRequest.method !== "dispatch_turn") return;
            const params = hostRequest.params as { message?: unknown };
            if (typeof params.message === "string") dispatchMessages.push(params.message);
          }),
        },
      );
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }));

      const started = await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-03 start",
          objective: "Start one run from the framed protocol.",
          doneCondition: "Run exists in get_run with running or paused state.",
          turnCap: 12,
        }),
      );
      expect(started?.ok).toBe(true);
      const run = (started?.result as { run: { runRef: string; objective: string; state: string } })
        .run;
      expect(run.runRef).toMatch(/^run\.full-auto\./);
      expect(run.objective).toContain("framed protocol");

      const earlyHandoff = await server.handleLine(
        request("handoff-early", 1, "handoff", {
          runRef: run.runRef,
          targetLaneRef: "claude-local",
        }),
      );
      expect(earlyHandoff?.ok).toBe(false);
      expect(earlyHandoff?.error?.message).toContain("only while paused");

      const detail = await server.handleLine(request("3", 1, "get_run", { runRef: run.runRef }));
      expect(detail?.ok).toBe(true);
      expect((detail?.result as { run: { title: string } }).run.title).toBe("FA-03 start");

      const paused = await server.handleLine(request("4", 1, "pause", { runRef: run.runRef }));
      expect(paused?.ok).toBe(true);
      expect((paused?.result as { run: { state: string } }).run.state).toMatch(/paus/);

      const refusedHandoff = await server.handleLine(
        request("handoff-refused", 1, "handoff", {
          runRef: run.runRef,
          targetLaneRef: "lane.unknown",
        }),
      );
      expect(refusedHandoff?.ok).toBe(false);

      const sourceThreadRef = openFullAutoRunRegistry(
        resolveFullAutoRunsPath({ dataRoot: root }),
      ).get(run.runRef)?.threadRef;
      expect(sourceThreadRef).toBeDefined();

      const handedOff = await server.handleLine(
        request("handoff-ok", 1, "handoff", {
          runRef: run.runRef,
          targetLaneRef: "claude-local",
          reason:
            "Owner requested a second provider with sk-live-fake-api-key-should-never-appear-1234567890.",
        }),
      );
      expect(handedOff?.ok).toBe(true);
      const handoffResult = handedOff?.result as {
        run: { lane: string };
        transition: {
          from: string;
          to: string;
          disposition: string;
          sourceThreadRef?: string;
          targetThreadRef?: string;
        };
      };
      expect(handoffResult.run.lane).toBe("claude-local");
      expect(handoffResult.transition).toMatchObject({
        from: "codex-local",
        to: "claude-local",
        disposition: "complete_within_bounds",
      });
      expect(JSON.stringify(handedOff)).not.toContain(
        "sk-live-fake-api-key-should-never-appear-1234567890",
      );
      expect(
        openFullAutoRunRegistry(resolveFullAutoRunsPath({ dataRoot: root })).get(run.runRef)
          ?.profile?.lane,
      ).toBe("claude-local");
      const boundThread = openFullAutoRunRegistry(resolveFullAutoRunsPath({ dataRoot: root })).get(
        run.runRef,
      )?.threadRef;
      expect(boundThread).toBeDefined();
      expect(boundThread).not.toBe(sourceThreadRef);
      expect(handoffResult.transition.sourceThreadRef).toBe(sourceThreadRef);
      expect(handoffResult.transition.targetThreadRef).toBe(boundThread);
      const threadRegistry = openFullAutoRegistry(resolveFullAutoRegistryPath({ dataRoot: root }));
      expect(
        sourceThreadRef === undefined ? null : threadRegistry.record(sourceThreadRef),
      ).toBeNull();
      expect(
        boundThread === undefined ? undefined : threadRegistry.record(boundThread)?.profile?.lane,
      ).toBe("claude-local");

      const resumed = await server.handleLine(request("5", 1, "resume", { runRef: run.runRef }));
      expect(resumed?.ok).toBe(true);
      expect(dispatchMessages.at(-1)).toContain('"from": "codex-local"');
      expect(dispatchMessages.at(-1)).toContain('"to": "claude-local"');
      expect(dispatchMessages.at(-1)).toContain("Start one run from the framed protocol.");

      const stopped = await server.handleLine(request("6", 1, "stop", { runRef: run.runRef }));
      expect(stopped?.ok).toBe(true);
      expect((stopped?.result as { run: { state: string } }).run.state).toBe("stopped");

      // list_runs stays redacted (no objective text).
      const listed = await server.handleLine(request("7", 1, "list_runs"));
      expect(JSON.stringify(listed)).not.toContain("framed protocol");
    });
  });

  test("a launcher turn cap of one reaches cap_reached after one accepted continuation", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      let dispatchedTurns = 0;
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: makeOmegaEffectdTestHost((hostRequest) => {
            if (hostRequest.method === "dispatch_turn") dispatchedTurns += 1;
          }),
        },
      );
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }));

      const started = await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "One turn only",
          objective: "Accept exactly one continuation.",
          doneCondition: "The configured continuation cap is reached.",
          turnCap: 1,
        }),
      );

      expect(started?.ok).toBe(true);
      expect(dispatchedTurns).toBe(1);
      expect((started?.result as { run: { turnCap: number; state: string } }).run).toMatchObject({
        turnCap: 1,
        state: "cap_reached",
      });

      const runRef = (started?.result as { run: { runRef: string } }).run.runRef;
      const detail = await server.handleLine(request("3", 1, "get_run", { runRef }));
      expect(
        (detail?.result as { run: { successfulAttempts: number; state: string } }).run,
      ).toMatchObject({ successfulAttempts: 1, state: "cap_reached" });
      expect(dispatchedTurns).toBe(1);
    });
  });

  test("stops a paused run without refreshing an unavailable provider lane", async () => {
    await withRoot(async (root) => {
      const baseHost = makeOmegaEffectdTestHost();
      let providerAvailable = true;
      const requestsAfterProviderLoss: string[] = [];
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: async (hostRequest) => {
            if (!providerAvailable) requestsAfterProviderLoss.push(hostRequest.method);
            if (hostRequest.method === "lane_readiness" && !providerAvailable) {
              return { known: true, admitted: true, fullAuto: true, state: "unavailable" };
            }
            if (hostRequest.method === "refresh_evidence" && !providerAvailable) {
              throw new Error("That provider lane has no verified authentication.");
            }
            return baseHost(hostRequest);
          },
        },
      );
      await server.handleLine(request("init-stop-unready", 0, "initialize", { generation: 1 }));

      const started = await server.handleLine(
        request("start-stop-unready", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "Stop without provider auth",
          objective: "Prove Stop remains a local owner control.",
          doneCondition: "The paused run reaches stopped after provider authentication is lost.",
        }),
      );
      expect(started?.ok).toBe(true);
      const runRef = (started?.result as { run: { runRef: string } }).run.runRef;

      const paused = await server.handleLine(request("pause-stop-unready", 1, "pause", { runRef }));
      expect(paused?.ok).toBe(true);
      expect((paused?.result as { run: { state: string } }).run.state).toMatch(/paus/);

      providerAvailable = false;
      const stopped = await server.handleLine(request("stop-stop-unready", 1, "stop", { runRef }));

      expect(stopped?.ok).toBe(true);
      expect((stopped?.result as { run: { state: string } }).run.state).toBe("stopped");
      expect(requestsAfterProviderLoss).not.toContain("refresh_evidence");
      expect(requestsAfterProviderLoss).not.toContain("lane_readiness");
      expect(requestsAfterProviderLoss).toContain("interrupt_turn");
    });
  });

  test("repairs an interrupted provider-thread registry transfer before reconciliation", async () => {
    await withRoot(async (root) => {
      const runsPath = resolveFullAutoRunsPath({ dataRoot: root });
      const threadsPath = resolveFullAutoRegistryPath({ dataRoot: root });
      const runRegistry = openFullAutoRunRegistry(runsPath);
      const threadRegistry = openFullAutoRegistry(threadsPath);
      const sourceThreadRef = "thread.omega.source";
      const targetThreadRef = "thread.omega.target";
      threadRegistry.set(sourceThreadRef, true, {
        workspaceRef: "workspace.omega.supervised",
        profile: { lane: "codex-local" },
      });
      const started = runRegistry.startNew({
        title: "Interrupted handoff",
        objective: "Recover the execution binding.",
        doneCondition: "The target provider thread owns the grant.",
        objectiveSource: "user",
        workspaceRef: "workspace.omega.supervised",
        threadRef: sourceThreadRef,
        profile: { lane: "codex-local" },
        actor: "control_api",
        reason: "test",
      });
      expect(started.ok).toBe(true);
      if (!started.ok) return;
      runRegistry.rebindExecution(started.run.runRef, {
        threadRef: targetThreadRef,
        profile: { lane: "claude-local" },
      });

      createOmegaEffectdFramedServer(
        createOmegaEffectdService({ paths: { dataRoot: root } }),
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );

      const recovered = openFullAutoRegistry(threadsPath);
      expect(recovered.record(sourceThreadRef)).toBeNull();
      expect(recovered.record(targetThreadRef)?.profile?.lane).toBe("claude-local");
    });
  });

  test("FA-04 capacity, guardrail immunity, missing-thread stall, and redacted attention", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );
      const init = await server.handleLine(request("1", 0, "initialize", { generation: 1 }));
      expect(init?.ok).toBe(true);
      expect((init?.result as { capabilities: string[] }).capabilities).toContain("get_capacity");
      expect((init?.result as { capabilities: string[] }).capabilities).toContain(
        "decide_attention",
      );

      const capacity = await server.handleLine(request("2", 1, "get_capacity"));
      expect(capacity?.ok).toBe(true);
      const cap = capacity?.result as {
        activeRunLimit: number;
        nonOverridableGuardrails: string[];
        ownerConfigurableGuardrails: string[];
        enabledThreadsNeverEvicted: boolean;
        lanes: Array<{ lane: string; state: string }>;
      };
      expect(cap.activeRunLimit).toBe(8);
      expect(cap.nonOverridableGuardrails).toEqual([
        "workspace_binding",
        "own_capacity_only",
        "no_rate_limit_reset_triggering",
      ]);
      expect(cap.ownerConfigurableGuardrails).toEqual([
        "maxWallClockMs",
        "maxTurns",
        "maxPerTurnFailures",
        "tokenBudgetRef",
      ]);
      expect(cap.enabledThreadsNeverEvicted).toBe(true);
      expect(cap.lanes.length).toBeGreaterThan(0);
      expect(cap.lanes.some((lane) => lane.lane === "codex-local")).toBe(true);

      const started = await server.handleLine(
        request("3", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-04 missing thread",
          objective: "SECRET_OBJECTIVE_SHOULD_NOT_LEAK_INTO_ATTENTION",
          doneCondition: "SECRET_DONE_CONDITION",
          turnCap: 8,
          guardrails: {
            maxTurns: 8,
            workspace_binding: false,
            own_capacity_only: false,
            no_rate_limit_reset_triggering: true,
          },
        }),
      );
      expect(started?.ok).toBe(true);
      const run = (started?.result as { run: { runRef: string; threadRef: string; state: string } })
        .run;
      expect(run.threadRef).toMatch(/^thread\.omega\./);

      // Drop the thread record on disk (simulates cache eviction / vanished host
      // thread). Enabled Full Auto records are never silently dropped by the
      // registry eviction policy — this is the falsifier class for FA-04.
      const registryPath = resolveFullAutoRegistryPath({ dataRoot: root });
      writeFileSync(
        registryPath,
        JSON.stringify(
          { schema: "openagents.desktop.full_auto_registry.v1", records: [] },
          null,
          2,
        ),
      );

      const serviceB = createOmegaEffectdService({ paths: { dataRoot: root } });
      const serverB = createOmegaEffectdFramedServer(
        serviceB,
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );
      await serverB.handleLine(request("10", 0, "initialize", { generation: 2 }));
      const detail = await serverB.handleLine(request("11", 2, "get_run", { runRef: run.runRef }));
      expect(detail?.ok).toBe(true);
      const stalled = (
        detail?.result as {
          run: {
            state: string;
            stallCause: string | null;
            recoveryAction: string;
            objective: string;
          };
        }
      ).run;
      expect(stalled.state).toBe("stalled");
      expect(stalled.stallCause).toBe("host_thread_missing");
      expect(stalled.recoveryAction).toBe("stop_only");
      expect(stalled.objective).toContain("SECRET_OBJECTIVE");

      const attention = await serverB.handleLine(
        request("12", 2, "decide_attention", {
          runRef: run.runRef,
          permissionGranted: true,
        }),
      );
      expect(attention?.ok).toBe(true);
      const note = (
        attention?.result as {
          attention: { notify: boolean; title: string; body: string; dedupKey: string } | null;
        }
      ).attention;
      expect(note).not.toBeNull();
      expect(note?.notify).toBe(true);
      expect(note?.title).toContain("stalled");
      expect(JSON.stringify(note)).not.toContain("SECRET_OBJECTIVE");
      expect(JSON.stringify(note)).not.toContain("SECRET_DONE");
    });
  });

  test("FA-05 report, receipt redaction, mobile intent, and Sync stub", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }));

      const started = await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-05 report",
          objective: "PRIVATE_OBJECTIVE_TEXT_MUST_NOT_ENTER_RECEIPT",
          doneCondition: "PRIVATE_DONE_CONDITION",
          turnCap: 6,
        }),
      );
      expect(started?.ok).toBe(true);
      const runRef = (started?.result as { run: { runRef: string } }).run.runRef;

      const report = await server.handleLine(request("3", 1, "get_report", { runRef }));
      expect(report?.ok).toBe(true);
      expect((report?.result as { report: { runRef: string } }).report.runRef).toBe(runRef);

      const receipt = await server.handleLine(request("4", 1, "get_receipt", { runRef }));
      expect(receipt?.ok).toBe(true);
      const publicReceipt = (receipt?.result as { receipt: Record<string, unknown> }).receipt;
      expect(typeof publicReceipt.objectiveDigest).toBe("string");
      expect(JSON.stringify(publicReceipt)).not.toContain("PRIVATE_OBJECTIVE");
      expect(JSON.stringify(publicReceipt)).not.toContain("PRIVATE_DONE");

      const paused = await server.handleLine(
        request("5", 1, "apply_control_intent", {
          intentId: "intent.pause.1",
          runRef,
          action: "pause",
        }),
      );
      expect(paused?.ok).toBe(true);
      expect((paused?.result as { outcome: { status: string } }).outcome.status).toBe("applied");

      const resumed = await server.handleLine(
        request("6", 1, "apply_control_intent", {
          intentId: "intent.resume.1",
          runRef,
          action: "resume",
        }),
      );
      expect((resumed?.result as { outcome: { status: string } }).outcome.status).toBe("applied");

      const sync = await server.handleLine(request("7", 1, "get_sync_status"));
      expect(sync?.ok).toBe(true);
      expect(
        (sync?.result as { available: boolean; publishBlocksDispatch: boolean }).available,
      ).toBe(false);
      expect((sync?.result as { publishBlocksDispatch: boolean }).publishBlocksDispatch).toBe(
        false,
      );

      const publish = await server.handleLine(request("8", 1, "publish_projection", { runRef }));
      expect(publish?.ok).toBe(true);
      expect((publish?.result as { status: string }).status).toBe("sync_unavailable");

      const stopped = await server.handleLine(
        request("9", 1, "apply_control_intent", {
          intentId: "intent.stop.1",
          runRef,
          action: "stop",
        }),
      );
      expect(
        (stopped?.result as { outcome: { status: string; resultLifecycleState?: string } }).outcome
          .status,
      ).toBe("applied");
    });
  });

  test("FA-06 native project/worktree binding and boundary assessment", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }));

      const refused = await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "unsafe",
          objective: "Should refuse rebase-unsafe starts.",
          doneCondition: "Start is refused.",
          projectRef: "project.1",
          worktreeRef: "worktree.1",
          rebaseUnsafe: true,
        }),
      );
      expect(refused?.ok).toBe(false);
      expect(refused?.error?.message).toContain("rebase_unsafe");

      const started = await server.handleLine(
        request("3", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-06 bind",
          objective: "Bind to native project truth.",
          doneCondition: "Evidence refs exist.",
          projectRef: "project.99",
          worktreeRef: "worktree.12",
          worktreeAbsolutePath: "/tmp/omega-fa06-demo",
          gitHead: "deadbeef",
        }),
      );
      expect(started?.ok).toBe(true);
      const run = (
        started?.result as {
          run: {
            runRef: string;
            nativeEvidence: { projectRef: string; worktreeRef: string; gitHead: string | null };
          };
        }
      ).run;
      expect(run.nativeEvidence.projectRef).toBe("project.99");
      expect(run.nativeEvidence.worktreeRef).toBe("worktree.12");
      expect(run.nativeEvidence.gitHead).toBe("deadbeef");

      const binding = await server.handleLine(
        request("4", 1, "get_native_binding", { runRef: run.runRef }),
      );
      expect(binding?.ok).toBe(true);
      expect((binding?.result as { binding: { projectRef: string } }).binding.projectRef).toBe(
        "project.99",
      );

      const assess = await server.handleLine(
        request("5", 1, "assess_native_boundary", { runRef: run.runRef }),
      );
      expect(assess?.ok).toBe(true);
      expect((assess?.result as { assessment: { ok: boolean } }).assessment.ok).toBe(true);
    });
  });

  /**
   * OMEGA-MOB-31-03 (omega#47). The mobile Full Auto adjunct projects the EXACT
   * unattended duration of a run, and refuses a run whose start this host never
   * recorded rather than reporting a zero. That refusal is only survivable if a
   * live host actually emits a numeric start -- which, before this, it did not:
   * `get_run` carried a formatted `updatedAt` and nothing else, so every live
   * run was refused. These assertions hold the wire end of it.
   */
  test("a live run reports its host-recorded numeric start, not just a formatted timestamp", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }));

      const before = Date.now();
      const started = await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "issue47 numeric start",
          objective: "Project the exact unattended duration of a live run.",
          doneCondition: "The phone measures the duration instead of parsing it.",
        }),
      );
      expect(started?.ok).toBe(true);
      const after = Date.now();
      const runRef = (started?.result as { run: { runRef: string } }).run.runRef;

      const detail = await server.handleLine(request("3", 1, "get_run", { runRef }));
      const run = (
        detail?.result as {
          run: { startedAtMs: number | null; updatedAt: string };
        }
      ).run;

      // Numeric, and taken from THIS host's clock while the request was in
      // flight -- not a client value and not a parse of anything.
      expect(typeof run.startedAtMs).toBe("number");
      expect(run.startedAtMs).toBeGreaterThanOrEqual(before);
      expect(run.startedAtMs).toBeLessThanOrEqual(after);

      // `updatedAt` remains display text. Nothing downstream needs to parse it
      // to know how long the run has been unattended.
      expect(typeof run.updatedAt).toBe("string");

      // `list_runs` agrees with `get_run`, so the monitor and the phone cannot
      // hold two opinions about when a run began.
      const listed = await server.handleLine(request("4", 1, "list_runs"));
      const snapshot = (
        listed?.result as {
          runs: ReadonlyArray<{ runRef: string; startedAtMs: number | null }>;
        }
      ).runs.find((entry) => entry.runRef === runRef);
      expect(snapshot?.startedAtMs).toBe(run.startedAtMs);

      // A Pause and Resume is not a new start. Pausing a run overnight must not
      // erase the unattended hours the owner is asking about.
      await server.handleLine(request("5", 1, "pause", { runRef }));
      await server.handleLine(request("6", 1, "resume", { runRef }));
      const resumed = await server.handleLine(request("7", 1, "get_run", { runRef }));
      expect((resumed?.result as { run: { startedAtMs: number | null } }).run.startedAtMs).toBe(
        run.startedAtMs,
      );

      // And it survives a restart, because it is durable rather than derived.
      const serviceB = createOmegaEffectdService({ paths: { dataRoot: root } });
      const serverB = createOmegaEffectdFramedServer(
        serviceB,
        { dataRoot: root },
        { hostRequestHandler: makeOmegaEffectdTestHost() },
      );
      await serverB.handleLine(request("20", 0, "initialize", { generation: 2 }));
      const afterRestart = await serverB.handleLine(request("21", 2, "get_run", { runRef }));
      expect(
        (afterRestart?.result as { run: { startedAtMs: number | null } }).run.startedAtMs,
      ).toBe(run.startedAtMs);
    });
  });

  // OMEGA-MOB-31-03 (omega#47) / OMEGA-FA-10 (omega#43): one finished unit,
  // through the framed protocol, with every hop a host measurement.
  test("a host-verified completion publishes an evidence chain no client could assert", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const turns: Array<Record<string, unknown>> = [];
      let dispatched = false;
      const head = "4f2b8c1d9e0a7b6c5d4e3f2a1b0c9d8e7f6a5b4c";
      const base = "0123456789abcdef0123456789abcdef01234567";
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: async (hostRequest) => {
            const params = hostRequest.params as Record<string, unknown>;
            switch (hostRequest.method) {
              case "resolve_workspace":
                return { workspaceRef: params.expectedWorkspaceRef ?? "/tmp/ws" };
              case "lane_readiness":
                return { known: true, admitted: true, fullAuto: true, state: "available" };
              case "create_thread":
                return { threadRef: "thread.omega.1" };
              case "dispatch_turn": {
                if (!dispatched) {
                  dispatched = true;
                  turns.push({
                    schema: "openagents.desktop.local_turn_record.v1",
                    threadRef: "thread.omega.1",
                    turnRef: "turn.full-auto.1",
                    lane: "codex-local",
                    userMessageKey: "msg.user.1",
                    assistantMessageKey: "msg.assistant.1",
                    accountRef: null,
                    providerSessionRef: null,
                    model: null,
                    phase: "completed",
                    persistedCursor: 1,
                    // The turn SELF-REPORTS. That is a request for the host to
                    // look, never a completion.
                    assistantText: "landed it\n\nFULL-AUTO-COMPLETE\n",
                    assistantSegments: [],
                    recoveryGeneration: 0,
                    disposition: "completed",
                    createdAt: "2026-07-25T18:31:14.000Z",
                    updatedAt: "2026-07-25T18:31:14.000Z",
                  });
                }
                return { accepted: true };
              }
              case "refresh_evidence":
                return { present: true, revision: turns.length, live: null, turns };
              case "append_system_note":
                return { appended: true };
              default:
                return {};
            }
          },
          // The host's own child process, and the host's own Git reader.
          verificationExec: async () => ({ exitCode: 0, stdout: "ok" }),
          workspaceProbe: async ({ baselineRef }) => ({
            headRef: baselineRef === undefined ? base : head,
            generation: baselineRef === undefined ? 6 : 7,
            diffShortstat: baselineRef === undefined ? "" : "2 files changed, 3 insertions(+)",
          }),
        },
      );
      await server.handleLine(request("1", 0, "initialize", { generation: 1 }));

      const started = await server.handleLine(
        request("2", 1, "start", {
          workspaceRef: "/tmp/ws",
          title: "finished unit",
          objective: "Land one finished unit.",
          doneCondition: "It is landed.\nverify: pnpm test",
          autonomy: true,
          // A client asserting its own evidence. None of this may survive.
          evidence: {
            objectiveRef: "objective.client.forged",
            turnRef: "turn.client.forged",
            changeRef: "change.client.forged",
            projectGeneration: "generation.project.99999",
            verificationRef: "verification.client.forged",
            testOutcome: "outcome.test.passed",
            testCommand: "true",
            diffSummary: "999 files changed",
            hostExecuted: true,
          },
          decisionRef: "decision.client.forged",
          authorityReceiptRef: "receipt.client.forged",
        }),
      );
      expect(started?.ok).toBe(true);
      const runRef = (started?.result as { run: { runRef: string } }).run.runRef;

      // Pause/Resume forces the second reconciliation pass: the one that
      // re-reads the journal, sees the self-report, and runs the host check.
      await server.handleLine(request("3", 1, "pause", { runRef }));
      await server.handleLine(request("4", 1, "resume", { runRef }));
      for (let attempt = 0; attempt < 50; attempt += 1) {
        const polled = await server.handleLine(
          request(`poll-${attempt}`, 1, "get_run", { runRef }),
        );
        if ((polled?.result as { run: { state: string } }).run.state === "completed") break;
        await new Promise((resolve) => setTimeout(resolve, 20));
      }

      const detail = (
        (await server.handleLine(request("5", 1, "get_run", { runRef })))?.result as {
          run: { state: string; terminalReasonRef: string | null };
        }
      ).run;
      expect(detail.state).toBe("completed");
      // A finished run names WHY it finished in typed form, from its own state
      // and the actor that ended it -- never by reading its prose explanation.
      expect(detail.terminalReasonRef).toBe("terminal.full_auto.completed.control_api");

      const report = (
        (await server.handleLine(request("6", 1, "get_report", { runRef })))?.result as {
          report: Record<string, unknown>;
        }
      ).report;
      const receipt = (
        (await server.handleLine(request("7", 1, "get_receipt", { runRef })))?.result as {
          receipt: Record<string, unknown>;
        }
      ).receipt;

      const evidence = report.evidence as Record<string, unknown>;
      expect(evidence.hostExecuted).toBe(true);
      expect(evidence.turnRef).toBe("turn.full-auto.1");
      expect(evidence.changeRef).toBe(`change.${head}`);
      expect(evidence.projectGeneration).toBe("generation.project.00007");
      expect(evidence.diffSummary).toBe(
        `since ${base.slice(0, 7)}: 2 files changed, 3 insertions(+)`,
      );
      expect(evidence.testCommand).toBe("pnpm test");
      expect(evidence.objectiveRef).toBe(`objective.${report.objectiveDigest as string}`);

      // The receipt carries the authority hops and agrees with the report on
      // every hop they share, because both are projected from one record.
      expect(receipt.authorityRef).toBe("authority.omega.host.full_auto_completion");
      expect(receipt.allowed).toBe(true);
      for (const field of ["objectiveRef", "turnRef", "changeRef", "verificationRef"]) {
        expect(receipt[field]).toBe(evidence[field]);
      }

      // Nothing the client sent reached either record.
      const published = JSON.stringify({ report, receipt, detail });
      for (const forged of ["client.forged", "generation.project.99999", "999 files changed"]) {
        expect(published).not.toContain(forged);
      }
    });
  });
});
