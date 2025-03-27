import { McpAgent } from "agents/mcp";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

export class MyMCP extends McpAgent {
  server = new McpServer({
    name: "OpenAgents GitHub MCP",
    version: "0.0.1",
  });

  async init() {
    this.server.tool("add", { a: z.number(), b: z.number() }, async ({ a, b }) => ({
      content: [{ type: "text", text: String(a + b) }],
    }));

    this.server.tool("hello", {}, async () => ({
      content: [{ type: "text", text: "Hello world" }]
    }))
  }
}

export default {
  fetch: MyMCP.mount("/sse", {
    corsOptions: {
      origin: "*",
      methods: "GET,POST",
      headers: "*",
    },
  }).fetch,
};
