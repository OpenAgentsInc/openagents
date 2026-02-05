import { routeAgentRequest, type Schedule } from "agents";

import { getSchedulePrompt } from "agents/schedule";

import { AIChatAgent } from "@cloudflare/ai-chat";
import {
  generateId,
  streamText,
  type StreamTextOnFinishCallback,
  stepCountIs,
  createUIMessageStream,
  convertToModelMessages,
  createUIMessageStreamResponse,
  type ToolSet
} from "ai";
import { openai } from "@ai-sdk/openai";
import * as Data from "effect/Data";
import * as Effect from "effect/Effect";
import * as Runtime from "effect/Runtime";
import { processToolCalls, cleanupMessages } from "./utils";
import { tools, executions } from "./tools";
import * as EffectWorkers from "./effect/workers";
// import { env } from "cloudflare:workers";

const model = openai("gpt-4o-2024-11-20");
// Cloudflare AI Gateway
// const openai = createOpenAI({
//   apiKey: env.OPENAI_API_KEY,
//   baseURL: env.GATEWAY_BASE_URL,
// });

class DemoEffectError extends Data.TaggedError("DemoEffectError")<{
  message: string;
  cause?: unknown;
}> {}

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Chat extends AIChatAgent<Env> {
  /**
   * Handles incoming chat messages and manages the response stream
   */
  override onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>,
    options?: { abortSignal?: AbortSignal }
  ) {
    const agent = this;
    return Effect.runPromise(
      Effect.gen(function* () {
        // const mcpConnection = await this.mcp.connect(
        //   "https://path-to-mcp-server/sse"
        // );

        // Collect all tools, including MCP tools
        const allTools = {
          ...tools,
          ...agent.mcp.getAITools()
        } as ToolSet;

        const runtime = yield* Effect.runtime();
        const stream = createUIMessageStream({
          execute: ({ writer }) =>
            Runtime.runPromise(
              runtime,
              Effect.gen(function* () {
                // Clean up incomplete tool calls to prevent API errors
                const cleanedMessages = cleanupMessages(agent.messages);

                // Process any pending tool calls from previous messages
                // This handles human-in-the-loop confirmations for tools
                const processedMessages = yield* Effect.tryPromise({
                  try: () =>
                    processToolCalls({
                      messages: cleanedMessages,
                      dataStream: writer,
                      tools: allTools,
                      executions
                    }),
                  catch: (cause) =>
                    new DemoEffectError({
                      message: "Tool processing failed",
                      cause
                    })
                });

                const modelMessages = yield* Effect.tryPromise({
                  try: () => convertToModelMessages(processedMessages),
                  catch: (cause) =>
                    new DemoEffectError({
                      message: "Message conversion failed",
                      cause
                    })
                });

                const result = yield* Effect.try({
                  try: () =>
                    streamText({
                      system: `You are a helpful assistant that can do various tasks... 

${getSchedulePrompt({ date: new Date() })}

If the user asks to schedule a task, use the schedule tool to schedule the task.
`,

                      messages: modelMessages,
                      model,
                      tools: allTools,
                      // Type boundary: streamText expects specific tool types, but base class uses ToolSet
                      // This is safe because our tools satisfy ToolSet interface (verified by 'satisfies' in tools.ts)
                      onFinish: onFinish as unknown as StreamTextOnFinishCallback<
                        typeof allTools
                      >,
                      stopWhen: stepCountIs(10),
                      ...(options?.abortSignal
                        ? { abortSignal: options.abortSignal }
                        : {})
                    }),
                  catch: (cause) =>
                    new DemoEffectError({
                      message: "streamText failed",
                      cause
                    })
                });

                yield* Effect.sync(() => {
                  writer.merge(result.toUIMessageStream());
                });
              })
            )
        });

        return createUIMessageStreamResponse({ stream });
      })
    );
  }
  async executeTask(description: string, _task: Schedule<string>) {
    void _task;
    return Effect.runPromise(
      Effect.tryPromise({
        try: () =>
          this.saveMessages([
            ...this.messages,
            {
              id: generateId(),
              role: "user",
              parts: [
                {
                  type: "text",
                  text: `Running scheduled task: ${description}`
                }
              ],
              metadata: {
                createdAt: new Date()
              }
            }
          ]),
        catch: (cause) =>
          new DemoEffectError({ message: "Save messages failed", cause })
      })
    );
  }
}

/**
 * Worker entry point that routes incoming requests to the appropriate handler
 */
const handleRequest = (request: Request, env: Env) =>
  Effect.gen(function* () {
    const url = new URL(request.url);

    if (url.pathname === "/check-open-ai-key") {
      const hasOpenAIKey = !!process.env.OPENAI_API_KEY;
      return Response.json({
        success: hasOpenAIKey
      });
    }
    if (!process.env.OPENAI_API_KEY) {
      yield* Effect.sync(() => {
        console.error(
          "OPENAI_API_KEY is not set, don't forget to set it locally in .dev.vars, and use `wrangler secret bulk .dev.vars` to upload it to production"
        );
      });
    }

    const response = yield* Effect.tryPromise({
      try: () => routeAgentRequest(request, env),
      catch: (cause) =>
        new DemoEffectError({ message: "Agent routing failed", cause })
    });

    return response ?? new Response("Not found", { status: 404 });
  });

export default EffectWorkers.serve<Env>((request, env) =>
  handleRequest(request, env)
);
