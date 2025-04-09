import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { Agent, routeAgentRequest, type Connection, type Schedule, type WSMessage } from "agents"
import { AIChatAgent } from "agents/ai-chat-agent";
import { streamText, createDataStreamResponse, type StreamTextOnFinishCallback, tool, generateId } from "ai";
import { env } from "cloudflare:workers";
import { tools, executions } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";
import { z } from "zod";
import { processToolCalls } from "./utils";
import { unstable_getSchedulePrompt } from "agents/schedule";

export const agentContext = new AsyncLocalStorage<Coder>();

const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Coder extends Agent<Env> {
  /**
   * Handles incoming chat messages and manages the response stream
   * @param onFinish - Callback function executed when streaming completes
   */

//   // biome-ignore lint/complexity/noBannedTypes: <explanation>
//   async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
//     // Create a streaming response that handles both text and tool outputs
//     return agentContext.run(this, async () => {
//       const dataStreamResponse = createDataStreamResponse({
//         execute: async (dataStream) => {
//           // Process any pending tool calls from previous messages
//           // This handles human-in-the-loop confirmations for tools
//           const processedMessages = await processToolCalls({
//             messages: this.messages,
//             dataStream,
//             tools,
//             executions,
//           });

//           // Stream the AI response using GPT-4
//           const result = streamText({
//             model,
//             system: `You are a helpful assistant that can do various tasks...

// ${unstable_getSchedulePrompt({ date: new Date() })}

// If the user asks to schedule a task, use the schedule tool to schedule the task.
// `,
//             messages: processedMessages,
//             tools,
//             onFinish,
//             onError: (error) => {
//               console.error("Error while streaming:", error);
//             },
//             maxSteps: 10,
//           });

//           // Merge the AI response stream with tool execution outputs
//           result.mergeIntoDataStream(dataStream);
//         },
//       });

//       return dataStreamResponse;
//     });
//   }
//   async executeTask(description: string, task: Schedule<string>) {
//     await this.saveMessages([
//       ...this.messages,
//       {
//         id: generateId(),
//         role: "user",
//         content: `Running scheduled task: ${description}`,
//         createdAt: new Date(),
//       },
//     ]);
//   }
// }

// export class Coder extends AIChatAgent<Env> {
//   public githubToken?: string;

//   // async onMessage(connection: Connection, message: WSMessage): Promise<void> {
//   //   this.extractToken(connection, message);
//   //   return super.onMessage(connection, message);
//   // }

//   async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
//     // return agentContext.run(this, async () => {
//     //   const dataStreamResponse = createDataStreamResponse({
//     //     execute: async (dataStream) => {
//     const stream = streamText({
//       // tools: {
//       //   getLocalTime: tool({
//       //     description: "get the local time for a specified location",
//       //     parameters: z.object({ location: z.string() }),
//       //     execute: async ({ location }) => {
//       //       console.log(`Getting local time for ${location}`);
//       //       return "10am";
//       //     },
//       //   })
//       // },
//       // toolCallStreaming: true,
//       // tools,
//       model,
//       messages: [
//         { role: 'system', content: 'You are Coder, a helpful assistant. Use the provided tools to help the user.' },
//         ...this.messages
//       ],
//       onFinish,
//       // maxSteps: 5
//     });

//     return stream.toDataStreamResponse()
//     // stream.mergeIntoDataStream(dataStream);
//     // }
//     // });

//     // return dataStreamResponse;
//   }
//   // })


//   // return agentContext.run(this, async () => {



//   //   const dataStreamResponse = createDataStreamResponse({
//   //     execute: async (dataStream) => {
//   //       const stream = streamText({
//   //         tools: {
//   //           getLocalTime: tool({
//   //             description: "get the local time for a specified location",
//   //             parameters: z.object({ location: z.string() }),
//   //             execute: async ({ location }) => {
//   //               console.log(`Getting local time for ${location}`);
//   //               return "10am";
//   //             },
//   //           })
//   //         },
//   //         toolCallStreaming: true,
//   //         // tools,
//   //         model,
//   //         messages: [
//   //           { role: 'system', content: 'You are Coder, a helpful assistant. Use the provided tools to help the user.' },
//   //           ...this.messages
//   //         ],
//   //         onFinish,
//   //         maxSteps: 5
//   //       });
//   //       stream.mergeIntoDataStream(dataStream);
//   //     }
//   //   });

//   //   return dataStreamResponse;
//   // })


//   extractToken(connection: Connection, message: WSMessage) {
//     console.log("extracting token from message");
//     if (typeof message === "string") {
//       let data: any
//       try {
//         data = JSON.parse(message)
//       } catch (error) {
//         console.log("Failed to parse message as JSON");
//         return;
//       }

//       if (data.type === "cf_agent_use_chat_request" && data.init?.method === "POST") {
//         const body = data.init.body;
//         try {
//           const requestData = JSON.parse(body as string);
//           const githubToken = requestData.githubToken;

//           if (githubToken) {
//             console.log(`Found githubToken in message, length: ${githubToken.length}`);
//             // Directly set the token on the instance
//             this.githubToken = githubToken;
//           }
//         } catch (e) {
//           console.error("Error parsing body:", e);
//         }
//       }
//     }
//   }

//   // Public method to get the GitHub token for tools
//   getGitHubToken(): string | undefined {
//     return this.githubToken;
//   }
// }


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
