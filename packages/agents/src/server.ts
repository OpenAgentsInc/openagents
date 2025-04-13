import { Agent, routeAgentRequest, unstable_callable, type Connection, type Schedule, type WSMessage } from "agents"
import { type UIMessage, generateId, generateText, experimental_createMCPClient as createMCPClient, type ToolSet } from "ai";
import { env } from "cloudflare:workers";
import { AsyncLocalStorage } from "node:async_hooks";
import type { UIPart } from "@openagents/core/src/chat/types";
import { createOpenRouter } from '@openrouter/ai-sdk-provider';
import { type ToolContext } from "@openagents/core/src/tools/toolContext";
import { getFileContentsTool } from "@openagents/core/src/tools/github/getFileContents";
import { addIssueCommentTool } from "@openagents/core/src/tools/github/addIssueComment";
import { tools as availableTools } from "./tools";
const openrouter = createOpenRouter({ apiKey: env.OPENROUTER_API_KEY })
const model = openrouter("google/gemini-2.5-pro-preview-03-25");

export const agentContext = new AsyncLocalStorage<Coder>();

interface CoderState {
  messages: UIMessage[];
}

/** * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Coder extends Agent<Env, CoderState> {
  initialState: CoderState = {
    messages: []
  };
  tools: ToolSet = {};

  async executeTask(description: string, task: Schedule<string>) {
    this.setState({
      messages: [
        ...this.state.messages,
        {
          id: generateId(),
          role: "user",
          content: `Running scheduled task: ${description}`,
          createdAt: new Date(),
          parts: [
            {
              type: "text",
              text: `Running scheduled task: ${description}`
            }
          ],
        },
      ],
    });
    // now infer based on this message
  }

  onMessage(connection: Connection, message: WSMessage) {
    const parsedMessage = JSON.parse(message as string);
    console.log("IN ON MESSAGE AND HAVE PARSED MESSAGE", parsedMessage);
    const githubToken = parsedMessage.githubToken;
    this.infer(githubToken)
  }

  @unstable_callable({
    description: "Generate an AI response based on the current messages",
    streaming: true
  })
  async infer(githubToken: string) {
    return agentContext.run(this, async () => {
      // Get current state messages
      let messages = this.state.messages || [];

      // If there's more than 10 messages, take the first 3 and last 5
      if (messages.length > 10) {
        messages = messages.slice(0, 3).concat(messages.slice(-5));
        console.log("Truncated messages to first 3 and last 5", messages);
      }

      const toolContext: ToolContext = { githubToken }
      const tools = {
        get_file_contents: getFileContentsTool(toolContext),
        add_issue_comment: addIssueCommentTool(toolContext),
        ...availableTools
      }

      const result = await generateText({
        system: `You are a helpful assistant. Help the user with their GitHub repository.`,
        model,
        messages,
        tools,
        maxTokens: 2500,
        temperature: 0.7,
        maxSteps: 5,
      });

      // Create message parts array to handle both text and tool calls
      const messageParts: UIPart[] = [];

      // Add text part if there is text content
      if (result.text) {
        messageParts.push({
          type: 'text' as const,
          text: result.text
        });
      }

      // Add tool calls and their results together
      if (result.toolCalls && result.toolCalls.length > 0) {
        for (const toolCall of result.toolCalls) {
          // @ts-ignore - toolCall type issue
          const toolResult = result.toolResults?.find(r => r.toolCallId === toolCall.toolCallId);

          // Add the tool call
          messageParts.push({
            type: 'tool-invocation' as const,
            toolInvocation: {
              state: 'call' as const,
              // @ts-ignore - toolCall type issue
              toolCallId: toolCall.toolCallId,
              toolName: toolCall.toolName as "getWeatherInformation",
              args: toolCall.args
            }
          });

          // Immediately add its result if available
          if (toolResult) {
            messageParts.push({
              type: 'tool-invocation' as const,
              toolInvocation: {
                state: 'result' as const,
                // @ts-ignore - toolCall type issue
                toolCallId: toolCall.toolCallId,
                toolName: toolCall.toolName as "getWeatherInformation",
                args: toolCall.args,
                // @ts-ignore - toolResult type issue
                result: toolResult.result
              }
            });
          }
        }
      }

      // Update state with the new message containing all parts
      this.setState({
        messages: [
          ...messages,
          {
            id: generateId(),
            role: 'assistant' as const,
            content: result.text || '',
            createdAt: new Date(),
            parts: messageParts
          }
        ]
      });

      return {};
    })
  }
}

/**
 * Worker entry point that routes incoming requests to the appropriate handler
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Route the request to our agent or return 404 if not found
    return (
      (await routeAgentRequest(request, env, { cors: true })) ||
      new Response("Not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
