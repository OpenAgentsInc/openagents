import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { routeAgentRequest, type Connection, type WSMessage } from "agents"
import { AIChatAgent } from "agents/ai-chat-agent";
import { streamText, tool, type StreamTextOnFinishCallback } from "ai";
import { env } from "cloudflare:workers";
import { z } from "zod";
import { GitHubContentSchema } from "../../../apps/mcp-github-server/src/common/types";

async function githubRequest(url: string, options: { token?: string }) {
  const headers: Record<string, string> = {
    'Accept': 'application/vnd.github.v3+json',
  };
  if (options.token) {
    headers['Authorization'] = `Bearer ${options.token}`;
  }
  const response = await fetch(url, { headers });
  if (!response.ok) {
    const error = await response.text();
    throw new Error(`GitHub API error (${response.status}): ${error}`);
  }
  return response.json();
}

async function getFileContents(
  owner: string,
  repo: string,
  path: string,
  branch?: string,
  token?: string
) {
  let url = `https://api.github.com/repos/${owner}/${repo}/contents/${path}`;
  if (branch) {
    url += `?ref=${branch}`;
  }

  const response = await githubRequest(url, { token });
  const data = GitHubContentSchema.parse(response);

  // If it's a file, decode the content
  if (!Array.isArray(data) && data.content) {
    // Replace newlines and spaces that GitHub adds to base64
    const cleanContent = data.content.replace(/\n/g, '');
    data.content = atob(cleanContent);
  }

  return data;
}

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

  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    console.log("onChatMessage");

    // check if we have token here
    if (this.githubToken) {
      console.log("WE HAVE TOKEN HERE");
    } else {
      console.log("NO TOKEN HERE");
    }

    const stream = streamText({
      toolCallStreaming: true,
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
            const data = await getFileContents(owner, repo, path, branch, this.githubToken);
            if (Array.isArray(data)) {
              throw new Error('Path points to a directory, not a file');
            }
            console.log("data", data);
            return data.content;
          }
        })
      },
      model,
      messages: [
        { role: 'system', content: 'You are Coder, a helpful assistant.' },
        ...this.messages
      ],
      onFinish,
      maxSteps: 5
    });

    return stream.toDataStreamResponse();
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
