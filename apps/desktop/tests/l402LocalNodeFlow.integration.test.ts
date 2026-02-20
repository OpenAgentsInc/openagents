import fs from "node:fs";
import path from "node:path";

import {
  decodeL402ObservabilityRecordSync,
  L402ObservabilityFieldKeys,
  type L402ObservabilityRecord,
} from "@openagentsinc/lightning-effect";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "@effect/vitest";
import { afterAll } from "vitest";

import { AuthGatewayService } from "../src/effect/authGateway";
import { ConnectivityProbeService } from "../src/effect/connectivity";
import { DesktopAppService } from "../src/effect/app";
import { makeDesktopLayer } from "../src/effect/layer";
import { TaskProviderService } from "../src/effect/taskProvider";
import type {
  DesktopRuntimeState,
  ExecutorTask,
  ExecutorTaskRequest,
  ExecutorTaskStatus,
} from "../src/effect/model";

type TaskEvent = Readonly<{
  readonly taskId: string;
  readonly ownerId: string;
  readonly fromStatus?: ExecutorTaskStatus;
  readonly toStatus: ExecutorTaskStatus;
  readonly actor: "web_worker" | "desktop_executor" | "system";
  readonly reason?: string;
  readonly requestId?: string;
  readonly errorCode?: string;
  readonly errorMessage?: string;
  readonly metadata?: unknown;
  readonly createdAtMs: number;
}>;

type ApiCall = Readonly<{
  readonly method: string;
  readonly path: string;
  readonly requestId: string | null;
  readonly authorization: string | null;
  readonly body: unknown;
}>;

type SellerCall = Readonly<{
  readonly path: string;
  readonly authorization: string | null;
  readonly status: number;
}>;

type FlowArtifact = Readonly<{
  readonly flow: "success" | "blocked";
  readonly taskId: string;
  readonly taskStatus: ExecutorTaskStatus;
  readonly createRequestId: string;
  readonly transitionRequestIds: ReadonlyArray<string>;
  readonly proofReference?: string;
  readonly paymentId?: string | null;
  readonly blockedErrorCode?: string;
  readonly blockedReason?: string;
  readonly sellerCalls: ReadonlyArray<SellerCall>;
  readonly observabilityRecords: ReadonlyArray<L402ObservabilityRecord>;
}>;

const now = () => Date.now();

const toRequest = (input: RequestInfo | URL, init?: RequestInit): Request =>
  input instanceof Request && init === undefined ? input : new Request(input, init);

const jsonResponse = (body: unknown, status = 200, headers?: Record<string, string>): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(headers ?? {}),
    },
  });

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;

const asString = (value: unknown): string | undefined =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;

const asNumber = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined;

const taskStatuses: ReadonlyArray<ExecutorTaskStatus> = [
  "queued",
  "approved",
  "running",
  "paid",
  "cached",
  "blocked",
  "failed",
  "completed",
];

const transitionGraph: Readonly<Record<ExecutorTaskStatus, ReadonlyArray<ExecutorTaskStatus>>> = {
  queued: ["approved", "running", "blocked", "failed", "completed"],
  approved: ["running", "blocked", "failed", "completed"],
  running: ["paid", "cached", "blocked", "failed", "completed"],
  paid: ["completed", "failed", "cached"],
  cached: ["completed", "running", "failed"],
  blocked: ["queued", "failed"],
  failed: ["queued"],
  completed: [],
};

const canTransition = (from: ExecutorTaskStatus, to: ExecutorTaskStatus): boolean =>
  from === to || transitionGraph[from].includes(to);

const ensureStatus = (value: unknown): ExecutorTaskStatus => {
  if (typeof value !== "string" || !taskStatuses.includes(value as ExecutorTaskStatus)) {
    throw new Error("invalid_status");
  }
  return value as ExecutorTaskStatus;
};

const toWalletState = (
  snapshot: DesktopRuntimeState,
): L402ObservabilityRecord["walletState"] => {
  if (snapshot.wallet.recoveryState === "restore_ready" || snapshot.wallet.recoveryState === "restored") {
    return "recovering";
  }
  if (snapshot.wallet.walletState === "locked") return "locked";
  if (snapshot.wallet.walletState === "unlocked") return "unlocked";
  return "initializing";
};

const toNodeSyncStatus = (
  snapshot: DesktopRuntimeState,
): L402ObservabilityRecord["nodeSyncStatus"] => {
  if (snapshot.lnd.lifecycle !== "running") return "degraded";
  if (snapshot.lnd.health === "unhealthy") return "degraded";
  if (snapshot.lnd.sync.lastError) return "degraded";
  if (snapshot.lnd.sync.syncedToChain && snapshot.lnd.sync.walletSynced) return "synced";
  return "syncing";
};

const makeLocalObservabilityRecord = (input: {
  readonly requestId: string | null;
  readonly userId: string | null;
  readonly taskId: string | null;
  readonly endpoint: string | null;
  readonly quotedCostMsats: number | null;
  readonly capAppliedMsats: number | null;
  readonly paidAmountMsats: number | null;
  readonly paymentProofRef: string | null;
  readonly cacheHit: boolean | null;
  readonly denyReason: string | null;
  readonly executor: L402ObservabilityRecord["executor"];
  readonly plane: L402ObservabilityRecord["plane"];
  readonly desktopSessionId: string | null;
  readonly snapshot: DesktopRuntimeState;
}): L402ObservabilityRecord =>
  decodeL402ObservabilityRecordSync({
    requestId: input.requestId,
    userId: input.userId,
    paywallId: null,
    taskId: input.taskId,
    endpoint: input.endpoint,
    quotedCostMsats: input.quotedCostMsats,
    capAppliedMsats: input.capAppliedMsats,
    paidAmountMsats: input.paidAmountMsats,
    paymentProofRef: input.paymentProofRef,
    cacheHit: input.cacheHit,
    denyReason: input.denyReason,
    executor: input.executor,
    plane: input.plane,
    executionPath: "local-node",
    desktopSessionId: input.desktopSessionId,
    desktopRuntimeStatus:
      input.snapshot.lnd.lifecycle === "unavailable" ? "unavailable" : input.snapshot.lnd.lifecycle,
    walletState: toWalletState(input.snapshot),
    nodeSyncStatus: toNodeSyncStatus(input.snapshot),
    observedAtMs: now(),
  });

class LocalNodeFlowHarness {
  readonly openAgentsBaseUrl = "https://openagents.local";
  readonly sellerBaseUrl = "https://seller.local";
  readonly expectedToken = "token_local_node";
  readonly expectedUserId = "user_local_node";

  private taskCounter = 0;
  private readonly tasks: Array<ExecutorTask> = [];
  readonly events: Array<TaskEvent> = [];
  readonly apiCalls: Array<ApiCall> = [];
  readonly sellerCalls: Array<SellerCall> = [];

  private taskById(taskId: string): ExecutorTask {
    const found = this.tasks.find((task) => task.id === taskId);
    if (!found) throw new Error("task_not_found");
    return found;
  }

  private toTaskPayload(task: ExecutorTask): Record<string, unknown> {
    return {
      taskId: task.id,
      ownerId: task.ownerId,
      status: task.status,
      request: task.request,
      attemptCount: task.attemptCount,
      createdAtMs: task.createdAtMs,
      updatedAtMs: task.updatedAtMs,
      ...(task.idempotencyKey ? { idempotencyKey: task.idempotencyKey } : {}),
      ...(task.source ? { source: task.source } : {}),
      ...(task.requestId ? { requestId: task.requestId } : {}),
      ...(task.metadata !== undefined ? { metadata: task.metadata } : {}),
      ...(task.lastErrorCode ? { lastErrorCode: task.lastErrorCode } : {}),
      ...(task.lastErrorMessage ? { lastErrorMessage: task.lastErrorMessage } : {}),
      lastTransitionAtMs: task.updatedAtMs,
    };
  }

  private recordApiCall(request: Request, body: unknown): void {
    const url = new URL(request.url);
    this.apiCalls.push({
      method: request.method,
      path: `${url.pathname}${url.search}`,
      requestId: request.headers.get("x-oa-request-id"),
      authorization: request.headers.get("authorization"),
      body,
    });
  }

  private assertAuth(request: Request): string | null {
    const authorization = request.headers.get("authorization");
    if (authorization !== `Bearer ${this.expectedToken}`) {
      throw new Error("unauthorized");
    }
    return request.headers.get("x-oa-request-id");
  }

  private createTaskFromRequest(
    requestPayload: ExecutorTaskRequest,
    idempotencyKey: string | undefined,
    source: string | undefined,
    requestId: string | undefined,
    metadata: unknown,
  ): ExecutorTask {
    const nowMs = now();
    this.taskCounter += 1;
    return {
      id: `task-${this.taskCounter}`,
      ownerId: this.expectedUserId,
      status: "queued",
      request: requestPayload,
      attemptCount: 0,
      createdAtMs: nowMs,
      updatedAtMs: nowMs,
      ...(idempotencyKey ? { idempotencyKey } : {}),
      ...(source ? { source } : {}),
      ...(requestId ? { requestId } : {}),
      ...(metadata !== undefined ? { metadata } : {}),
    };
  }

  private handleCreateTask(request: Request, body: unknown): Response {
    const requestId = this.assertAuth(request);
    const payload = asRecord(body);
    if (!payload) return jsonResponse({ ok: false, error: "invalid_input" }, 400);

    const requestPayloadRecord = asRecord(payload.request);
    const url = asString(requestPayloadRecord?.url);
    const maxSpendMsatsRaw = requestPayloadRecord?.maxSpendMsats;
    if (!requestPayloadRecord || !url || typeof maxSpendMsatsRaw !== "number") {
      return jsonResponse({ ok: false, error: "invalid_input" }, 400);
    }

    const requestPayload: ExecutorTaskRequest = {
      url,
      maxSpendMsats: Math.max(0, Math.floor(maxSpendMsatsRaw)),
    };
    const method = asString(requestPayloadRecord.method);
    if (method === "GET" || method === "POST" || method === "PUT" || method === "PATCH" || method === "DELETE") {
      (requestPayload as { method?: ExecutorTaskRequest["method"] }).method = method;
    }
    const headersRecord = asRecord(requestPayloadRecord.headers);
    if (headersRecord) {
      const headersEntries = Object.entries(headersRecord).filter(
        ([headerKey, headerValue]) => typeof headerKey === "string" && typeof headerValue === "string",
      );
      if (headersEntries.length > 0) {
        (requestPayload as { headers?: Readonly<Record<string, string>> }).headers = Object.fromEntries(headersEntries) as Record<
          string,
          string
        >;
      }
    }
    const bodyText = asString(requestPayloadRecord.body);
    if (bodyText) {
      (requestPayload as { body?: string }).body = bodyText;
    }
    const challengeHeader = asString(requestPayloadRecord.challengeHeader);
    if (challengeHeader) {
      (requestPayload as { challengeHeader?: string }).challengeHeader = challengeHeader;
    }
    if (typeof requestPayloadRecord.forceRefresh === "boolean") {
      (requestPayload as { forceRefresh?: boolean }).forceRefresh = requestPayloadRecord.forceRefresh;
    }
    const scope = asString(requestPayloadRecord.scope);
    if (scope) {
      (requestPayload as { scope?: string }).scope = scope;
    }
    if (typeof requestPayloadRecord.cacheTtlMs === "number") {
      (requestPayload as { cacheTtlMs?: number }).cacheTtlMs = Math.max(0, Math.floor(requestPayloadRecord.cacheTtlMs));
    }

    const idempotencyKey = asString(payload.idempotencyKey);
    const source = asString(payload.source);

    const existing = idempotencyKey
      ? this.tasks.find((task) => task.idempotencyKey === idempotencyKey && task.ownerId === this.expectedUserId)
      : null;

    if (existing) {
      return jsonResponse({
        ok: true,
        existed: true,
        task: this.toTaskPayload(existing),
        requestId: requestId ?? null,
      });
    }

    const task = this.createTaskFromRequest(
      requestPayload,
      idempotencyKey,
      source,
      requestId ?? undefined,
      payload.metadata,
    );
    this.tasks.push(task);

    return jsonResponse({
      ok: true,
      existed: false,
      task: this.toTaskPayload(task),
      requestId: requestId ?? null,
    });
  }

  private handleListTasks(request: Request): Response {
    const requestId = this.assertAuth(request);
    const url = new URL(request.url);
    const statusRaw = url.searchParams.get("status");
    const limitRaw = url.searchParams.get("limit");
    const limit = limitRaw ? Math.max(1, Math.min(200, Number(limitRaw) || 100)) : 100;

    const filtered = typeof statusRaw === "string" && statusRaw.length > 0
      ? this.tasks.filter((task) => task.status === statusRaw)
      : this.tasks;
    const tasks = [...filtered]
      .sort((a, b) => b.updatedAtMs - a.updatedAtMs)
      .slice(0, limit)
      .map((task) => this.toTaskPayload(task));

    return jsonResponse({
      ok: true,
      tasks,
      requestId: requestId ?? null,
    });
  }

  private handleTransitionTask(request: Request, taskId: string, body: unknown): Response {
    const requestId = this.assertAuth(request);
    const payload = asRecord(body);
    if (!payload) return jsonResponse({ ok: false, error: "invalid_input" }, 400);

    const toStatus = ensureStatus(payload.toStatus);
    const task = this.taskById(taskId);
    if (!canTransition(task.status, toStatus)) {
      return jsonResponse({ ok: false, error: "invalid_transition" }, 400);
    }

    if (task.status === toStatus) {
      return jsonResponse({
        ok: true,
        changed: false,
        task: this.toTaskPayload(task),
        event: null,
        requestId: requestId ?? null,
      });
    }

    const nextAttempt = toStatus === "running" ? task.attemptCount + 1 : task.attemptCount;
    const nowMs = now();
    const actorRaw = asString(payload.actor);
    const actor = (actorRaw === "web_worker" || actorRaw === "desktop_executor" || actorRaw === "system"
      ? actorRaw
      : "desktop_executor") as TaskEvent["actor"];
    const reason = asString(payload.reason);
    const errorCode = asString(payload.errorCode);
    const errorMessage = asString(payload.errorMessage);

    const updated: ExecutorTask = {
      ...task,
      status: toStatus,
      attemptCount: nextAttempt,
      updatedAtMs: nowMs,
      ...(errorCode ? { lastErrorCode: errorCode } : {}),
      ...(errorMessage ? { lastErrorMessage: errorMessage, failureReason: errorMessage } : {}),
    };

    const index = this.tasks.findIndex((row) => row.id === taskId);
    this.tasks[index] = updated;

    const event: TaskEvent = {
      taskId: updated.id,
      ownerId: updated.ownerId,
      fromStatus: task.status,
      toStatus,
      actor,
      ...(reason ? { reason } : {}),
      ...(requestId ? { requestId } : {}),
      ...(errorCode ? { errorCode } : {}),
      ...(errorMessage ? { errorMessage } : {}),
      ...(payload.metadata !== undefined ? { metadata: payload.metadata } : {}),
      createdAtMs: nowMs,
    };
    this.events.push(event);

    return jsonResponse({
      ok: true,
      changed: true,
      task: this.toTaskPayload(updated),
      event,
      requestId: requestId ?? null,
    });
  }

  private sellerChallenge(invoiceSuffix: string, amountMsats: number): Response {
    return jsonResponse(
      { error: "payment_required" },
      402,
      {
        "www-authenticate": `L402 invoice="lnbcrt1invoice_${invoiceSuffix}", macaroon="mac_${invoiceSuffix}", amount_msats=${amountMsats}`,
      },
    );
  }

  private handleSeller(request: Request): Response {
    const url = new URL(request.url);
    const authorization = request.headers.get("authorization");

    if (url.pathname === "/premium-success") {
      if (!authorization?.startsWith("L402 ")) {
        this.sellerCalls.push({
          path: url.pathname,
          authorization,
          status: 402,
        });
        return this.sellerChallenge("success", 2_500);
      }

      this.sellerCalls.push({
        path: url.pathname,
        authorization,
        status: 200,
      });
      return jsonResponse({ ok: true, resource: "premium-success" }, 200);
    }

    if (url.pathname === "/premium-overbudget") {
      if (!authorization?.startsWith("L402 ")) {
        this.sellerCalls.push({
          path: url.pathname,
          authorization,
          status: 402,
        });
        return this.sellerChallenge("overbudget", 9_000);
      }

      this.sellerCalls.push({
        path: url.pathname,
        authorization,
        status: 200,
      });
      return jsonResponse({ ok: true, resource: "premium-overbudget" }, 200);
    }

    this.sellerCalls.push({
      path: url.pathname,
      authorization,
      status: 404,
    });
    return jsonResponse({ ok: false, error: "not_found" }, 404);
  }

  async fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
    const request = toRequest(input, init);
    const url = new URL(request.url);

    if (url.origin === this.openAgentsBaseUrl) {
      const body = request.method === "GET" ? null : await request.clone().json().catch(() => null);
      this.recordApiCall(request, body);

      if (url.pathname === "/api/lightning/l402/tasks" && request.method === "POST") {
        return this.handleCreateTask(request, body);
      }

      if (url.pathname === "/api/lightning/l402/tasks" && request.method === "GET") {
        return this.handleListTasks(request);
      }

      const transitionMatch = /^\/api\/lightning\/l402\/tasks\/([^/]+)\/transition$/.exec(url.pathname);
      if (transitionMatch && request.method === "POST") {
        const taskId = decodeURIComponent(transitionMatch[1] ?? "");
        return this.handleTransitionTask(request, taskId, body);
      }

      return jsonResponse({ ok: false, error: "not_found" }, 404);
    }

    if (url.origin === this.sellerBaseUrl) {
      return this.handleSeller(request);
    }

    return jsonResponse({ ok: false, error: "unknown_origin" }, 404);
  }

  eventsForTask(taskId: string): ReadonlyArray<TaskEvent> {
    return this.events.filter((event) => event.taskId === taskId);
  }
}

const authGatewayTestLayer = Layer.succeed(
  AuthGatewayService,
  AuthGatewayService.of({
    startMagicCode: () => Effect.void,
    verifyMagicCode: ({ email }) =>
      Effect.succeed({
        userId: "user_local_node",
        token: "token_local_node",
        user: {
          id: "user_local_node",
          email,
          firstName: "Local",
          lastName: "Node",
        },
      }),
    getSession: () =>
      Effect.succeed({
        userId: null,
        token: null,
        user: null,
      }),
  }),
);

const connectivityTestLayer = Layer.succeed(
  ConnectivityProbeService,
  ConnectivityProbeService.of({
    probe: () =>
      Effect.succeed({
        openAgentsReachable: true,
        syncReachable: true,
        syncProvider: "khala",
        checkedAtMs: now(),
      }),
  }),
);

const withFetchHarness = <A, E, R>(
  harness: LocalNodeFlowHarness,
  program: Effect.Effect<A, E, R>,
): Effect.Effect<A, E, R> =>
  Effect.acquireUseRelease(
    Effect.sync(() => {
      const previousFetch = globalThis.fetch;
      globalThis.fetch = harness.fetch.bind(harness) as typeof fetch;
      return previousFetch;
    }),
    () => program,
    (previousFetch) =>
      Effect.sync(() => {
        globalThis.fetch = previousFetch;
      }),
  );

const artifactOutputPath = process.env.OA_L402_LOCAL_NODE_ARTIFACT_PATH
  ? path.resolve(process.env.OA_L402_LOCAL_NODE_ARTIFACT_PATH)
  : null;
const artifacts: Array<FlowArtifact> = [];

afterAll(() => {
  if (!artifactOutputPath) return;
  fs.mkdirSync(path.dirname(artifactOutputPath), { recursive: true });
  fs.writeFileSync(
    artifactOutputPath,
    `${JSON.stringify(
      {
        generatedAtMs: now(),
        flows: artifacts,
      },
      null,
      2,
    )}\n`,
    "utf8",
  );
});

describe("desktop local-node l402 full flow (deterministic harness)", () => {
  it.effect("success path correlates request -> task -> proof identifiers", () => {
    const harness = new LocalNodeFlowHarness();
    const layer = makeDesktopLayer(
      {
        openAgentsBaseUrl: harness.openAgentsBaseUrl,
        convexUrl: "https://convex.local",
        khalaSyncEnabled: true,
        khalaSyncUrl: "wss://khala.local/sync/socket/websocket",
        executorTickMs: 100,
      },
      {
        authGateway: authGatewayTestLayer,
        connectivity: connectivityTestLayer,
      },
    ) as unknown as Layer.Layer<DesktopAppService | TaskProviderService, never, never>;

    return withFetchHarness(
      harness,
      Effect.gen(function* () {
        const app = yield* DesktopAppService;
        yield* app.bootstrap();
        yield* app.verifyMagicCode({ email: "local-node@openagents.com", code: "123456" });

        const queuedTask = yield* app.enqueueDemoTask(`${harness.sellerBaseUrl}/premium-success`);
        expect(queuedTask.status).toBe("queued");

        // Executor must not run queued tasks (approval gating).
        yield* app.tickExecutor();

        const tasks = yield* app.listTasks();
        const stillQueuedTask = tasks.find((task) => task.id === queuedTask.id);
        expect(stillQueuedTask?.status).toBe("queued");
        expect(harness.eventsForTask(queuedTask.id)).toHaveLength(0);
        expect(harness.sellerCalls).toHaveLength(0);

        // Approve then execute.
        const taskProvider = yield* TaskProviderService;
        yield* taskProvider.transitionTask({
          taskId: queuedTask.id,
          token: harness.expectedToken,
          toStatus: "approved",
          reason: "test_approval",
        });

        yield* app.tickExecutor();

        const tasksAfter = yield* app.listTasks();
        const completedTask = tasksAfter.find((task) => task.id === queuedTask.id);
        expect(completedTask?.status).toBe("completed");

        const events = harness.eventsForTask(queuedTask.id);
        expect(events.map((event) => event.toStatus)).toEqual(["approved", "running", "paid", "completed"]);

        const createCall = harness.apiCalls.find(
          (call) => call.path === "/api/lightning/l402/tasks" && call.method === "POST",
        );
        expect(createCall?.requestId).toMatch(/^desktop-/);

        const paidEvent = events.find((event) => event.toStatus === "paid");
        expect(paidEvent?.requestId).toMatch(/^desktop-/);
        const paidMetadata = asRecord(paidEvent?.metadata);
        expect(asString(paidMetadata?.proofReference)).toMatch(/^preimage:/);
        expect(asString(paidMetadata?.paymentId)).toMatch(/^([a-z0-9_:-])+$/);

        expect(harness.sellerCalls).toHaveLength(2);
        expect(harness.sellerCalls[0]?.status).toBe(402);
        expect(harness.sellerCalls[0]?.authorization).toBeNull();
        expect(harness.sellerCalls[1]?.status).toBe(200);
        expect(harness.sellerCalls[1]?.authorization).toMatch(/^L402 /);

        const transitionRequestIds = events
          .map((event) => event.requestId ?? null)
          .filter((requestId): requestId is string => typeof requestId === "string" && requestId.length > 0);
        const proofReference = asString(paidMetadata?.proofReference);
        const paymentId = asString(paidMetadata?.paymentId) ?? null;

        expect(transitionRequestIds.length).toBe(4);
        for (const requestId of transitionRequestIds) {
          expect(requestId).toMatch(/^desktop-/);
        }

        const runtimeSnapshot = yield* app.snapshot();
        const paidAmountMsats = asNumber(paidMetadata?.amountMsats) ?? null;
        const cacheHit = asString(paidMetadata?.cacheStatus) === "hit" ? true : false;
        const successObservability = makeLocalObservabilityRecord({
          requestId: paidEvent?.requestId ?? null,
          userId: completedTask?.ownerId ?? null,
          taskId: queuedTask.id,
          endpoint: queuedTask.request.url,
          quotedCostMsats: paidAmountMsats,
          capAppliedMsats: queuedTask.request.maxSpendMsats,
          paidAmountMsats,
          paymentProofRef: proofReference ?? null,
          cacheHit,
          denyReason: null,
          executor: "desktop",
          plane: "settlement",
          desktopSessionId: createCall?.requestId ?? null,
          snapshot: runtimeSnapshot,
        });
        const successUiProjection = makeLocalObservabilityRecord({
          requestId: transitionRequestIds[transitionRequestIds.length - 1] ?? null,
          userId: completedTask?.ownerId ?? null,
          taskId: queuedTask.id,
          endpoint: "openagents.com/home#l402-transactions",
          quotedCostMsats: paidAmountMsats,
          capAppliedMsats: queuedTask.request.maxSpendMsats,
          paidAmountMsats,
          paymentProofRef: proofReference ?? null,
          cacheHit,
          denyReason: null,
          executor: "desktop",
          plane: "ui",
          desktopSessionId: createCall?.requestId ?? null,
          snapshot: runtimeSnapshot,
        });

        for (const record of [successObservability, successUiProjection]) {
          const missingKeys = L402ObservabilityFieldKeys.filter((key) => !(key in record));
          expect(missingKeys).toEqual([]);
        }

        artifacts.push({
          flow: "success",
          taskId: queuedTask.id,
          taskStatus: completedTask?.status ?? "queued",
          createRequestId: createCall?.requestId ?? "missing",
          transitionRequestIds,
          ...(proofReference ? { proofReference } : {}),
          paymentId,
          sellerCalls: harness.sellerCalls,
          observabilityRecords: [successObservability, successUiProjection],
        });
      }).pipe(Effect.provide(layer)),
    );
  });

  it.effect("deny path records blocked reason with correlated identifiers", () => {
    const harness = new LocalNodeFlowHarness();
    const layer = makeDesktopLayer(
      {
        openAgentsBaseUrl: harness.openAgentsBaseUrl,
        convexUrl: "https://convex.local",
        khalaSyncEnabled: true,
        khalaSyncUrl: "wss://khala.local/sync/socket/websocket",
        executorTickMs: 100,
      },
      {
        authGateway: authGatewayTestLayer,
        connectivity: connectivityTestLayer,
      },
    ) as unknown as Layer.Layer<DesktopAppService | TaskProviderService, never, never>;

    return withFetchHarness(
      harness,
      Effect.gen(function* () {
        const app = yield* DesktopAppService;
        yield* app.bootstrap();
        yield* app.verifyMagicCode({ email: "local-node@openagents.com", code: "123456" });

        const queuedTask = yield* app.enqueueDemoTask(`${harness.sellerBaseUrl}/premium-overbudget`);
        expect(queuedTask.status).toBe("queued");

        // Executor must not run queued tasks (approval gating).
        yield* app.tickExecutor();

        const tasks = yield* app.listTasks();
        const stillQueuedTask = tasks.find((task) => task.id === queuedTask.id);
        expect(stillQueuedTask?.status).toBe("queued");
        expect(harness.eventsForTask(queuedTask.id)).toHaveLength(0);
        expect(harness.sellerCalls).toHaveLength(0);

        // Approve then execute (expected to block before payment).
        const taskProvider = yield* TaskProviderService;
        yield* taskProvider.transitionTask({
          taskId: queuedTask.id,
          token: harness.expectedToken,
          toStatus: "approved",
          reason: "test_approval",
        });

        yield* app.tickExecutor();

        const tasksAfter = yield* app.listTasks();
        const blockedTask = tasksAfter.find((task) => task.id === queuedTask.id);
        expect(blockedTask?.status).toBe("blocked");
        expect(blockedTask?.failureReason).toContain("Quoted invoice amount exceeds configured spend cap");

        const events = harness.eventsForTask(queuedTask.id);
        expect(events.map((event) => event.toStatus)).toEqual(["approved", "running", "blocked"]);

        const blockedEvent = events.find((event) => event.toStatus === "blocked");
        expect(blockedEvent?.requestId).toMatch(/^desktop-/);
        expect(blockedEvent?.errorCode).toBe("BudgetExceededError");
        expect(blockedEvent?.errorMessage).toContain("Quoted invoice amount exceeds configured spend cap");

        expect(harness.sellerCalls).toHaveLength(1);
        expect(harness.sellerCalls[0]?.status).toBe(402);
        expect(harness.sellerCalls[0]?.authorization).toBeNull();

        const createCall = harness.apiCalls.find(
          (call) => call.path === "/api/lightning/l402/tasks" && call.method === "POST",
        );
        const transitionRequestIds = events
          .map((event) => event.requestId ?? null)
          .filter((requestId): requestId is string => typeof requestId === "string" && requestId.length > 0);

        const runtimeSnapshot = yield* app.snapshot();
        const blockedObservability = makeLocalObservabilityRecord({
          requestId: blockedEvent?.requestId ?? null,
          userId: blockedTask?.ownerId ?? null,
          taskId: queuedTask.id,
          endpoint: queuedTask.request.url,
          quotedCostMsats: null,
          capAppliedMsats: queuedTask.request.maxSpendMsats,
          paidAmountMsats: null,
          paymentProofRef: null,
          cacheHit: false,
          denyReason: blockedEvent?.errorMessage ?? null,
          executor: "desktop",
          plane: "settlement",
          desktopSessionId: createCall?.requestId ?? null,
          snapshot: runtimeSnapshot,
        });
        const blockedUiProjection = makeLocalObservabilityRecord({
          requestId: transitionRequestIds[transitionRequestIds.length - 1] ?? null,
          userId: blockedTask?.ownerId ?? null,
          taskId: queuedTask.id,
          endpoint: "openagents.com/home#l402-transactions",
          quotedCostMsats: null,
          capAppliedMsats: queuedTask.request.maxSpendMsats,
          paidAmountMsats: null,
          paymentProofRef: null,
          cacheHit: false,
          denyReason: blockedEvent?.errorMessage ?? null,
          executor: "desktop",
          plane: "ui",
          desktopSessionId: createCall?.requestId ?? null,
          snapshot: runtimeSnapshot,
        });

        for (const record of [blockedObservability, blockedUiProjection]) {
          const missingKeys = L402ObservabilityFieldKeys.filter((key) => !(key in record));
          expect(missingKeys).toEqual([]);
        }

        artifacts.push({
          flow: "blocked",
          taskId: queuedTask.id,
          taskStatus: blockedTask?.status ?? "queued",
          createRequestId: createCall?.requestId ?? "missing",
          transitionRequestIds,
          ...(blockedEvent?.errorCode ? { blockedErrorCode: blockedEvent.errorCode } : {}),
          ...(blockedEvent?.errorMessage ? { blockedReason: blockedEvent.errorMessage } : {}),
          sellerCalls: harness.sellerCalls,
          observabilityRecords: [blockedObservability, blockedUiProjection],
        });
      }).pipe(Effect.provide(layer)),
    );
  });
});
