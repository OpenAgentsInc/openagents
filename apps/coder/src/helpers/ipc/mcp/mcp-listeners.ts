import { ipcMain } from "electron";
import type { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { getCurrentUrl } from "@openagents/core";

let mcpClient: Client | null = null;

export function setMcpClient(client: any) {
  mcpClient = client;
}

export function addMcpEventListeners() {
  ipcMain.handle('mcp:call', async (_, name: string, args: Record<string, any>) => {
    if (!mcpClient) {
      throw new Error('MCP client not connected');
    }

    try {
      const result = await mcpClient.callTool({
        name,
        arguments: args
      });
      return result;
    } catch (error) {
      console.error('Error calling tool:', error);
      throw error;
    }
  });

  ipcMain.handle('mcp:getUrl', () => {
    return getCurrentUrl();
  });
}
