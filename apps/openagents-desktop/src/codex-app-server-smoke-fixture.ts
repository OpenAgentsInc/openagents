import { EventEmitter } from "node:events";
import { PassThrough } from "node:stream";

import type { CodexAppServerSpawn } from "./codex-app-server-client.ts";
import { FIXTURE_CODEX_LOCAL_TEXT } from "./codex-local-runtime.ts";

const THREAD_ID = "thread-codex-app-server-smoke";
const TURN_ID = "turn-codex-app-server-smoke";
const APPROVAL_ID = 91;

/**
 * Protocol-speaking Codex app-server fixture for the installed Electron smoke.
 *
 * Unlike the retired `codex exec --json` transcript fixture, this behaves as
 * the provider process: it originates a JSON-RPC approval request and does not
 * finish the turn until the production client returns the correlated answer.
 */
export type CodexAppServerSmokeReceipt = Readonly<{
  requestId: number | null;
  decision: "accept" | null;
  completionEmitted: boolean;
  localImageTurns: number;
  maxLocalImageCount: number;
}>;

export const makeCodexAppServerSmokeHarness = (): Readonly<{
  spawn: CodexAppServerSpawn;
  receipt: () => CodexAppServerSmokeReceipt;
}> => {
  let requestId: number | null = null;
  let decision: "accept" | null = null;
  let completionEmitted = false;
  let localImageTurns = 0;
  let maxLocalImageCount = 0;
  const spawn: CodexAppServerSpawn = () => {
    const stdin = new PassThrough();
    const stdout = new PassThrough();
    const child = new EventEmitter() as EventEmitter & {
      stdin: PassThrough;
      stdout: PassThrough;
      stderr: PassThrough;
      kill: () => boolean;
    };
    child.stdin = stdin;
    child.stdout = stdout;
    child.stderr = new PassThrough();
    child.kill = () => {
      child.emit("close", 0);
      return true;
    };

    const write = (message: unknown): void => {
      stdout.write(`${JSON.stringify(message)}\n`);
    };
    const notify = (method: string, params: unknown): void => write({ method, params });
    const cwd = process.cwd();
    const thread = {
      cliVersion: "0.144.1",
      createdAt: 0,
      cwd,
      ephemeral: false,
      id: THREAD_ID,
      modelProvider: "openai",
      preview: "OpenAgents desktop smoke fixture",
      sessionId: THREAD_ID,
      source: "appServer" as const,
      status: { type: "idle" as const },
      turns: [],
      updatedAt: 0,
    };
    const threadResponse = {
      approvalPolicy: "on-request" as const,
      approvalsReviewer: "user" as const,
      cwd,
      model: "gpt-5.6-sol",
      modelProvider: "openai",
      sandbox: { type: "dangerFullAccess" as const },
      thread,
    };
    let turnSequence = 0;
    let activeTurnId = TURN_ID;
    let activeApprovalId = APPROVAL_ID;
    let activeCommandItemId = "item-command-smoke";
    let activeAgentItemId = "item-agent-smoke";
    const completeApprovedTurn = (): void => {
      completionEmitted = true;
      notify("item/completed", {
        threadId: THREAD_ID,
        turnId: activeTurnId,
        completedAtMs: 1,
        item: {
          id: activeCommandItemId,
          type: "commandExecution",
          command: "echo fixture",
          commandActions: [],
          cwd,
          aggregatedOutput: "fixture",
          exitCode: 0,
          status: "completed",
        },
      });
      notify("item/agentMessage/delta", {
        threadId: THREAD_ID,
        turnId: activeTurnId,
        itemId: activeAgentItemId,
        delta: FIXTURE_CODEX_LOCAL_TEXT,
      });
      notify("item/completed", {
        threadId: THREAD_ID,
        turnId: activeTurnId,
        completedAtMs: 2,
        item: {
          id: activeAgentItemId,
          type: "agentMessage",
          text: FIXTURE_CODEX_LOCAL_TEXT,
        },
      });
      notify("thread/tokenUsage/updated", {
        threadId: THREAD_ID,
        turnId: activeTurnId,
        tokenUsage: {
          last: {
            inputTokens: 900,
            cachedInputTokens: 600,
            outputTokens: 40,
            reasoningOutputTokens: 12,
            totalTokens: 952,
          },
          total: {
            inputTokens: 900,
            cachedInputTokens: 600,
            outputTokens: 40,
            reasoningOutputTokens: 12,
            totalTokens: 952,
          },
        },
      });
      notify("turn/completed", {
        threadId: THREAD_ID,
        turn: { id: activeTurnId, items: [], status: "completed", error: null },
      });
    };

    let buffered = "";
    stdin.on("data", (chunk) => {
      buffered += chunk.toString("utf8");
      while (buffered.includes("\n")) {
        const newline = buffered.indexOf("\n");
        const line = buffered.slice(0, newline);
        buffered = buffered.slice(newline + 1);
        if (line === "") continue;
        const message = JSON.parse(line) as Record<string, unknown>;
        if (message.method === "initialize" && typeof message.id === "number") {
          write({
            id: message.id,
            result: {
              codexHome: process.env.CODEX_HOME ?? `${process.env.HOME ?? "/tmp"}/.codex`,
              platformFamily: process.platform === "win32" ? "windows" : "unix",
              platformOs: process.platform === "darwin" ? "macos" : process.platform,
              userAgent: "openagents-desktop-smoke-fixture",
            },
          });
        } else if (
          (message.method === "thread/start" || message.method === "thread/resume") &&
          typeof message.id === "number"
        ) {
          write({ id: message.id, result: threadResponse });
        } else if (message.method === "turn/start" && typeof message.id === "number") {
          turnSequence += 1;
          activeTurnId = turnSequence === 1 ? TURN_ID : `${TURN_ID}-${turnSequence}`;
          activeApprovalId = APPROVAL_ID + turnSequence - 1;
          activeCommandItemId = `item-command-smoke-${turnSequence}`;
          activeAgentItemId = `item-agent-smoke-${turnSequence}`;
          const params = message.params as { input?: unknown; approvalPolicy?: unknown } | undefined;
          const entries = Array.isArray(params?.input) ? params.input : [];
          const localImageCount = entries.filter(entry =>
            entry !== null && typeof entry === "object" && (entry as { type?: unknown }).type === "localImage"
          ).length;
          if (localImageCount > 0) localImageTurns += 1;
          maxLocalImageCount = Math.max(maxLocalImageCount, localImageCount);
          write({
            id: message.id,
            result: { turn: { id: activeTurnId, items: [], status: "inProgress" } },
          });
          notify("item/completed", {
            threadId: THREAD_ID,
            turnId: activeTurnId,
            completedAtMs: 0,
            item: {
              id: "item-reasoning-smoke",
              type: "reasoning",
              summary: ["planned the fixture reply"],
            },
          });
          notify("item/started", {
            threadId: THREAD_ID,
            turnId: activeTurnId,
            startedAtMs: 0,
            item: {
              id: activeCommandItemId,
              type: "commandExecution",
              command: "echo fixture",
              commandActions: [],
              cwd,
              status: "inProgress",
            },
          });
          // Full Auto requests approvalPolicy=never. The real provider must
          // not manufacture an approval interruption on that lane, so the
          // protocol fixture completes directly as well. Interactive turns
          // retain the correlated provider-originated approval below.
          if (params?.approvalPolicy === "never") {
            completeApprovedTurn();
            continue;
          }
          write({
            id: activeApprovalId,
            method: "item/commandExecution/requestApproval",
            params: {
              threadId: THREAD_ID,
              turnId: activeTurnId,
              itemId: activeCommandItemId,
              command: "echo fixture",
              reason: "Run the fixture command",
              startedAtMs: 0,
            },
          });
          requestId = activeApprovalId;
        } else if (message.id === activeApprovalId) {
          const result = message.result as { decision?: unknown } | undefined;
          if (result?.decision === "accept") {
            decision = "accept";
            completeApprovedTurn();
          } else {
            notify("error", {
              threadId: THREAD_ID,
              turnId: activeTurnId,
              willRetry: false,
              error: { message: "The fixture approval was not accepted" },
            });
          }
        }
      }
    });

    return child as never;
  };
  return {
    spawn,
    receipt: () => ({
      requestId,
      decision,
      completionEmitted,
      localImageTurns,
      maxLocalImageCount,
    }),
  };
};
