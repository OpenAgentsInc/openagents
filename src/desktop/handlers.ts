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
  createSuccessResponse,
  createErrorResponse,
} from "./protocol.js";
import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { readyTasks as getReadyTasks } from "../tasks/service.js";
import { extractCredentialsFromKeychain } from "../sandbox/credentials.js";

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
}

export async function startTBRun(options: TBRunOptions): Promise<{ runId: string }> {
  // Generate run ID
  const timestamp = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 14);
  const random = Math.random().toString(36).slice(2, 8);
  const runId = `tb-${timestamp}-${random}`;

  // Build command args with absolute paths
  const scriptPath = join(PROJECT_ROOT, "src/cli/tbench-local.ts");
  const suitePath = options.suitePath.startsWith("/")
    ? options.suitePath
    : join(PROJECT_ROOT, options.suitePath);
  const outputDir = options.outputDir
    ? options.outputDir.startsWith("/")
      ? options.outputDir
      : join(PROJECT_ROOT, options.outputDir)
    : join(PROJECT_ROOT, "results", runId);

  const args = [scriptPath, "--suite", suitePath, "--output", outputDir];

  if (options.taskIds?.length) {
    args.push("--tasks", options.taskIds.join(","));
  }

  if (options.timeout) {
    args.push("--timeout", String(options.timeout));
  }

  if (options.maxTurns) {
    args.push("--max-turns", String(options.maxTurns));
  }

  console.log(`[TB] Starting run ${runId}:`, args.join(" "));

  // Spawn subprocess from project root
  activeTBRun = spawn({
    cmd: ["bun", ...args],
    cwd: PROJECT_ROOT,
    stdout: "inherit",
    stderr: "inherit",
    env: {
      ...process.env,
      HOME: process.env.HOME ?? Bun.env.HOME,
      PATH: process.env.PATH ?? Bun.env.PATH,
    },
  });

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
    const program = getReadyTasks(tasksPath, { limit });
    const tasks = await Effect.runPromise(
      program.pipe(Effect.provide(BunContext.layer))
    );
    console.log(`[Handler] Found ${tasks.length} ready tasks`);

    return tasks.map((t) => ({
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
