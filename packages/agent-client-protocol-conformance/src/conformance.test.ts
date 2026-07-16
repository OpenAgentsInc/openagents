import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { decodeStableAcpMethodPayload } from "@openagentsinc/agent-client-protocol/stable";
import { AcpSessionRuntime } from "@openagentsinc/agent-client-runtime-bridge/session-runtime";
import { describe, expect, it } from "vite-plus/test";

import {
  buildCompatibilityMatrix,
  buildCoverageReport,
  buildFaultMatrix,
  FAULT_CASES,
} from "./artifacts.ts";
import {
  UnadvertisedPeerMethodError,
  capabilityGuardedReverseHandler,
  evaluateCapability,
  peerNegotiationFromResponses,
  requireNegotiatedPeerMethod,
} from "./capabilities.ts";
import { STABLE_CONFORMANCE_CASES, assertStableManifestCoverage } from "./cases.ts";
import { executeFaultCase } from "./faults.ts";
import { definePeerScenario, runPeerScenario, startPeerScenarioTransport } from "./harness.ts";
import { summarizeSafeInitialize } from "./live.ts";
import {
  buildAcpLiveReleaseArtifact,
  validateAcpDesktopReleaseArtifact,
  validateAcpLiveReleaseArtifact,
} from "./live-release.ts";
import { materializeMcpServers, McpReferenceError } from "./mcp.ts";
import {
  ConformanceProjectionState,
  projectSessionUpdateForConformance,
  projectStopReasonForConformance,
} from "./projection.ts";
import { acpReleaseEvidenceClass, validateAcpReleaseMatrix } from "./release.ts";
import { assertSecretAbsent } from "./transcript.ts";
import {
  CONTENT_BLOCK_FIXTURES,
  SESSION_UPDATE_FIXTURES,
  STOP_REASONS,
  TOOL_CONTENT_FIXTURES,
  TOOL_KINDS,
  TOOL_STATUSES,
  observeSessionUpdate,
} from "./variants.ts";

describe("stable Agent Client Protocol conformance", () => {
  it("builds a closed, redacted candidate live-release receipt", () => {
    const artifact = buildAcpLiveReleaseArtifact({
      recordedAt: "2026-07-16T16:00:00.000Z",
      openAgentsRevision: "a".repeat(40),
      platform: "darwin-arm64-node-24.0.0",
      peers: [
        {
          peer: "grok",
          result: "partial",
          binary: { reportedVersion: "0.2.101", executableSha256: "b".repeat(64) },
          negotiation: {
            wireVersion: 1,
            authMethodIds: ["cached_token"],
            capabilityKeys: ["list"],
          },
          scenarios: [
            { id: "initialize", result: "live-pass", safeDetail: "Initialize completed" },
          ],
          counters: {
            updateCount: 2,
            updateKinds: ["agent_message_chunk"],
            promptCount: 1,
          },
        },
      ],
    });

    expect(validateAcpLiveReleaseArtifact(artifact)).toEqual({ valid: true, errors: [] });
    expect(JSON.stringify(artifact)).not.toMatch(/prompt text|response text|session-[0-9]/i);

    const leaking = structuredClone(artifact) as unknown as {
      peers: Array<{ scenarios: Array<{ safeDetail: string }> }>;
    };
    leaking.peers[0]!.scenarios[0]!.safeDetail = "/Users/example/private";
    expect(validateAcpLiveReleaseArtifact(leaking as never)).toMatchObject({ valid: false });

    const invented = structuredClone(artifact) as unknown as {
      peers: Array<{ scenarios: Array<{ id: string }> }>;
    };
    invented.peers[0]!.scenarios[0]!.id = "invented-release-proof";
    expect(validateAcpLiveReleaseArtifact(invented as never)).toMatchObject({ valid: false });
  });

  it("validates packaged Desktop interruption and recovery without accepting retained data", () => {
    const artifact = {
      format: "openagents-acp-desktop-release-run-v1",
      protocol: "Agent Client Protocol",
      protocolExclusions: ["Agent Communication Protocol", "A2A"],
      proofClass: "candidate-packaged-desktop-live",
      claimAuthority: "none-release-matrix-only",
      recordedAt: "2026-07-16T16:46:36.674Z",
      openAgentsRevision: "a".repeat(40),
      platform: "darwin-arm64-node-24.13.1",
      provider: "cursor",
      lane: "acp:cursor-agent",
      packaged: true,
      interruption: {
        mismatchedWorkspaceRefused: true,
        laneConfigured: true,
        laneAdmitted: true,
        exitedDuringRunningTurn: true,
      },
      recovery: {
        reusedDesktopState: true,
        explicitlyReenabledSameThread: true,
        recoveredSameThread: true,
        freshThreadRetryAfterFailure: false,
        laneConfigured: true,
        interruptedTurnSettled: true,
        durableCompletedTurn: true,
        disabled: true,
      },
      redaction: {
        promptTextRetained: false,
        responseTextRetained: false,
        threadIdentifiersRetained: false,
        authMaterialRetained: false,
        absolutePathsRetained: false,
      },
    } as const;
    expect(validateAcpDesktopReleaseArtifact(artifact)).toEqual({ valid: true, errors: [] });
    const leaking = structuredClone(artifact) as unknown as {
      redaction: { promptTextRetained: boolean };
    };
    leaking.redaction.promptTextRetained = true;
    expect(validateAcpDesktopReleaseArtifact(leaking as never)).toMatchObject({ valid: false });
  });

  it("reduces live initialize evidence to capability/auth IDs without provider or host metadata", () => {
    const summary = summarizeSafeInitialize({
      protocolVersion: 1,
      agentCapabilities: {
        loadSession: true,
        auth: { logout: {}, _meta: { token: "secret-token" } },
        sessionCapabilities: { resume: {} },
        _meta: { hostname: "secret-host", agentInstanceId: "secret-id" },
      },
      authMethods: [{ id: "cached_token", description: "from /private/home" }],
      _meta: { currentWorkingDirectory: "/private/workspace", agentId: "secret-id" },
    });
    expect(summary).toEqual({
      protocolVersion: 1,
      advertisedCapabilityKeys: ["auth", "loadSession", "sessionCapabilities"],
      advertisedAuthCapabilityKeys: ["logout"],
      advertisedSessionCapabilityKeys: ["resume"],
      authMethodIds: ["cached_token"],
    });
    expect(JSON.stringify(summary)).not.toMatch(/secret|private|hostname|agentInstanceId/);
  });
  it("executes replay-before-response and response-before-next-read lifecycle barriers", async () => {
    const scenario = definePeerScenario({
      name: "session-runtime-race-matrix",
      actions: [
        {
          method: "initialize",
          result: {
            protocolVersion: 1,
            agentInfo: { name: "fixture", version: "1.0.0" },
            agentCapabilities: { loadSession: true },
            authMethods: [],
          },
        },
        {
          method: "session/load",
          notifications: [
            {
              method: "session/update",
              params: {
                sessionId: "existing",
                update: {
                  sessionUpdate: "agent_message_chunk",
                  content: { type: "text", text: "replay" },
                },
              },
            },
          ],
          result: {},
        },
        {
          method: "session/prompt",
          result: { stopReason: "end_turn" },
          afterResponseTurns: 1,
          notificationsAfterResponse: [
            {
              method: "session/update",
              params: {
                sessionId: "existing",
                update: {
                  sessionUpdate: "agent_message_chunk",
                  content: { type: "text", text: "tail" },
                },
              },
            },
          ],
        },
      ],
    });
    const transport = await startPeerScenarioTransport(scenario);
    const updates: Array<{ phase: string; disposition: string }> = [];
    const runtime = new AcpSessionRuntime({
      profile: "standard",
      createTransport: async () => transport,
      clientCapabilities: {
        fs: { readTextFile: false, writeTextFile: false },
        terminal: false,
      },
      onUpdate(update) {
        updates.push({ phase: update.phase, disposition: update.disposition });
      },
    });
    expect((await runtime.start()).ok).toBe(true);
    expect(
      await runtime.loadSession({
        cwd: "/workspace",
        canonicalThreadSeed: "fixture",
        peerSessionId: "existing",
      }),
    ).toMatchObject({ ok: true, value: { phase: "live" } });
    expect(updates).toMatchObject([{ phase: "replay", disposition: "applied" }]);
    expect(await runtime.prompt("existing", [{ type: "text", text: "hello" }])).toMatchObject({
      ok: true,
      value: { terminal: "completed" },
    });
    expect(updates).toMatchObject([
      { phase: "replay", disposition: "applied" },
      { phase: "live", disposition: "applied" },
    ]);
    await runtime.shutdown();
  });
  it("pins Grok replay-load and Cursor resume/config lifecycle fixtures", () => {
    const fixtures = [
      "fixtures/peers/grok/source-c68e39f/replay-load.json",
      "fixtures/peers/cursor/t3-bde0a4c0/resume-config.json",
    ].map(
      (path) =>
        JSON.parse(readFileSync(resolve(import.meta.dirname, "..", path), "utf8")) as Record<
          string,
          unknown
        >,
    );
    expect(fixtures.map((fixture) => fixture.method)).toEqual(["session/load", "session/resume"]);
    for (const fixture of fixtures) {
      const method = String(fixture.method);
      expect(
        decodeStableAcpMethodPayload({
          direction: "client-to-agent",
          method,
          phase: "params",
          payload: fixture.request,
        }),
      ).toMatchObject({ _tag: "Decoded" });
      expect(
        decodeStableAcpMethodPayload({
          direction: "client-to-agent",
          method,
          phase: "result",
          payload: fixture.response,
        }),
      ).toMatchObject({ _tag: "Decoded" });
      for (const update of fixture.updatesBeforeResponse as unknown[]) {
        expect(
          decodeStableAcpMethodPayload({
            direction: "agent-to-client",
            method: "session/update",
            phase: "params",
            payload: update,
          }),
        ).toMatchObject({ _tag: "Decoded" });
      }
      expect(fixture.expected).toMatchObject({
        phaseBeforeBarrier: "replay",
        phaseAfterBarrier: "live",
      });
    }
  });
  it("has one explicit support case for every pinned stable manifest member", () => {
    expect(assertStableManifestCoverage()).toEqual({ covered: 23, manifest: 23 });
    for (const value of STABLE_CONFORMANCE_CASES) {
      expect(
        decodeStableAcpMethodPayload({
          direction: value.direction,
          method: value.method,
          phase: "params",
          payload: value.params,
        }),
      ).toMatchObject({ _tag: "Decoded" });
      if (value.kind === "request")
        expect(
          decodeStableAcpMethodPayload({
            direction: value.direction,
            method: value.method,
            phase: "result",
            payload: value.result,
          }),
        ).toMatchObject({ _tag: "Decoded" });
    }
  });

  it.each([
    "filesystem",
    "terminal",
    "auth",
    "modes",
    "configuration",
    "session-lifecycle",
  ] as const)("makes %s present, absent, and violation behavior explicit", (family) => {
    expect(evaluateCapability({ family, advertised: true, peerInvoked: true })).toBe("allowed");
    expect(evaluateCapability({ family, advertised: false, peerInvoked: false })).toBe(
      "unsupported",
    );
    expect(evaluateCapability({ family, advertised: false, peerInvoked: true })).toBe(
      "peer-violation",
    );
  });

  it("enforces advertised and absent filesystem/terminal capability on actual reverse requests", async () => {
    for (const method of ["fs/read_text_file", "terminal/create"]) {
      for (const advertised of [true, false]) {
        let brokerInvocations = 0;
        const params =
          method === "fs/read_text_file"
            ? { sessionId: "s", path: "/workspace/a.txt" }
            : { sessionId: "s", command: "printf" };
        const result = await runPeerScenario(
          definePeerScenario({
            name: advertised ? "capability-present" : "capability-lie",
            actions: [
              {
                method: "initialize",
                result: { protocolVersion: 1, agentCapabilities: {}, authMethods: [] },
                reverseRequests: [{ method, params }],
                ignoreReverseErrors: true,
              },
            ],
          }),
          [
            {
              method: "initialize",
              params: {
                protocolVersion: 1,
                clientCapabilities: {
                  fs: {
                    readTextFile: method === "fs/read_text_file" && advertised,
                    writeTextFile: false,
                  },
                  terminal: method === "terminal/create" && advertised,
                },
              },
            },
          ],
          {
            [method]: capabilityGuardedReverseHandler(
              method,
              {
                fs: {
                  readTextFile: method === "fs/read_text_file" && advertised,
                  writeTextFile: false,
                },
                terminal: method === "terminal/create" && advertised,
              },
              () => {
                brokerInvocations += 1;
                return method === "fs/read_text_file"
                  ? { content: "fixture" }
                  : { terminalId: "t" };
              },
            ),
          },
        );
        expect(result.receipt.counters.reverseRequests).toBe(1);
        expect(brokerInvocations).toBe(advertised ? 1 : 0);
      }
    }
  });

  it("derives auth, modes, configuration, and lifecycle gates from actual peer advertisements", async () => {
    const presentInitialize = {
      protocolVersion: 1,
      authMethods: [{ id: "cached_token", name: "Cached" }],
      agentCapabilities: { sessionCapabilities: { list: {} } },
    };
    const presentSession = {
      sessionId: "s",
      modes: { currentModeId: "agent", availableModes: [{ id: "agent", name: "Agent" }] },
      configOptions: [{ id: "enabled", name: "Enabled", type: "boolean", currentValue: true }],
    };
    const capture = async (initialize: unknown, session: unknown) => {
      const result = await runPeerScenario(
        definePeerScenario({
          name: "advertisement-capture",
          actions: [
            { method: "initialize", result: initialize },
            { method: "session/new", result: session },
          ],
        }),
        [
          { method: "initialize", params: { protocolVersion: 1 } },
          { method: "session/new", params: { cwd: "/workspace", mcpServers: [] } },
        ],
      );
      return peerNegotiationFromResponses(result.results[0], result.results[1]);
    };
    const advertised = await capture(presentInitialize, presentSession);
    const absent = await capture(
      { protocolVersion: 1, authMethods: [], agentCapabilities: {} },
      { sessionId: "s" },
    );
    for (const method of [
      "authenticate",
      "session/set_mode",
      "session/set_config_option",
      "session/list",
    ]) {
      expect(() => requireNegotiatedPeerMethod(method, advertised)).not.toThrow();
      expect(() => requireNegotiatedPeerMethod(method, absent)).toThrow(
        new UnadvertisedPeerMethodError(method),
      );
      const value = STABLE_CONFORMANCE_CASES.find(
        (candidate) => candidate.direction === "client-to-agent" && candidate.method === method,
      );
      if (value === undefined) throw new Error(`missing ${method}`);
      const result = await runPeerScenario(
        definePeerScenario({
          name: `advertised-${method}`,
          actions: [{ method, result: value.result }],
        }),
        [{ method, params: value.params }],
      );
      expect(result.receipt.counters.requestsCompleted).toBe(1);
    }
  });

  it("runs all stable client methods and all reverse methods through the production transport", async () => {
    const outbound = STABLE_CONFORMANCE_CASES.filter(
      (value) => value.direction !== "agent-to-client",
    );
    const reverse = STABLE_CONFORMANCE_CASES.filter(
      (value) => value.direction === "agent-to-client",
    );
    const scenario = definePeerScenario({
      name: "stable-surface",
      actions: outbound.map((value, index) => ({
        method: value.method,
        result: value.result,
        ...(index % 2 === 0 ? { fragmentBytes: 7 } : {}),
        ...(value.method === "initialize"
          ? {
              notifications: reverse
                .filter((item) => item.kind === "notification")
                .map((item) => ({ method: item.method, params: item.params })),
              reverseRequests: reverse
                .filter((item) => item.kind === "request")
                .map((item) => ({ method: item.method, params: item.params })),
            }
          : {}),
      })),
    });
    const reverseHandlers = Object.fromEntries(
      reverse
        .filter((value) => value.kind === "request")
        .map((value) => [value.method, () => value.result]),
    );
    const result = await runPeerScenario(
      scenario,
      outbound.map((value) => ({ method: value.method, params: value.params, kind: value.kind })),
      reverseHandlers,
    );
    expect(result.receipt.counters.requestsCompleted).toBe(
      outbound.filter((value) => value.kind === "request").length,
    );
    expect(result.receipt.counters.reverseRequests).toBe(
      reverse.filter((value) => value.kind === "request").length,
    );
    expect(result.notifications.map((value) => value.method)).toContain("session/update");
    expect(new Set(result.transcript.map((value) => value.generation)).size).toBe(1);
  });

  it("correlates concurrent requests across more than one logical session", async () => {
    const scenario = definePeerScenario({
      name: "multi-session",
      actions: [{ method: "session/prompt", result: { stopReason: "end_turn" }, fragmentBytes: 3 }],
    });
    const result = await runPeerScenario(
      scenario,
      ["a", "b", "c", "d"].map((sessionId) => ({
        method: "session/prompt",
        params: { sessionId, prompt: [{ type: "text", text: "fixture" }] },
      })),
    );
    expect(result.results).toEqual(Array.from({ length: 4 }, () => ({ stopReason: "end_turn" })));
    expect(result.receipt.counters.peakInFlight).toBeGreaterThan(1);
  });

  it("observes lossless native envelopes transiently and serializes timestamped redacted lifecycle evidence", async () => {
    const privateCanary = "prompt-private-canary";
    let lossless = false;
    const result = await runPeerScenario(
      definePeerScenario({
        name: "transcript-evidence",
        actions: [
          {
            method: "session/prompt",
            result: { stopReason: "end_turn" },
            stderr: "token=stderr-secret-canary\n",
          },
        ],
      }),
      [
        {
          method: "session/prompt",
          params: {
            sessionId: "s",
            prompt: [{ type: "text", text: privateCanary }],
          },
        },
      ],
      {},
      {
        onPrivateNative(rows) {
          lossless = JSON.stringify(rows).includes(privateCanary);
        },
      },
    );
    expect(lossless).toBe(true);
    expect(result.receipt.state).toBe("disposed");
    expect(result.transcript.some((row) => row.direction === "stderr")).toBe(true);
    expect(result.transcript.some((row) => row.direction === "lifecycle")).toBe(true);
    expect(
      result.transcript.every(
        (row, index, rows) => index === 0 || row.atMs >= rows[index - 1]!.atMs,
      ),
    ).toBe(true);
    assertSecretAbsent(result, [privateCanary, "stderr-secret-canary"]);
  });
});

describe("variant retention and fixture authority", () => {
  it("covers every stable content, stop, update, tool status, kind, and content variant", () => {
    for (const content of CONTENT_BLOCK_FIXTURES) {
      expect(
        decodeStableAcpMethodPayload({
          direction: "client-to-agent",
          method: "session/prompt",
          phase: "params",
          payload: { sessionId: "s", prompt: [content] },
        }),
      ).toMatchObject({ _tag: "Decoded" });
      expect(
        projectSessionUpdateForConformance({
          sessionUpdate: "agent_message_chunk",
          content,
        }),
      ).toMatchObject({ kind: "message-delta", payload: { content } });
    }
    for (const stopReason of STOP_REASONS) {
      expect(
        decodeStableAcpMethodPayload({
          direction: "client-to-agent",
          method: "session/prompt",
          phase: "result",
          payload: { stopReason },
        }),
      ).toMatchObject({ _tag: "Decoded" });
      expect(projectStopReasonForConformance(stopReason)).toMatchObject({
        kind: "turn-stop",
        payload: { reason: stopReason },
      });
    }
    for (const update of SESSION_UPDATE_FIXTURES) {
      expect(observeSessionUpdate(update)).toMatchObject({
        classification: "known",
        discriminator: update.sessionUpdate,
        native: { update },
      });
      expect(projectSessionUpdateForConformance(update).kind).not.toBe("degraded");
    }
    for (const status of TOOL_STATUSES) {
      expect(
        observeSessionUpdate({ sessionUpdate: "tool_call_update", toolCallId: "t", status }),
      ).toMatchObject({ classification: "known", native: { update: { status } } });
      expect(
        projectSessionUpdateForConformance({
          sessionUpdate: "tool_call_update",
          toolCallId: "t",
          status,
        }),
      ).toMatchObject({ kind: "tool-call-update", payload: { status } });
    }
    for (const kind of TOOL_KINDS) {
      expect(
        observeSessionUpdate({
          sessionUpdate: "tool_call",
          toolCallId: "t",
          title: "fixture",
          kind,
          status: "pending",
        }),
      ).toMatchObject({ classification: "known", native: { update: { kind } } });
      expect(
        projectSessionUpdateForConformance({
          sessionUpdate: "tool_call",
          toolCallId: "t",
          title: "fixture",
          kind,
          status: "pending",
        }),
      ).toMatchObject({ kind: "tool-call", payload: { kind } });
    }
    for (const content of TOOL_CONTENT_FIXTURES) {
      expect(
        observeSessionUpdate({
          sessionUpdate: "tool_call_update",
          toolCallId: "t",
          content: [content],
        }),
      ).toMatchObject({ classification: "known", native: { update: { content: [content] } } });
      expect(
        projectSessionUpdateForConformance({
          sessionUpdate: "tool_call_update",
          toolCallId: "t",
          content: [content],
        }),
      ).toMatchObject({ kind: "tool-call-update", payload: { content: [content] } });
    }
  });

  it("retains an unknown future update privately without classifying it as stable", () => {
    const unknown = { sessionUpdate: "future_peer_update", secret: "native-only" };
    const observed = observeSessionUpdate(unknown);
    expect(observed).toMatchObject({
      classification: "unknown",
      discriminator: "future_peer_update",
      native: { update: unknown },
    });
    expect(projectSessionUpdateForConformance(unknown)).toMatchObject({
      kind: "degraded",
      payload: {
        reason: "unknown-session-update",
        discriminator: "future_peer_update",
      },
    });
    expect(Object.isFrozen(observed.native)).toBe(true);
  });

  it("deduplicates and quarantines order, terminal-state, and generation violations", () => {
    const state = new ConformanceProjectionState();
    const base = {
      generation: 1,
      sessionId: "s",
      updateId: "1",
      sequence: 1,
      update: {
        sessionUpdate: "tool_call_update",
        toolCallId: "t",
        status: "completed",
      },
    } as const;
    expect(state.apply(base).outcome).toBe("applied");
    expect(state.apply(base).outcome).toBe("duplicate");
    expect(
      state.apply({
        ...base,
        updateId: "2",
        sequence: 2,
        update: { ...base.update, status: "pending" },
      }),
    ).toMatchObject({ outcome: "quarantined", reason: "tool-state-regression" });
    expect(state.apply({ ...base, generation: 2, updateId: "3", sequence: 1 })).toMatchObject({
      outcome: "applied",
    });
    expect(state.apply({ ...base, generation: 1, updateId: "4", sequence: 3 })).toMatchObject({
      outcome: "quarantined",
      reason: "old-generation",
    });
  });

  it("accepts every legal tool transition and quarantines every regression/terminal rewrite", () => {
    const legal = new Set([
      "pending>pending",
      "pending>in_progress",
      "pending>completed",
      "pending>failed",
      "in_progress>in_progress",
      "in_progress>completed",
      "in_progress>failed",
      "completed>completed",
      "failed>failed",
    ]);
    for (const from of TOOL_STATUSES) {
      for (const to of TOOL_STATUSES) {
        const state = new ConformanceProjectionState();
        expect(
          state.apply({
            generation: 1,
            sessionId: "s",
            updateId: "1",
            sequence: 1,
            update: {
              sessionUpdate: "tool_call_update",
              toolCallId: "t",
              status: from,
            },
          }).outcome,
        ).toBe("applied");
        const next = state.apply({
          generation: 1,
          sessionId: "s",
          updateId: "2",
          sequence: 2,
          update: { sessionUpdate: "tool_call_update", toolCallId: "t", status: to },
        });
        if (legal.has(`${from}>${to}`)) expect(next.outcome).toBe("applied");
        else
          expect(next).toMatchObject({
            outcome: "quarantined",
            reason: "tool-state-regression",
          });
      }
    }
  });

  it.each(["grok/source-c68e39f", "cursor/t3-bde0a4c0"])(
    "keeps independently versioned %s provenance non-authoritative",
    (fixture) => {
      const profile = JSON.parse(
        readFileSync(
          resolve(import.meta.dirname, "../fixtures/peers", fixture, "profile.json"),
          "utf8",
        ),
      );
      expect(profile).toMatchObject({
        protocolVersion: 1,
        schemaRelease: "schema-v1.19.0",
        notACompatibilityClaim: true,
        captureProvenance: { sanitizerVersion: 1 },
      });
      expect(profile.proofClass).not.toBe("live-binary");
    },
  );
});

describe("MCP custody, durable evidence, and deterministic faults", () => {
  const secret = "mcp-secret-canary-never-durable";
  const server = {
    name: "brokered",
    type: "http" as const,
    url: "https://mcp.invalid/rpc",
    credentialRef: { id: "valid", expiresAt: "2030-01-01T00:00:00.000Z" },
  };

  it("materializes an authorized short-lived reference only in memory and redacts it immediately", async () => {
    const materialized = await materializeMcpServers([server], {
      now: new Date("2029-01-01T00:00:00Z"),
      resolve: ({ id }) => (id === "valid" ? secret : undefined),
    });
    const params = { cwd: "/workspace", mcpServers: materialized };
    let sawLosslessTransient = false;
    const result = await runPeerScenario(
      definePeerScenario({
        name: "authorized-mcp",
        actions: [
          {
            method: "session/new",
            result: { sessionId: "mcp-session" },
            expectParamsSha256: createHash("sha256").update(JSON.stringify(params)).digest("hex"),
          },
        ],
      }),
      [{ method: "session/new", params }],
      {},
      {
        onPrivateNative(rows) {
          sawLosslessTransient = JSON.stringify(rows).includes(secret);
        },
      },
    );
    expect(sawLosslessTransient).toBe(true);
    assertSecretAbsent(result, [secret]);
    expect(result.results).toEqual([{ sessionId: "mcp-session" }]);
    expect(JSON.stringify(result.transcript)).toContain("[REDACTED]");
    expect(JSON.stringify(server)).not.toContain(secret);
  });

  it("refuses invalid and expired references before a peer request", async () => {
    await expect(
      materializeMcpServers([server], {
        now: new Date("2029-01-01T00:00:00Z"),
        resolve: () => undefined,
      }),
    ).rejects.toEqual(new McpReferenceError("invalid"));
    await expect(
      materializeMcpServers(
        [{ ...server, credentialRef: { id: "expired", expiresAt: "2020-01-01T00:00:00Z" } }],
        { now: new Date("2029-01-01T00:00:00Z"), resolve: () => secret },
      ),
    ).rejects.toEqual(new McpReferenceError("expired"));
    await expect(
      materializeMcpServers([{ ...server, credentialRef: { id: "", expiresAt: "not-a-date" } }], {
        now: new Date("2029-01-01T00:00:00Z"),
        resolve: () => secret,
      }),
    ).rejects.toEqual(new McpReferenceError("invalid"));
    await expect(
      materializeMcpServers([server], {
        now: new Date("2029-01-01T00:00:00Z"),
        resolve: () => "",
      }),
    ).rejects.toEqual(new McpReferenceError("invalid"));
  });

  it("keeps generated coverage, compatibility, and bounded fault artifacts complete", () => {
    expect(buildCoverageReport()).toMatchObject({ covered: 23, manifest: 23 });
    expect(buildCompatibilityMatrix().rows).toHaveLength(23);
    expect(buildFaultMatrix()).toMatchObject({ bounded: true, timeoutMs: 2000 });
    expect(buildFaultMatrix().rows).toHaveLength(FAULT_CASES.length);
    expect(buildFaultMatrix().rows.every((row) => row.result === "declared-for-execution")).toBe(
      true,
    );
    expect(new Set(FAULT_CASES.map(([layer]) => layer))).toEqual(
      new Set([
        "parser",
        "protocol",
        "authority",
        "projection",
        "lifecycle",
        "transport",
        "recovery",
      ]),
    );
  });

  it("keeps named-peer release claims independent, fresh, redacted, and live-gated", () => {
    expect(acpReleaseEvidenceClass("grok", "auth-secondary")).toBe("optional-live-peer");
    expect(acpReleaseEvidenceClass("cursor", "auth-secondary")).toBe("not-applicable");
    const matrix = JSON.parse(
      readFileSync(resolve(import.meta.dirname, "../compatibility/release-matrix.json"), "utf8"),
    ) as Record<string, unknown>;
    expect(validateAcpReleaseMatrix(matrix, { now: new Date("2026-07-16T16:00:00.000Z") })).toEqual(
      { valid: true, errors: [] },
    );

    const promoted = structuredClone(matrix) as {
      peers: Array<{ claimState: string; releaseEligible: boolean }>;
    };
    promoted.peers[0]!.claimState = "supported";
    promoted.peers[0]!.releaseEligible = true;
    expect(
      validateAcpReleaseMatrix(promoted, { now: new Date("2026-07-16T16:00:00.000Z") }),
    ).toMatchObject({ valid: false });

    const hostLeaking = structuredClone(matrix) as Record<string, unknown>;
    hostLeaking["privatePath"] = "/Users/example/private-repository";
    expect(
      validateAcpReleaseMatrix(hostLeaking, { now: new Date("2026-07-16T16:00:00.000Z") }),
    ).toMatchObject({ valid: false });

    const identityMissing = structuredClone(matrix) as {
      openAgents: { schemaSha256?: string };
    };
    delete identityMissing.openAgents.schemaSha256;
    expect(
      validateAcpReleaseMatrix(identityMissing, { now: new Date("2026-07-16T16:00:00.000Z") }),
    ).toMatchObject({ valid: false });

    const scenarioMissing = structuredClone(matrix) as {
      peers: Array<{ scenarios: unknown[] }>;
    };
    scenarioMissing.peers[0]!.scenarios.pop();
    expect(
      validateAcpReleaseMatrix(scenarioMissing, { now: new Date("2026-07-16T16:00:00.000Z") }),
    ).toMatchObject({ valid: false });

    const selfExempted = structuredClone(matrix) as {
      peers: Array<{
        claimState: string;
        releaseEligible: boolean;
        scenarios: Array<{ requiredForSupported: boolean }>;
      }>;
    };
    for (const peer of selfExempted.peers) {
      peer.claimState = "supported";
      peer.releaseEligible = true;
      for (const scenario of peer.scenarios) scenario.requiredForSupported = false;
    }
    expect(
      validateAcpReleaseMatrix(selfExempted, { now: new Date("2026-07-16T16:00:00.000Z") }),
    ).toMatchObject({ valid: false });

    const inventedScenario = structuredClone(matrix) as {
      peers: Array<{ scenarios: Array<{ id: string }> }>;
    };
    inventedScenario.peers[0]!.scenarios[0]!.id = "self-authorized-promotion";
    expect(
      validateAcpReleaseMatrix(inventedScenario, { now: new Date("2026-07-16T16:00:00.000Z") }),
    ).toMatchObject({ valid: false });

    const emptyLiveEvidence = structuredClone(matrix) as {
      peers: Array<{ scenarios: Array<{ result: string; evidenceRefs: string[] }> }>;
    };
    const liveScenario = emptyLiveEvidence.peers[0]!.scenarios.find(
      (scenario) => scenario.result === "live-pass",
    )!;
    liveScenario.evidenceRefs = [];
    expect(
      validateAcpReleaseMatrix(emptyLiveEvidence, { now: new Date("2026-07-16T16:00:00.000Z") }),
    ).toMatchObject({ valid: false });

    const remoteEvidence = structuredClone(matrix) as {
      peers: Array<{ scenarios: Array<{ evidenceRefs: string[] }> }>;
    };
    remoteEvidence.peers[0]!.scenarios[0]!.evidenceRefs = ["https://example.invalid/proof.json"];
    expect(
      validateAcpReleaseMatrix(remoteEvidence, { now: new Date("2026-07-16T16:00:00.000Z") }),
    ).toMatchObject({ valid: false });

    const futureEvidence = structuredClone(matrix) as { recordedAt: string };
    futureEvidence.recordedAt = "2026-07-17T16:00:00.000Z";
    expect(
      validateAcpReleaseMatrix(futureEvidence, { now: new Date("2026-07-16T16:00:00.000Z") }),
    ).toMatchObject({ valid: false });

    const unsupportedPromotion = structuredClone(matrix) as {
      peers: Array<{
        claimState: string;
        releaseEligible: boolean;
        scenarios: Array<{ requiredForSupported: boolean; result: string }>;
      }>;
    };
    unsupportedPromotion.peers[0]!.claimState = "supported";
    unsupportedPromotion.peers[0]!.releaseEligible = true;
    for (const scenario of unsupportedPromotion.peers[0]!.scenarios)
      if (scenario.requiredForSupported) scenario.result = "unsupported";
    expect(
      validateAcpReleaseMatrix(unsupportedPromotion, {
        now: new Date("2026-07-16T16:00:00.000Z"),
      }),
    ).toMatchObject({ valid: false });
  });

  it.each(FAULT_CASES)("executes bounded %s/%s fault evidence", async (layer, fault) => {
    const result = await executeFaultCase(layer, fault);
    expect(result).toMatchObject({ layer, fault, result: "pass" });
    expect(result.boundedMs).toBeLessThanOrEqual(2_000);
    expect(result.oracle).not.toBe("execution-failed");
  });
});
