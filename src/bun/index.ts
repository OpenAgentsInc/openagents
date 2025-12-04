import { BrowserWindow, BrowserView } from "electrobun/bun";
import type { HudMessage } from "../hud/protocol.js";
import { HUD_WS_PORT, parseHudMessage } from "../hud/protocol.js";
import { spawn } from "bun";

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

interface HudRpcSchema {
  bun: {
    requests: {
      loadTBSuite: (suitePath: string) => Promise<TBSuiteInfo>;
      startTBRun: (options: TBRunOptions) => Promise<{ runId: string }>;
      stopTBRun: () => Promise<{ stopped: boolean }>;
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
  const file = Bun.file(suitePath);
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

  // Build command args
  const args = [
    "src/cli/tbench-local.ts",
    "--suite", options.suitePath,
    "--output", options.outputDir || `./results/${runId}`,
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

  // Spawn subprocess
  activeTBRun = spawn({
    cmd: ["bun", ...args],
    stdout: "inherit",
    stderr: "inherit",
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
