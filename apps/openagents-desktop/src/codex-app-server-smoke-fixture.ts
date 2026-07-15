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
    const completeApprovedTurn = (): void => {
      completionEmitted = true;
      notify("item/completed", {
        threadId: THREAD_ID,
        turnId: TURN_ID,
        item: {
          id: "item-command-smoke",
          type: "commandExecution",
          command: "echo fixture",
          aggregatedOutput: "fixture",
          exitCode: 0,
          status: "completed",
        },
      });
      notify("item/agentMessage/delta", {
        threadId: THREAD_ID,
        turnId: TURN_ID,
        delta: FIXTURE_CODEX_LOCAL_TEXT,
      });
      notify("thread/tokenUsage/updated", {
        threadId: THREAD_ID,
        turnId: TURN_ID,
        tokenUsage: {
          last: {
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
        turn: { id: TURN_ID, status: "completed", error: null },
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
          write({ id: message.id, result: {} });
        } else if (
          (message.method === "thread/start" || message.method === "thread/resume") &&
          typeof message.id === "number"
        ) {
          write({ id: message.id, result: { thread: { id: THREAD_ID } } });
        } else if (message.method === "turn/start" && typeof message.id === "number") {
          const params = message.params as { input?: unknown } | undefined;
          const entries = Array.isArray(params?.input) ? params.input : [];
          const localImageCount = entries.filter(entry =>
            entry !== null && typeof entry === "object" && (entry as { type?: unknown }).type === "localImage"
          ).length;
          if (localImageCount > 0) localImageTurns += 1;
          maxLocalImageCount = Math.max(maxLocalImageCount, localImageCount);
          write({ id: message.id, result: { turn: { id: TURN_ID } } });
          notify("item/completed", {
            threadId: THREAD_ID,
            turnId: TURN_ID,
            item: {
              id: "item-reasoning-smoke",
              type: "reasoning",
              summary: ["planned the fixture reply"],
            },
          });
          notify("item/started", {
            threadId: THREAD_ID,
            turnId: TURN_ID,
            item: {
              id: "item-command-smoke",
              type: "commandExecution",
              command: "echo fixture",
              status: "inProgress",
            },
          });
          write({
            id: APPROVAL_ID,
            method: "item/commandExecution/requestApproval",
            params: {
              threadId: THREAD_ID,
              turnId: TURN_ID,
              itemId: "item-command-smoke",
              command: "echo fixture",
              reason: "Run the fixture command",
              startedAtMs: 0,
            },
          });
          requestId = APPROVAL_ID;
        } else if (message.id === APPROVAL_ID) {
          const result = message.result as { decision?: unknown } | undefined;
          if (result?.decision === "accept") {
            decision = "accept";
            completeApprovedTurn();
          } else {
            notify("error", {
              threadId: THREAD_ID,
              turnId: TURN_ID,
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
