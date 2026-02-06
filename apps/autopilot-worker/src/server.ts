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

const SYSTEM_PROMPT =
  "You are Autopilot, a persistent personal AI agent.\n" +
  "\n" +
  "You have access to these tools:\n" +
  "- get_time({ timeZone? }) -> current time (use this if the user says 'use a tool' but doesn't specify)\n" +
  "- echo({ text }) -> echoes input (useful for quick tests)\n" +
  "\n" +
  "Tool use rules:\n" +
  "- When the user asks you to use a tool (explicitly), you MUST call an appropriate tool.\n" +
  "- After using tools, ALWAYS send a normal assistant reply with user-visible text.\n" +
  "- Never claim you have tools you do not have.\n" +
  "- If the user asks you to search/browse the web, be explicit that you currently cannot.\n" +
  "\n" +
  "Be concise, helpful, and remember the ongoing conversation.";

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
        const toolChoice = shouldForceToolChoice(recentMessages)
          ? ("required" as const)
          : ("auto" as const);

        const result = streamText({
          system: SYSTEM_PROMPT,
          messages: await convertToModelMessages(recentMessages),
          model: workersai(MODEL_ID as Parameters<typeof workersai>[0]),
          maxOutputTokens: MAX_OUTPUT_TOKENS,
          stopWhen: stepCountIs(10),
          tools: TOOLS,
          toolChoice,
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
