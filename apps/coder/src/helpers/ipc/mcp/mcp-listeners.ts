import { ipcMain } from "electron";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";

let mcpClient: Client | null = null;

export function setMcpClient(client: Client) {
  mcpClient = client;
}

export function addMcpEventListeners() {
  ipcMain.handle('mcp:add', async (_, a: number, b: number) => {
    if (!mcpClient) {
      throw new Error('MCP client not connected');
    }

    try {
      const result = await mcpClient.callTool({
        name: 'add',
        arguments: { a, b }
      });
      return result;
    } catch (error) {
      console.error('Error calling add tool:', error);
      throw error;
    }
  });
}
