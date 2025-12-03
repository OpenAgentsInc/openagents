import { describe, test, expect } from "bun:test";
import { createMechaCoderMcpServer } from "./mcp-bridge.js";

describe("createMechaCoderMcpServer", () => {
  test("creates MCP server", () => {
    const server = createMechaCoderMcpServer({
      openagentsDir: "/tmp/.openagents",
    });

    expect(server).toBeDefined();
  });

  test("handles callbacks when provided", async () => {
    const server = createMechaCoderMcpServer({
      openagentsDir: "/tmp/.openagents",
      onSubtaskComplete: async (summary, files) => {
        expect(summary).toBe("Test summary");
        expect(files).toEqual(["file.ts"]);
      },
      onHelpRequested: async (issue, suggestion) => {
        expect(issue).toBe("Test issue");
        expect(suggestion).toBe("Test suggestion");
      },
    });

    expect(server).toBeDefined();
    // Note: Actual tool invocation would require MCP runtime
    // These tests verify the server can be created with callbacks
  });

  test("creates server without callbacks", () => {
    const server = createMechaCoderMcpServer({
      openagentsDir: "/tmp/.openagents",
    });

    expect(server).toBeDefined();
  });
});
