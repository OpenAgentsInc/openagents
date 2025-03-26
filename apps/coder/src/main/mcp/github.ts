import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { ipcMain } from "electron";

let mcpClient: Client | null = null;

export const initGithubMcp = async () => {
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

  await client.connect(transport);
  mcpClient = client;

  // Set up IPC handlers
  ipcMain.handle("mcp:list-issues", async (_, owner: string, repo: string) => {
    if (!mcpClient) throw new Error("MCP client not initialized");
    return mcpClient.callTool({
      name: "list_issues",
      arguments: { owner, repo }
    });
  });

  ipcMain.handle("mcp:list-prs", async (_, owner: string, repo: string) => {
    if (!mcpClient) throw new Error("MCP client not initialized");
    return mcpClient.callTool({
      name: "list_pull_requests",
      arguments: { owner, repo }
    });
  });

  ipcMain.handle("mcp:view-file", async (_, owner: string, repo: string, path: string) => {
    if (!mcpClient) throw new Error("MCP client not initialized");
    return mcpClient.callTool({
      name: "get_file_contents",
      arguments: { owner, repo, path }
    });
  });
};