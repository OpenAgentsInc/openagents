import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";
import { mkdtempSync, mkdirSync, writeFileSync, symlinkSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  APPLE_FM_WORKSPACE_TOOL_REFS,
  DEFAULT_APPLE_FM_WORKSPACE_TOOL_CAPS,
  makeAppleFmWorkspaceReadOnlyExecutors,
} from "./workspace-tools.js";
import { makeAppleFmToolCallbackSession } from "./tools.js";
import type { AppleFmToolDefinition } from "./tools.js";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const noopExecute = () => Effect.succeed({} as unknown);

function makeWorkspace(): { root: string; outside: string; cleanup: () => void } {
  const base = mkdtempSync(join(tmpdir(), "afm-ws-"));
  const root = join(base, "ws");
  const outside = join(base, "outside");
  mkdirSync(root);
  mkdirSync(outside);
  writeFileSync(join(root, "README.md"), "# Sample Project\nhello world needle\n");
  writeFileSync(join(outside, "secret.txt"), "TOP SECRET");
  mkdirSync(join(root, "src"));
  writeFileSync(join(root, "src", "index.ts"), "export const answer = 42; // needle here\n");
  return { root, outside, cleanup: () => rmSync(base, { recursive: true, force: true }) };
}

const run = <A>(effect: Effect.Effect<A, never>): Promise<A> => Effect.runPromise(effect);

describe("Apple FM workspace read-only tool executors", () => {
  test("read_file reads a confined file and reports byte length", async () => {
    const ws = makeWorkspace();
    try {
      const ex = makeAppleFmWorkspaceReadOnlyExecutors(ws.root);
      const out = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.readFile]({ path: "README.md" }, {} as never))) as Record<string, unknown>;
      expect(out.tool).toBe("read_file");
      expect(out.path).toBe("README.md");
      expect(String(out.content)).toContain("Sample Project");
      expect(out.error).toBeUndefined();
    } finally {
      ws.cleanup();
    }
  });

  test("read_file refuses a workspace-escape path", async () => {
    const ws = makeWorkspace();
    try {
      const ex = makeAppleFmWorkspaceReadOnlyExecutors(ws.root);
      const out = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.readFile]({ path: "../outside/secret.txt" }, {} as never))) as Record<string, unknown>;
      expect(out.error).toBeDefined();
      expect(out.content).toBeUndefined();
      expect(String(out.error)).toContain("escape");
    } finally {
      ws.cleanup();
    }
  });

  test("read_file refuses absolute paths", async () => {
    const ws = makeWorkspace();
    try {
      const ex = makeAppleFmWorkspaceReadOnlyExecutors(ws.root);
      const out = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.readFile]({ path: "/etc/hosts" }, {} as never))) as Record<string, unknown>;
      expect(String(out.error)).toContain("absolute");
    } finally {
      ws.cleanup();
    }
  });

  test("read_file refuses a symlink that escapes the workspace", async () => {
    const ws = makeWorkspace();
    try {
      symlinkSync(join(ws.outside, "secret.txt"), join(ws.root, "link.txt"));
      const ex = makeAppleFmWorkspaceReadOnlyExecutors(ws.root);
      const out = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.readFile]({ path: "link.txt" }, {} as never))) as Record<string, unknown>;
      expect(out.error).toBeDefined();
      expect(out.content).toBeUndefined();
    } finally {
      ws.cleanup();
    }
  });

  test("read_file truncates output at the byte cap", async () => {
    const ws = makeWorkspace();
    try {
      writeFileSync(join(ws.root, "big.txt"), "x".repeat(DEFAULT_APPLE_FM_WORKSPACE_TOOL_CAPS.maxFileBytes + 1000));
      const ex = makeAppleFmWorkspaceReadOnlyExecutors(ws.root);
      const out = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.readFile]({ path: "big.txt" }, {} as never))) as Record<string, unknown>;
      expect(out.truncated).toBe(true);
      expect(String(out.content).length).toBe(DEFAULT_APPLE_FM_WORKSPACE_TOOL_CAPS.maxFileBytes);
    } finally {
      ws.cleanup();
    }
  });

  test("list_files lists the workspace root and refuses an escape", async () => {
    const ws = makeWorkspace();
    try {
      const ex = makeAppleFmWorkspaceReadOnlyExecutors(ws.root);
      const listed = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.listFiles]({ path: "." }, {} as never))) as Record<string, unknown>;
      expect(listed.tool).toBe("list_files");
      const names = (listed.entries as Array<{ name: string }>).map((e) => e.name);
      expect(names).toContain("README.md");
      expect(names).toContain("src");

      const escaped = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.listFiles]({ path: "../outside" }, {} as never))) as Record<string, unknown>;
      expect(escaped.error).toBeDefined();
      expect(escaped.entries).toBeUndefined();
    } finally {
      ws.cleanup();
    }
  });

  test("list_files caps the number of entries", async () => {
    const ws = makeWorkspace();
    try {
      for (let i = 0; i < DEFAULT_APPLE_FM_WORKSPACE_TOOL_CAPS.maxListEntries + 20; i += 1) {
        writeFileSync(join(ws.root, `f${i}.txt`), "x");
      }
      const ex = makeAppleFmWorkspaceReadOnlyExecutors(ws.root);
      const out = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.listFiles]({ path: "." }, {} as never))) as Record<string, unknown>;
      expect(out.truncated).toBe(true);
      expect((out.entries as unknown[]).length).toBe(DEFAULT_APPLE_FM_WORKSPACE_TOOL_CAPS.maxListEntries);
    } finally {
      ws.cleanup();
    }
  });

  test("code_search finds a literal match with a workspace-relative path", async () => {
    const ws = makeWorkspace();
    try {
      const ex = makeAppleFmWorkspaceReadOnlyExecutors(ws.root);
      const out = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.codeSearch]({ query: "needle" }, {} as never))) as Record<string, unknown>;
      expect(out.tool).toBe("code_search");
      const matches = out.matches as Array<{ path: string; line: number }>;
      expect(matches.length).toBeGreaterThan(0);
      for (const m of matches) {
        expect(m.path.startsWith("..")).toBe(false);
        expect(m.path.startsWith("/")).toBe(false);
      }
    } finally {
      ws.cleanup();
    }
  });

  test("code_search caps the number of matches", async () => {
    const ws = makeWorkspace();
    try {
      const cappedRoot = ws.root;
      for (let i = 0; i < DEFAULT_APPLE_FM_WORKSPACE_TOOL_CAPS.maxSearchMatches + 30; i += 1) {
        writeFileSync(join(cappedRoot, `m${i}.txt`), "needle\n");
      }
      const ex = makeAppleFmWorkspaceReadOnlyExecutors(cappedRoot);
      const out = (await run(ex[APPLE_FM_WORKSPACE_TOOL_REFS.codeSearch]({ query: "needle" }, {} as never))) as Record<string, unknown>;
      expect(out.truncated).toBe(true);
      expect((out.matches as unknown[]).length).toBe(DEFAULT_APPLE_FM_WORKSPACE_TOOL_CAPS.maxSearchMatches);
    } finally {
      ws.cleanup();
    }
  });
});

describe("Apple FM tool callback policy and round-trip gating", () => {
  const tool = (name: string, policy: "allow" | "approval_required" | "deny"): AppleFmToolDefinition => ({
    name: name as AppleFmToolDefinition["name"],
    policy,
    inputSchema: { type: "object", properties: {}, additionalProperties: false },
    execute: noopExecute,
  });

  test("denied tools are refused, approval-required tools pause, allow runs", async () => {
    const session = makeAppleFmToolCallbackSession({
      tools: [tool("read_file", "allow"), tool("code_search", "deny"), tool("list_files", "approval_required")],
    });
    const call = (toolName: string) =>
      Effect.runPromise(session.handleCallback({ token: session.token, toolCallId: "c1", toolName, input: {} }));

    expect((await call("read_file")).status).toBe("success");
    expect((await call("code_search")).status).toBe("refused");
    expect((await call("list_files")).status).toBe("approval_pending");
  });

  test("round-trip limit stops the loop", async () => {
    const session = makeAppleFmToolCallbackSession({
      tools: [tool("read_file", "allow")],
      maxModelRoundTrips: 1,
    });
    const first = await Effect.runPromise(session.handleCallback({ token: session.token, toolCallId: "c1", toolName: "read_file", input: {} }));
    expect(first.status).toBe("success");
    const second = await Effect.runPromise(session.handleCallback({ token: session.token, toolCallId: "c2", toolName: "read_file", input: {} }));
    expect(second.status).toBe("round_trip_limit");
  });

  test("an unknown tool name is rejected", async () => {
    const session = makeAppleFmToolCallbackSession({ tools: [tool("read_file", "allow")] });
    const response = await Effect.runPromise(session.handleCallback({ token: session.token, toolCallId: "c1", toolName: "shell", input: {} }));
    expect(response.status).toBe("unknown_tool");
  });
});
