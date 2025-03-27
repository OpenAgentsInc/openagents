import { McpAgent } from "agents/mcp";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { getFileContents, GetFileContentsSchema } from "./operations/files.js";

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
    }));

    this.server.tool("viewFile", GetFileContentsSchema.shape, async ({ owner, repo, path, branch, token }, { signal }) => {
      try {
        console.log("Starting viewFile with params:", { owner, repo, path, branch });

        const data = await Promise.race([
          getFileContents(owner, repo, path, branch, token).catch(e => {
            console.error("getFileContents error:", e);
            throw e;
          }),
          new Promise<ReturnType<typeof getFileContents>>((_, reject) =>
            setTimeout(() => reject(new Error("Request timed out")), 5000)
          )
        ]);

        console.log("Got response data:", {
          isArray: Array.isArray(data),
          hasContent: 'content' in data,
          dataType: Array.isArray(data) ? 'array' : data.type
        });

        if (Array.isArray(data)) {
          throw new Error("Path points to a directory, not a file");
        }

        // Break up large responses into chunks
        const content = data.content ?? "";
        console.log("Content length:", content.length);

        const chunkSize = 1000;
        const chunks = [];

        for (let i = 0; i < content.length; i += chunkSize) {
          chunks.push(content.slice(i, i + chunkSize));
        }

        console.log("Created chunks:", chunks.length);

        return {
          content: chunks.map(chunk => ({ type: "text", text: chunk }))
        };
      } catch (error: any) {
        console.error("Full error:", error);
        console.error("Error stack:", error.stack);
        return {
          content: [{ type: "text", text: `Error viewing file: ${error.message}\nStack: ${error.stack}` }]
        };
      }
    });
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
