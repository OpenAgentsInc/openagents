import { Client } from "@modelcontextprotocol/sdk/client/index.js";

class SSEClientTransport {
  private eventSource: EventSource | null = null;
  private messageQueue: { resolve: (value: any) => void; reject: (error: any) => void; }[] = [];
  private sessionId: string;

  constructor(private baseUrl: string = "http://localhost:3001") {
    this.sessionId = Math.random().toString(36).substring(7);
  }

  async start(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.eventSource = new EventSource(`${this.baseUrl}/sse?sessionId=${this.sessionId}`);
      
      this.eventSource.onopen = () => {
        resolve();
      };

      this.eventSource.onerror = (error) => {
        reject(error);
      };

      this.eventSource.onmessage = (event) => {
        const resolver = this.messageQueue.shift();
        if (resolver) {
          resolver.resolve(JSON.parse(event.data));
        }
      };
    });
  }

  async stop(): Promise<void> {
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
  }

  async sendMessage(message: any): Promise<any> {
    const response = await fetch(`${this.baseUrl}/messages?sessionId=${this.sessionId}`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(message),
    });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    return new Promise((resolve, reject) => {
      this.messageQueue.push({ resolve, reject });
    });
  }
}

export const createGithubMcpClient = async () => {
  const transport = new SSEClientTransport();
  
  const client = new Client(
    {
      name: "coder-github",
      version: "1.0.0"
    },
    {
      capabilities: {
        resources: {},
        tools: {}
      }
    }
  );

  try {
    await client.connect(transport);
    return client;
  } catch (error) {
    console.error("Failed to connect to GitHub MCP server:", error);
    throw error;
  }
};

export const listRepositoryIssues = async (client: Client, owner: string, repo: string) => {
  try {
    const result = await client.callTool({
      name: "list_issues",
      arguments: { owner, repo }
    });
    return result;
  } catch (error) {
    console.error("Failed to list issues:", error);
    throw error;
  }
};

export const listPullRequests = async (client: Client, owner: string, repo: string) => {
  try {
    const result = await client.callTool({
      name: "list_pull_requests",
      arguments: { owner, repo }
    });
    return result;
  } catch (error) {
    console.error("Failed to list pull requests:", error);
    throw error;
  }
};

export const viewFileContents = async (client: Client, owner: string, repo: string, path: string) => {
  try {
    const result = await client.callTool({
      name: "get_file_contents",
      arguments: { owner, repo, path }
    });
    return result;
  } catch (error) {
    console.error("Failed to view file contents:", error);
    throw error;
  }
};