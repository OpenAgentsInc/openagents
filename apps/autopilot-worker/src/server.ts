import { routeAgentRequest } from "agents";
import { AIChatAgent } from "@cloudflare/ai-chat";
import {
  convertToModelMessages,
  createUIMessageStream,
  createUIMessageStreamResponse,
  jsonSchema,
  stepCountIs,
  streamText,
  type StreamTextOnFinishCallback,
  tool,
  type ToolSet
} from "ai";
import { createWorkersAI } from "workers-ai-provider";

const MODEL_ID = "@cf/openai/gpt-oss-120b";
const MAX_CONTEXT_MESSAGES = 25;
const MAX_OUTPUT_TOKENS = 512;

const TOOLS: ToolSet = {
  get_time: tool({
    description:
      "Return the current time. Use this tool whenever the user asks you to use a tool but doesn't specify which one.",
    inputSchema: jsonSchema({
      type: "object",
      properties: {
        timeZone: {
          type: "string",
          description:
            "Optional IANA time zone name (e.g. 'UTC', 'America/Chicago')."
        }
      },
      additionalProperties: false
    }),
    execute: async ({ timeZone }: { timeZone?: string }) => {
      const now = new Date();
      const iso = now.toISOString();
      const epochMs = now.getTime();
      const epochSec = Math.floor(epochMs / 1000);

      let formatted: string | null = null;
      let resolvedTimeZone: string | null = null;

      if (timeZone) {
        // Intl is available in Workers. Keep output stable and small.
        const dtf = new Intl.DateTimeFormat("en-US", {
          timeZone,
          dateStyle: "medium",
          timeStyle: "medium"
        });
        formatted = dtf.format(now);
        resolvedTimeZone = timeZone;
      }

      return {
        iso,
        epochMs,
        epochSec,
        ...(formatted ? { formatted } : {}),
        ...(resolvedTimeZone ? { timeZone: resolvedTimeZone } : {})
      };
    }
  }),
  echo: tool({
    description: "Echo back the provided text. Useful for testing tool calling.",
    inputSchema: jsonSchema({
      type: "object",
      properties: {
        text: { type: "string" }
      },
      required: ["text"],
      additionalProperties: false
    }),
    execute: async ({ text }: { text: string }) => ({ text })
  })
};

const SYSTEM_PROMPT_BASE =
  "You are Autopilot, a persistent personal AI agent.\n" +
  "\n" +
  "Be concise, helpful, and remember the ongoing conversation.\n" +
  "\n" +
  "Important:\n" +
  "- Do not claim you can browse the web or call tools unless tools are explicitly available.\n";

const SYSTEM_PROMPT_WITH_TOOLS =
  SYSTEM_PROMPT_BASE +
  "\n" +
  "Tools available:\n" +
  "- get_time({ timeZone? }) -> current time\n" +
  "- echo({ text }) -> echoes input\n" +
  "\n" +
  "Tool use rules:\n" +
  "- If the user asks you to use a tool, you MUST call an appropriate tool.\n" +
  "- After using tools, ALWAYS send a normal assistant reply with user-visible text.\n" +
  "- Never claim you have tools you do not have.\n" +
  "- If the user asks you to search/browse the web, be explicit that you currently cannot.\n";

function shouldForceToolChoice(recentMessages: ReadonlyArray<unknown>) {
  // Heuristic: if the user explicitly asks to use tools, require at least one tool call.
  // This avoids "reasoning-only" replies that never actually invoke a tool.
  const lastUser = [...recentMessages].reverse().find((m: any) => m?.role === "user");
  const parts: ReadonlyArray<any> = Array.isArray((lastUser as any)?.parts)
    ? (lastUser as any).parts
    : [];
  const text = parts
    .filter((p) => p && typeof p === "object" && p.type === "text" && typeof p.text === "string")
    .map((p) => p.text)
    .join("")
    .trim();

  if (!text) return false;

  return (
    /\b(use|try|call|run)\b[\s\S]{0,40}\btools?\b/i.test(text) ||
    /\b(search|browse|look\s*up)\b[\s\S]{0,40}\b(web|internet)\b/i.test(text)
  );
}

function getLastUserText(recentMessages: ReadonlyArray<unknown>) {
  const lastUser = [...recentMessages].reverse().find((m: any) => m?.role === "user");
  const parts: ReadonlyArray<any> = Array.isArray((lastUser as any)?.parts)
    ? (lastUser as any).parts
    : [];
  return parts
    .filter((p) => p && typeof p === "object" && p.type === "text" && typeof p.text === "string")
    .map((p) => p.text)
    .join("")
    .trim();
}

function shouldEnableTools(recentMessages: ReadonlyArray<unknown>) {
  const text = getLastUserText(recentMessages);
  if (!text) return false;

  // Preserve streaming + reasoning for normal chat by default. Tools are opt-in.
  return (
    shouldForceToolChoice(recentMessages) ||
    /\b(get_time|echo)\b/i.test(text) ||
    /\bwhat(?:'s| is)\s+the\s+time\b/i.test(text) ||
    /\bcurrent\s+time\b/i.test(text)
  );
}

/**
 * Minimal persistent chat agent:
 * - Durable Object-backed transcript via AIChatAgent
 * - Workers AI model (no keys)
 * - No tools, no sandbox, no containers
 */
export class Chat extends AIChatAgent<Env> {
  override async onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>,
    options?: { abortSignal?: AbortSignal }
  ) {
    const workersai = createWorkersAI({ binding: this.env.AI });
    const recentMessages = this.messages.slice(-MAX_CONTEXT_MESSAGES);

    const stream = createUIMessageStream({
      execute: async ({ writer }) => {
        const toolsEnabled = shouldEnableTools(recentMessages);
        const forceTool = toolsEnabled && shouldForceToolChoice(recentMessages);

        const result = streamText({
          system: toolsEnabled ? SYSTEM_PROMPT_WITH_TOOLS : SYSTEM_PROMPT_BASE,
          messages: await convertToModelMessages(recentMessages),
          model: workersai(MODEL_ID as Parameters<typeof workersai>[0]),
          maxOutputTokens: MAX_OUTPUT_TOKENS,
          stopWhen: stepCountIs(10),
          ...(toolsEnabled
            ? {
                tools: TOOLS,
                // When tools are enabled, keep default 'auto' unless we need to force a tool call.
                ...(forceTool ? { toolChoice: "required" as const } : {})
              }
            : {}),
          // Base class uses this callback to persist messages + stream metadata.
          onFinish: onFinish as unknown as StreamTextOnFinishCallback<ToolSet>,
          ...(options?.abortSignal ? { abortSignal: options.abortSignal } : {})
        });

        writer.merge(result.toUIMessageStream());
      }
    });

    return createUIMessageStreamResponse({ stream });
  }
}

export default {
  async fetch(request: Request, env: Env) {
    const response = await routeAgentRequest(request, env);
    return response ?? new Response("Not found", { status: 404 });
  }
} satisfies ExportedHandler<Env>;
