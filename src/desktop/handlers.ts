/**
 * Desktop Request Handlers
 *
 * Handlers for TB operations, used by both:
 * - WebSocket RPC (server.ts)
 * - webview.bind() RPC (main.ts)
 */

import { spawn } from "bun";
import { resolve, dirname, join } from "node:path";
import {
  loadRecentRuns,
  loadTBRun,
  DEFAULT_TB_RUNS_DIR,
} from "../tbench-hud/persistence.js";
import type {
  SocketRequest,
  SocketResponse,
  TBSuiteInfo,
  TBRunHistoryItem,
  TBRunDetails,
  MCTask,
} from "./protocol.js";
import {
  isLoadTBSuiteRequest,
  isStartTBRunRequest,
  isStopTBRunRequest,
  isLoadRecentTBRunsRequest,
  isLoadTBRunDetailsRequest,
  isLoadReadyTasksRequest,
  isAssignTaskToMCRequest,
  isLoadUnifiedTrajectoriesRequest,
  isGetHFTrajectoryCountRequest,
  isGetHFTrajectoriesRequest,
  isGetHFTrajectoryRequest,
  isStartTestGenRequest,
  createSuccessResponse,
  createErrorResponse,
  type UnifiedTrajectory,
} from "./protocol.js";
import { Effect, Layer } from "effect";
import { BunContext } from "@effect/platform-bun";
import { readyTasks as getReadyTasks } from "../tasks/service.js";
import { DatabaseLive } from "../storage/database.js";
import { extractCredentialsFromKeychain } from "../sandbox/credentials.js";
import {
  makeTrajectoryService,
  DEFAULT_TRAJECTORIES_DIR,
  type TrajectoryMetadata,
} from "../atif/service.js";
import {
  OpenThoughtsService,
  OpenThoughtsServiceLive,
  HFDatasetServiceLive,
} from "../huggingface/index.js";
import type { Trajectory } from "../atif/schema.js";
import { runTestGenWithStreaming } from "../hillclimber/testgen-service.js";
import type { HudMessage } from "../hud/protocol.js";
import { generateCorrelationId } from "./protocol.js";

// ============================================================================
// HUD Message Sender
// ============================================================================

let hudMessageSender: ((msg: HudMessage) => void) | null = null;

/**
 * Set the HUD message sender (called from server-worker.ts)
 */
export function setTestGenHudSender(sender: (msg: HudMessage) => void): void {
  hudMessageSender = sender;
}

// ============================================================================
// Project Root Resolution
// ============================================================================

function getProjectRoot(): string {
  const metaPath = dirname(import.meta.path);

  // If running from compiled binary, handle path resolution
  if (metaPath.includes(".app/Contents")) {
    const buildIndex = metaPath.indexOf("/build/");
    if (buildIndex !== -1) {
      return metaPath.slice(0, buildIndex);
    }
  }

  // Go up from src/desktop/ to project root
  return resolve(metaPath, "../..");
}

const PROJECT_ROOT = getProjectRoot();

// ============================================================================
// TB Run State
// ============================================================================

let activeTBRun: ReturnType<typeof spawn> | null = null;

// ============================================================================
// TB Suite Loading
// ============================================================================

export async function loadTBSuite(suitePath: string): Promise<TBSuiteInfo> {
  const fullPath = suitePath.startsWith("/")
    ? suitePath
    : join(PROJECT_ROOT, suitePath);

  console.log(`[TB] Loading suite file: ${fullPath}`);

  const file = Bun.file(fullPath);
  const content = await file.text();
  const suite = JSON.parse(content);

  return {
    name: suite.name || "Terminal-Bench",
    version: suite.version || "unknown",
    tasks: (suite.tasks || []).map((t: Record<string, unknown>) => ({
      id: (t.id as string) || "",
      name: (t.name as string) || (t.id as string) || "",
      category: (t.category as string) || "uncategorized",
      difficulty: (t.difficulty as string) || "medium",
    })),
  };
}

// ============================================================================
// TB Run Spawning
// ============================================================================

export interface TBRunOptions {
  suitePath: string;
  taskIds?: string[];
  timeout?: number;
  maxTurns?: number;
  outputDir?: string;
  sandbox?: boolean;
  sandboxBackend?: "docker" | "macos-container";
  sandboxImage?: string;
  /** Model to use: "fm" (Foundation Model), "claude-code", or ollama:<model> */
  model?: "fm" | "claude-code" | string;
}

export async function startTBRun(options: TBRunOptions): Promise<{ runId: string }> {
  console.log("[TB] startTBRun received options:", JSON.stringify(options, null, 2));
  // Generate run ID
  const timestamp = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 14);
  const random = Math.random().toString(36).slice(2, 8);
  const runId = `tb-${timestamp}-${random}`;

  // Build command args with absolute paths
  // Choose script based on sandbox flag
  const scriptPath = options.sandbox
    ? join(PROJECT_ROOT, "src/cli/tbench-sandbox.ts")
    : join(PROJECT_ROOT, "src/cli/tbench-local.ts");

  const suitePath = options.suitePath.startsWith("/")
    ? options.suitePath
    : join(PROJECT_ROOT, options.suitePath);
  const outputDir = options.outputDir
    ? options.outputDir.startsWith("/")
      ? options.outputDir
      : join(PROJECT_ROOT, options.outputDir)
    : join(PROJECT_ROOT, "results", runId);

  const args = [scriptPath, "--suite", suitePath, "--output", outputDir, "--run-id", runId];

  if (options.taskIds?.length) {
    args.push("--tasks", options.taskIds.join(","));
  }

  if (options.timeout) {
    args.push("--timeout", String(options.timeout));
  }

  if (options.maxTurns) {
    args.push("--max-turns", String(options.maxTurns));
  }

  // Add sandbox options if sandbox mode is enabled
  if (options.sandbox) {
    if (options.sandboxBackend) {
      args.push("--sandbox-backend", options.sandboxBackend);
    }
    if (options.sandboxImage) {
      args.push("--sandbox-image", options.sandboxImage);
    }
  }

  // Add model option (default: claude-code, can be "fm" for Foundation Model)
  if (options.model) {
    args.push("--model", options.model);
  }

  console.log(`[TB] Starting run ${runId}:`, args.join(" "));

  // Spawn subprocess from project root with full environment
  // (needed for Claude Code SDK to find OAuth credentials in ~/.claude/)
  console.log("[TB] Desktop server environment keys:", Object.keys(process.env).sort());
  console.log("[TB] HOME:", process.env.HOME);
  console.log("[TB] PATH:", process.env.PATH);
  console.log("[TB] USER:", process.env.USER);
  activeTBRun = spawn({
    cmd: [process.execPath, ...args], // Use full path to bun executable
    cwd: PROJECT_ROOT,
    stdout: "pipe", // Use pipe to avoid interfering with SDK's subprocess stdio
    stderr: "pipe",
    stdin: "ignore",
    env: process.env, // Pass full environment for SDK subprocess
  });

  // Stream stdout asynchronously for real-time output
  (async () => {
    try {
      // Check if stdout is a ReadableStream before calling getReader()
      if (activeTBRun!.stdout instanceof ReadableStream) {
        const reader = activeTBRun!.stdout.getReader();
        const decoder = new TextDecoder();

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          const text = decoder.decode(value, { stream: true });
          process.stdout.write(text); // Stream to desktop server console
        }
      }
    } catch (err) {
      // Reader cancelled or subprocess killed - this is normal
    }
  })();

  // Stream stderr asynchronously for real-time output
  (async () => {
    try {
      // Check if stderr is a ReadableStream before calling getReader()
      if (activeTBRun!.stderr instanceof ReadableStream) {
        const reader = activeTBRun!.stderr.getReader();
        const decoder = new TextDecoder();

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          const text = decoder.decode(value, { stream: true });
          process.stderr.write(text); // Stream to desktop server console
        }
      }
    } catch (err) {
      // Reader cancelled or subprocess killed - this is normal
    }
  })();

  // Clean up when done
  activeTBRun.exited.then(() => {
    console.log(`[TB] Run ${runId} completed`);
    activeTBRun = null;
  });

  return { runId };
}

export function stopTBRun(): { stopped: boolean } {
  if (activeTBRun) {
    console.log("[TB] Stopping active run");
    activeTBRun.kill();
    activeTBRun = null;
    return { stopped: true };
  }
  return { stopped: false };
}

// ============================================================================
// TB Run History Loading
// ============================================================================

export async function loadRecentTBRuns(count: number = 20): Promise<TBRunHistoryItem[]> {
  const runsDir = join(PROJECT_ROOT, DEFAULT_TB_RUNS_DIR);

  try {
    const runs = await loadRecentRuns(count, runsDir);

    return runs.map((run) => ({
      runId: run.runId,
      suiteName: run.suiteName,
      suiteVersion: run.suiteVersion,
      timestamp: run.timestamp,
      passRate: run.passRate,
      passed: run.passed,
      failed: run.failed,
      timeout: run.timeout,
      error: run.error,
      totalDurationMs: run.totalDurationMs,
      totalTokens: run.totalTokens,
      taskCount: run.taskCount,
      filepath: run.filepath,
    }));
  } catch (err) {
    console.error("[TB] Failed to load runs:", err);
    return [];
  }
}

export async function loadTBRunDetails(runId: string): Promise<TBRunDetails | null> {
  const runsDir = join(PROJECT_ROOT, DEFAULT_TB_RUNS_DIR);
  console.log(`[TB] Loading run details for: ${runId}`);

  try {
    const runs = await loadRecentRuns(50, runsDir);
    const runMeta = runs.find((r) => r.runId === runId);

    if (!runMeta) {
      console.log(`[TB] Run not found: ${runId}`);
      return null;
    }

    const runFile = await loadTBRun(runMeta.filepath);
    console.log(`[TB] Loaded run with ${runFile.tasks.length} tasks`);

    return {
      meta: {
        runId: runFile.meta.runId,
        suiteName: runFile.meta.suiteName,
        suiteVersion: runFile.meta.suiteVersion,
        timestamp: runFile.meta.timestamp,
        passRate: runFile.meta.passRate,
        passed: runFile.meta.passed,
        failed: runFile.meta.failed,
        timeout: runFile.meta.timeout,
        error: runFile.meta.error,
        totalDurationMs: runFile.meta.totalDurationMs,
        totalTokens: runFile.meta.totalTokens,
        taskCount: runFile.meta.taskCount,
        filepath: runMeta.filepath,
      },
      tasks: runFile.tasks.map((t) => ({
        id: t.id,
        name: t.name,
        category: t.category,
        difficulty: t.difficulty,
        outcome: t.outcome,
        durationMs: t.durationMs,
        turns: t.turns,
        tokens: t.tokens,
        ...(t.outputLines !== undefined ? { outputLines: t.outputLines } : {}),
      })),
    };
  } catch (err) {
    console.error("[TB] Failed to load run details:", err);
    return null;
  }
}

/**
 * Get the project root directory.
 */
export function getProjectRootDir(): string {
  return PROJECT_ROOT;
}

// ============================================================================
// Ready Tasks Loading
// ============================================================================

const TASKS_FILE = ".openagents/tasks.jsonl";

// ============================================================================
// MechaCoder Task Assignment
// ============================================================================

/**
 * Ensure Claude Code credentials are available.
 * Extracts from Keychain and writes to ~/.claude/.credentials.json if missing.
 * Reuses the existing credential extraction service from src/sandbox/credentials.ts.
 */
async function ensureClaudeCredentials(): Promise<void> {
  const homeDir = process.env.HOME ?? Bun.env.HOME;
  if (!homeDir) {
    console.warn("[MC] HOME not set, skipping credential export");
    return;
  }

  const claudeDir = join(homeDir, ".claude");
  const credFile = join(claudeDir, ".credentials.json");

  // Check if credentials already exist
  const file = Bun.file(credFile);
  if (await file.exists()) {
    return; // Already have credentials
  }

  // Extract from Keychain using existing service
  try {
    const program = extractCredentialsFromKeychain();
    const jsonStr = await Effect.runPromise(program.pipe(
      Effect.catchAll((err) => {
        console.warn(`[MC] Could not extract Claude Code credentials: ${err.message}`);
        return Effect.fail(err);
      })
    ));

    // Ensure .claude directory exists
    await Bun.$`mkdir -p ${claudeDir}`.quiet();

    // Write credentials file
    await Bun.write(credFile, jsonStr);
    await Bun.$`chmod 600 ${credFile}`.quiet();

    console.log(`[MC] Exported Claude Code credentials to ${credFile}`);
  } catch (e) {
    console.warn(`[MC] Failed to export credentials: ${e}`);
  }
}

export async function assignTaskToMC(
  taskId: string,
  options?: { sandbox?: boolean }
): Promise<{ assigned: boolean }> {
  // Ensure Claude Code credentials are available before spawning
  await ensureClaudeCredentials();

  const scriptPath = join(PROJECT_ROOT, "src/agent/do-one-task.ts");
  const args = ["bun", scriptPath, "--dir", PROJECT_ROOT, "--cc-only"];

  if (options?.sandbox) {
    args.push("--sandbox");
  }

  // TODO: Add --task-id flag support to do-one-task.ts
  // For now, we'll just spawn MechaCoder and let it pick the next ready task
  console.log(`[MC] Assigning task ${taskId} to MechaCoder:`, args.join(" "));

  try {
    // Spawn as background process (fire-and-forget)
    const proc = spawn({
      cmd: args,
      cwd: PROJECT_ROOT,
      stdout: "inherit",
      stderr: "inherit",
      env: {
        ...process.env,
        HOME: process.env.HOME ?? Bun.env.HOME,
        PATH: process.env.PATH ?? Bun.env.PATH,
      },
    });

    // Don't wait for completion - let it run in background
    proc.exited.then(() => {
      console.log(`[MC] MechaCoder process completed for task ${taskId}`);
    });

    return { assigned: true };
  } catch (err) {
    console.error(`[MC] Failed to spawn MechaCoder:`, err);
    throw new Error(`Failed to spawn MechaCoder: ${err}`);
  }
}

// ============================================================================
// Ready Tasks Loading
// ============================================================================

export async function loadReadyTasks(limit?: number): Promise<MCTask[]> {
  const tasksPath = join(PROJECT_ROOT, TASKS_FILE);
  console.log(`[Handler] loadReadyTasks called, path: ${tasksPath}, limit: ${limit}`);

  try {
    const program = getReadyTasks(tasksPath);
    const tasks = await Effect.runPromise(
      program.pipe(Effect.provide(Layer.mergeAll(DatabaseLive, BunContext.layer)))
    );
    console.log(`[Handler] Found ${tasks.length} ready tasks`);

    // Apply limit if specified
    const limitedTasks =
      typeof limit === "number" && limit > 0 ? tasks.slice(0, limit) : tasks;

    return limitedTasks.map((t) => ({
      id: t.id,
      title: t.title,
      description: t.description ?? "",
      status: t.status,
      priority: t.priority,
      type: t.type,
      labels: Array.from(t.labels ?? []),
      createdAt: t.createdAt,
      updatedAt: t.updatedAt,
    }));
  } catch (err) {
    console.error("[Handler] Failed to load ready tasks:", err);
    return [];
  }
}

// ============================================================================
// Unified Trajectories Loading
// ============================================================================

export async function loadUnifiedTrajectories(limit: number = 50): Promise<UnifiedTrajectory[]> {
  console.log(`[Handler] loadUnifiedTrajectories called, limit: ${limit}`);
  const results: UnifiedTrajectory[] = [];

  // 1. Load TB runs
  try {
    const tbRuns = await loadRecentTBRuns(limit);
    for (const run of tbRuns) {
      results.push({
        id: run.runId,
        type: "tb-run",
        timestamp: run.timestamp,
        label: `TB: ${Math.round(run.passRate * 100)}% (${run.passed}/${run.taskCount})`,
        suiteName: run.suiteName,
        passRate: run.passRate,
        passed: run.passed,
        failed: run.failed,
        taskCount: run.taskCount,
      });
    }
    console.log(`[Handler] Loaded ${tbRuns.length} TB runs`);
  } catch (err) {
    console.error("[Handler] Failed to load TB runs:", err);
  }

  // 2. Load ATIF trajectories
  try {
    const trajectoriesDir = join(PROJECT_ROOT, DEFAULT_TRAJECTORIES_DIR);
    const program = Effect.gen(function* () {
      const service = yield* makeTrajectoryService({ trajectoriesDir });
      const sessionIds = yield* service.listTrajectories();
      const metadataList: TrajectoryMetadata[] = [];

      // Load metadata for each session (limit to recent ones)
      const recentIds = sessionIds.slice(0, limit);
      for (const sessionId of recentIds) {
        try {
          const meta = yield* service.getTrajectoryMetadata(sessionId);
          metadataList.push(meta);
        } catch {
          // Skip trajectories with errors
        }
      }

      return metadataList;
    });

    const metadataList = await Effect.runPromise(
      program.pipe(Effect.provide(BunContext.layer))
    );

    for (const meta of metadataList) {
      results.push({
        id: meta.sessionId,
        type: "atif",
        timestamp: meta.createdAt,
        label: `${meta.agentName}: ${meta.totalSteps} steps`,
        agentName: meta.agentName,
        totalSteps: meta.totalSteps,
        modelName: meta.modelName,
      });
    }
    console.log(`[Handler] Loaded ${metadataList.length} ATIF trajectories`);
  } catch (err) {
    console.error("[Handler] Failed to load ATIF trajectories:", err);
  }

  // 3. Sort by timestamp (newest first) and limit
  results.sort((a, b) =>
    new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
  );

  const limited = results.slice(0, limit);
  console.log(`[Handler] Returning ${limited.length} unified trajectories`);
  return limited;
}

// ============================================================================
// HuggingFace Trajectory Handlers
// ============================================================================

/**
 * Build the full layer stack for OpenThoughts service
 *
 * Layer dependencies:
 * - BunContext.layer provides FileSystem and Path
 * - HFDatasetServiceLive() depends on FileSystem/Path
 * - OpenThoughtsServiceLive depends on HFDatasetService
 */
const buildOpenThoughtsLayer = () => {
  const mainLayer = Layer.mergeAll(
    HFDatasetServiceLive(),
  ).pipe(
    Layer.provideMerge(BunContext.layer),
  );

  return OpenThoughtsServiceLive.pipe(
    Layer.provideMerge(mainLayer),
  );
};

/**
 * Get total count of HF trajectories
 */
async function getHFTrajectoryCount(): Promise<{ count: number }> {
  const program = Effect.gen(function* () {
    const service = yield* OpenThoughtsService;
    const count = yield* service.count();
    return { count };
  });

  const layer = buildOpenThoughtsLayer();
  return await Effect.runPromise(program.pipe(Effect.provide(layer)));
}

/**
 * Get paginated HF trajectories
 */
async function getHFTrajectories(offset = 0, limit = 100): Promise<Trajectory[]> {
  const program = Effect.gen(function* () {
    const service = yield* OpenThoughtsService;
    return yield* service.getTrajectories(offset, limit);
  });

  const layer = buildOpenThoughtsLayer();
  return await Effect.runPromise(program.pipe(Effect.provide(layer)));
}

/**
 * Get a single HF trajectory by index
 */
async function getHFTrajectory(index: number): Promise<Trajectory | null> {
  const program = Effect.gen(function* () {
    const service = yield* OpenThoughtsService;
    return yield* service.getTrajectory(index);
  });

  const layer = buildOpenThoughtsLayer();
  return await Effect.runPromise(program.pipe(Effect.provide(layer)));
}

// ============================================================================
// WebSocket Request Handler
// ============================================================================

/**
 * Handle a socket request and return a response.
 * Used by server.ts for WebSocket RPC.
 */
export async function handleRequest(request: SocketRequest): Promise<SocketResponse> {
  const { correlationId } = request;

  try {
    if (isLoadTBSuiteRequest(request)) {
      const data = await loadTBSuite(request.suitePath);
      return createSuccessResponse("response:loadTBSuite", correlationId, data);
    }

    if (isStartTBRunRequest(request)) {
      const data = await startTBRun({
        suitePath: request.suitePath,
        ...(request.taskIds && { taskIds: request.taskIds }),
        ...(request.timeout !== undefined && { timeout: request.timeout }),
        ...(request.maxTurns !== undefined && { maxTurns: request.maxTurns }),
        ...(request.outputDir && { outputDir: request.outputDir }),
        ...(request.model && { model: request.model }),
        ...(request.sandbox !== undefined && { sandbox: request.sandbox }),
        ...(request.sandboxBackend && { sandboxBackend: request.sandboxBackend }),
        ...(request.sandboxImage && { sandboxImage: request.sandboxImage }),
      });
      return createSuccessResponse("response:startTBRun", correlationId, data);
    }

    if (isStopTBRunRequest(request)) {
      const data = stopTBRun();
      return createSuccessResponse("response:stopTBRun", correlationId, data);
    }

    if (isLoadRecentTBRunsRequest(request)) {
      const data = await loadRecentTBRuns(request.count ?? 20);
      return createSuccessResponse("response:loadRecentTBRuns", correlationId, data);
    }

    if (isLoadTBRunDetailsRequest(request)) {
      const data = await loadTBRunDetails(request.runId);
      return createSuccessResponse("response:loadTBRunDetails", correlationId, data);
    }

    if (isLoadReadyTasksRequest(request)) {
      console.log("[Handler] Received loadReadyTasks request");
      const data = await loadReadyTasks(request.limit);
      console.log(`[Handler] Returning ${data.length} tasks`);
      return createSuccessResponse("response:loadReadyTasks", correlationId, data);
    }

    if (isAssignTaskToMCRequest(request)) {
      console.log(`[Handler] Received assignTaskToMC request for task ${request.taskId}`);
      const data = await assignTaskToMC(request.taskId, request.options);
      return createSuccessResponse("response:assignTaskToMC", correlationId, data);
    }

    if (isLoadUnifiedTrajectoriesRequest(request)) {
      console.log("[Handler] Received loadUnifiedTrajectories request");
      const data = await loadUnifiedTrajectories(request.limit ?? 50);
      return createSuccessResponse("response:loadUnifiedTrajectories", correlationId, data);
    }

    if (isGetHFTrajectoryCountRequest(request)) {
      console.log("[Handler] Received getHFTrajectoryCount request");
      const data = await getHFTrajectoryCount();
      return createSuccessResponse("response:getHFTrajectoryCount", correlationId, data);
    }

    if (isGetHFTrajectoriesRequest(request)) {
      console.log(`[Handler] Received getHFTrajectories request (offset=${request.offset}, limit=${request.limit})`);
      const data = await getHFTrajectories(request.offset, request.limit);
      return createSuccessResponse("response:getHFTrajectories", correlationId, data);
    }

    if (isGetHFTrajectoryRequest(request)) {
      console.log(`[Handler] Received getHFTrajectory request (index=${request.index})`);
      const data = await getHFTrajectory(request.index);
      return createSuccessResponse("response:getHFTrajectory", correlationId, data);
    }

    if (isStartTestGenRequest(request)) {
      console.log(`[Handler] Received startTestGen request (taskId=${request.taskId ?? "random"})`);

      if (!hudMessageSender) {
        return createErrorResponse("response:startTestGen", correlationId, "HUD message sender not initialized");
      }

      const sessionId = generateCorrelationId();
      const model = request.model ?? "local";

      // Run test generation in background with streaming HUD messages
      runTestGenWithStreaming(
        request.suitePath,
        request.taskId,
        sessionId,
        {
          onStart: (msg) => hudMessageSender!(msg),
          onTest: (msg) => hudMessageSender!(msg),
          onProgress: (msg) => hudMessageSender!(msg),
          onReflection: (msg) => hudMessageSender!(msg),
          onComplete: (msg) => hudMessageSender!(msg),
          onError: (msg) => hudMessageSender!(msg),
        },
        { model }
      ).catch((err) => {
        console.error(`[Handler] TestGen background error:`, err);
      });

      return createSuccessResponse("response:startTestGen", correlationId, { sessionId });
    }

    // Unknown request type
    const unknownRequest = request as { type: string };
    return createErrorResponse(
      "response:loadTBSuite" as const,
      correlationId,
      `Unknown request type: ${unknownRequest.type}`
    );
  } catch (err) {
    const errorMessage = err instanceof Error ? err.message : String(err);
    const requestAny = request as { type?: string };
    console.error(`[Handler] Error handling ${requestAny.type ?? "unknown"}:`, errorMessage);

    const responseType = (requestAny.type ?? "request:unknown").replace("request:", "response:") as SocketResponse["type"];
    return createErrorResponse(responseType, correlationId, errorMessage);
  }
}
