import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

const canary = process.env.OPENAGENTS_MCP_CANARY;
const marker = process.env.OPENAGENTS_MCP_MARKER;
if (typeof canary !== "string" || canary.length < 24 || typeof marker !== "string") process.exit(2);
writeFileSync(marker, `${createHash("sha256").update(canary).digest("hex")}\n`, {
  encoding: "utf8",
  mode: 0o600,
});

const server = new McpServer({ name: "openagents-acp-release-proof", version: "1" });
server.registerTool(
  "release_proof",
  { description: "Returns a public release-proof marker.", inputSchema: {} },
  async () => ({ content: [{ type: "text", text: "release proof ok" }] }),
);
await server.connect(new StdioServerTransport());
