import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

export const createGithubMcpClient = async () => {
  const transport = new StdioClientTransport({
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-github"]
  });

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