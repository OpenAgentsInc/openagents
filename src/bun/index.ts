import { BrowserWindow, BrowserView } from "electrobun/bun";
import type { HudMessage } from "../hud/protocol.js";
import { HUD_WS_PORT, parseHudMessage } from "../hud/protocol.js";
import { spawn } from "bun";
import { resolve, dirname, join } from "node:path";
import {
  loadRecentRuns,
  loadTBRun,
  type TBRunWithPath,
  type TBRunFile,
  type TBTaskResult,
  DEFAULT_TB_RUNS_DIR,
} from "../tbench-hud/persistence.js";

// Get the project root - handle both dev (from app bundle) and direct execution
function getProjectRoot(): string {
  const metaPath = dirname(import.meta.path);

  // If running from app bundle, the path contains .app/Contents
  // e.g., /path/to/openagents/build/dev-macos-arm64/OpenAgents-dev.app/Contents/Resources
  // We need to go up to the actual project root
  if (metaPath.includes(".app/Contents")) {
    // Find the build/ directory and go one level up
    const buildIndex = metaPath.indexOf("/build/");
    if (buildIndex !== -1) {
      return metaPath.slice(0, buildIndex);
    }
  }

  // Fallback: go up 2 levels from src/bun/
  return resolve(metaPath, "../..");
}

const PROJECT_ROOT = getProjectRoot();

// ============================================================================
// TB Run Types
// ============================================================================

interface TBRunOptions {
  suitePath: string;
  taskIds?: string[];
  timeout?: number;
  maxTurns?: number;
  outputDir?: string;
}

interface TBSuiteInfo {
  name: string;
  version: string;
  tasks: Array<{
    id: string;
    name: string;
    category: string;
    difficulty: string;
  }>;
}

// Track active TB runs
let activeTBRun: ReturnType<typeof spawn> | null = null;

// ============================================================================
// RPC Schema for HUD Messages
// ============================================================================

/**
 * Response type for loadRecentTBRuns RPC
 */
interface TBRunHistoryItem {
  runId: string;
  suiteName: string;
  suiteVersion: string;
  timestamp: string;
  passRate: number;
  passed: number;
  failed: number;
  timeout: number;
  error: number;
  totalDurationMs: number;
  totalTokens: number;
  taskCount: number;
  filepath: string;
}

/**
 * Response type for loadTBRunDetails RPC
 */
interface TBRunDetailsResponse {
  meta: TBRunHistoryItem;
  tasks: Array<{
    id: string;
    name: string;
    category: string;
    difficulty: string;
    outcome: string;
    durationMs: number;
    turns: number;
    tokens: number;
    outputLines?: number;
  }>;
}

interface HudRpcSchema {
  bun: {
    requests: {
      loadTBSuite: (suitePath: string) => Promise<TBSuiteInfo>;
      startTBRun: (options: TBRunOptions) => Promise<{ runId: string }>;
      stopTBRun: () => Promise<{ stopped: boolean }>;
      /** Load recent TB run metadata (for run history nodes) */
      loadRecentTBRuns: (count?: number) => Promise<TBRunHistoryItem[]>;
      /** Load full run details including tasks (for expanded view) */
      loadTBRunDetails: (runId: string) => Promise<TBRunDetailsResponse | null>;
    };
    messages: {};
  };
  webview: {
    requests: {};
    messages: {
      hudMessage: HudMessage;
    };
  };
}

// ============================================================================
// TB Suite Loading
// ============================================================================

async function loadTBSuiteFile(suitePath: string): Promise<TBSuiteInfo> {
  // Resolve relative paths from project root
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
      id: t.id as string || "",
      name: t.name as string || t.id as string || "",
      category: t.category as string || "uncategorized",
      difficulty: t.difficulty as string || "medium",
    })),
  };
}

// ============================================================================
// TB Run Spawning
// ============================================================================

function startTBRunProcess(options: TBRunOptions): { runId: string } {
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
    ? (options.outputDir.startsWith("/") ? options.outputDir : join(PROJECT_ROOT, options.outputDir))
    : join(PROJECT_ROOT, "results", runId);

  const args = [
    scriptPath,
    "--suite", suitePath,
    "--output", outputDir,
  ];

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
  console.log(`[TB] Project root: ${PROJECT_ROOT}`);

  // Spawn subprocess from project root
  // Explicitly pass environment so Claude CLI auth is available
  // (GUI apps on macOS don't inherit terminal environment)
  activeTBRun = spawn({
    cmd: ["bun", ...args],
    cwd: PROJECT_ROOT,
    stdout: "inherit",
    stderr: "inherit",
    env: {
      ...process.env,
      // Ensure HOME is set for Claude CLI config access
      HOME: process.env.HOME ?? Bun.env.HOME,
      // Preserve PATH for tool access
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

function stopTBRunProcess(): { stopped: boolean } {
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

/**
 * Load recent TB run metadata from filesystem.
 * This runs in the Bun process which has filesystem access.
 */
async function loadRecentTBRunsFromDisk(count: number): Promise<TBRunHistoryItem[]> {
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

/**
 * Load full TB run details including task results.
 * This runs in the Bun process which has filesystem access.
 */
async function loadTBRunDetailsFromDisk(runId: string): Promise<TBRunDetailsResponse | null> {
  const runsDir = join(PROJECT_ROOT, DEFAULT_TB_RUNS_DIR);
  console.log(`[TB] Loading run details for: ${runId}`);

  try {
    // First get the list to find the filepath
    const runs = await loadRecentRuns(50, runsDir);
    const runMeta = runs.find((r) => r.runId === runId);

    if (!runMeta) {
      console.log(`[TB] Run not found: ${runId}`);
      return null;
    }

    // Load full run file
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
        outputLines: t.outputLines,
      })),
    };
  } catch (err) {
    console.error("[TB] Failed to load run details:", err);
    return null;
  }
}

// Define RPC with HUD message support
const rpc = BrowserView.defineRPC<HudRpcSchema>({
  handlers: {
    requests: {
      loadTBSuite: async (suitePath: string) => {
        console.log(`[TB] Loading suite: ${suitePath}`);
        return loadTBSuiteFile(suitePath);
      },
      startTBRun: async (options: TBRunOptions) => {
        console.log(`[TB] Start run requested:`, options);
        return startTBRunProcess(options);
      },
      stopTBRun: async () => {
        return stopTBRunProcess();
      },
      loadRecentTBRuns: async (count?: number) => {
        return loadRecentTBRunsFromDisk(count ?? 20);
      },
      loadTBRunDetails: async (runId: string) => {
        return loadTBRunDetailsFromDisk(runId);
      },
    },
    messages: {},
  },
});

// Create the main application window
const mainWindow = new BrowserWindow({
  title: "OpenAgents",
  url: "views://mainview/index.html",
  frame: {
    width: 1200,
    height: 800,
    x: 200,
    y: 200,
  },
  rpc,
});

// ============================================================================
// HUD WebSocket Server
// ============================================================================

// Track connected clients
const hudClients = new Set<unknown>();

// Start WebSocket server for agent connections
const hudServer = Bun.serve({
  port: HUD_WS_PORT,
  fetch(req, server) {
    // Upgrade HTTP request to WebSocket
    if (server.upgrade(req)) {
      return;
    }
    return new Response("OpenAgents HUD WebSocket Server", { status: 200 });
  },
  websocket: {
    open(ws) {
      hudClients.add(ws);
      console.log(`[HUD] Agent connected (total: ${hudClients.size})`);
    },
    message(ws, message) {
      const data = typeof message === "string" ? message : message.toString();
      const parsed = parseHudMessage(data);

      if (parsed) {
        // Forward message to mainview via Electrobun RPC
        try {
          mainWindow.webview.rpc.send.hudMessage(parsed);
        } catch {
          // Webview may not be ready yet, silently ignore
        }
      }
    },
    close(ws) {
      hudClients.delete(ws);
      console.log(`[HUD] Agent disconnected (total: ${hudClients.size})`);
    },
  },
});

console.log(`[HUD] WebSocket server started on port ${HUD_WS_PORT}`);
console.log("Hello Electrobun app started!");

// Keep references to avoid GC
void mainWindow;
void hudServer;
