import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { routeAgentRequest, type Connection, type WSMessage } from "agents"
import { AIChatAgent } from "agents/ai-chat-agent";
import { streamText, tool, type StreamTextOnFinishCallback } from "ai";
import { env } from "cloudflare:workers";
import { z } from "zod";

const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

export class Coder extends AIChatAgent<Env> {
  public githubToken?: string;

  async onMessage(connection: Connection, message: WSMessage): Promise<void> {
    this.extractToken(connection, message);
    return super.onMessage(connection, message);
  }

  onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    console.log("onChatMessage");

    // check if we have token here
    if (this.githubToken) {
      console.log("WE HAVE TOKEN HERE");
    } else {
      console.log("NO TOKEN HERE");
    }

    const stream = streamText({
      tools: {
        fetchGitHubFileContent: tool({
          description: 'Fetch the content of a file from a GitHub repository',
          parameters: z.object({
            owner: z.string(),
            repo: z.string(),
            path: z.string(),
            branch: z.string(),
          }),
          execute: async ({ owner, repo, path, branch }) => {
            const response = await fetch(`https://api.github.com/repos/${owner}/${repo}/contents/${path}?ref=${branch}`, {
              headers: {
                'Authorization': `Bearer ${this.githubToken}`
              }
            })
            const data = await response.json() as { content: string }
            return data.content
          }
        })
      },
      model,
      messages: [
        { role: 'system', content: 'You are a helpful assistant that can answer questions and help with tasks. In your message, tell the user if you DO or DO NOT have a github token set based on this (truncated) value: ' + this.githubToken?.slice(0, 18) },
        ...this.messages
      ],
      onFinish,
    });

    return Promise.resolve(stream.toDataStreamResponse());
  }

  extractToken(connection: Connection, message: WSMessage) {
    console.log("extracting token from message");
    if (typeof message === "string") {
      let data: any
      try {
        data = JSON.parse(message)
      } catch (error) {
        console.log("Failed to parse message as JSON");
        return;
      }

      if (data.type === "cf_agent_use_chat_request" && data.init?.method === "POST") {
        const body = data.init.body;
        try {
          const requestData = JSON.parse(body as string);
          const githubToken = requestData.githubToken;

          if (githubToken) {
            console.log(`Found githubToken in message, length: ${githubToken.length}`);
            // Directly set the token on the instance
            this.githubToken = githubToken;
          }
        } catch (e) {
          console.error("Error parsing body:", e);
        }
      }
    }
  }

  // Public method to get the GitHub token for tools
  getGitHubToken(): string | undefined {
    return this.githubToken;
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
