import { appendFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { readChildTranscript } from "../src/coder-child-transcript.js";

const transcript = (lines: ReadonlyArray<unknown>) => {
  const path = join(mkdtempSync(join(tmpdir(), "oa-child-")), "child.jsonl");
  writeFileSync(path, lines.map((line) => JSON.stringify(line)).join("\n") + "\n");
  return path;
};

describe("reading a child this process ran", () => {
  it("reads its own records back", () => {
    const path = transcript([
      { type: "session", sessionId: "s1", model: "ox-alpha", cwd: "/repo" },
      { type: "tool", callId: "t1", name: "shell", arguments: { command: "git log" } },
      { type: "tool_result", callId: "t1", output: "abc123 a commit" },
      { type: "text", value: "The last commit is abc123." },
    ]);

    expect(readChildTranscript(path)).toEqual([
      { kind: "started", model: "ox-alpha", cwd: "/repo" },
      { kind: "tool", name: "shell", target: "git log" },
      { kind: "output", text: "abc123 a commit" },
      { kind: "text", text: "The last commit is abc123." },
    ]);
  });

  it("takes the one argument worth showing beside a tool", () => {
    const path = transcript([
      { type: "tool", name: "read", arguments: { path: "lib/a.ex", offset: 40 } },
      { type: "tool", name: "think", arguments: { thoughts: "hmm" } },
    ]);

    const entries = readChildTranscript(path);
    expect(entries[0]).toEqual({ kind: "tool", name: "read", target: "lib/a.ex" });
    // Nothing worth showing is shown as nothing, rather than as the first
    // field that happened to be a string.
    expect(entries[1]).toEqual({ kind: "tool", name: "think", target: undefined });
  });
});

describe("reading a child opencode ran", () => {
  it("reads opencode's own event stream through the parser the fleet uses", () => {
    const path = transcript([
      {
        type: "tool_use",
        part: {
          type: "tool",
          callID: "call_1",
          tool: "bash",
          state: { status: "completed", input: { command: "pnpm test" } },
        },
      },
    ]);

    const entries = readChildTranscript(path);
    expect(entries).toHaveLength(1);
    expect(entries[0]).toMatchObject({ kind: "tool", name: "bash" });
  });
});

describe("reading a child that is still writing", () => {
  it("ignores a half-written last line rather than losing the file", () => {
    const path = transcript([{ type: "text", value: "so far" }]);
    // The harness is mid-append. This is the common case while a child runs,
    // not an error state.
    appendFileSync(path, `{"type":"tool","name":"sh`);

    expect(readChildTranscript(path)).toEqual([{ kind: "text", text: "so far" }]);
  });

  it("re-reads once the file has grown, and not before", () => {
    const path = transcript([{ type: "text", value: "one" }]);
    expect(readChildTranscript(path)).toHaveLength(1);

    // Same size, so the parse is reused: a finished child re-parsed on every
    // frame is work that buys nothing.
    expect(readChildTranscript(path)).toHaveLength(1);

    appendFileSync(path, JSON.stringify({ type: "text", value: "two" }) + "\n");
    expect(readChildTranscript(path)).toHaveLength(2);
  });
});

describe("reading a child that has written nothing", () => {
  it("is empty for a path that does not exist, rather than throwing", () => {
    expect(readChildTranscript(join(tmpdir(), "oa-absent", "nothing.jsonl"))).toEqual([]);
  });

  it("is empty for a child the harness has not named a path for", () => {
    expect(readChildTranscript(undefined)).toEqual([]);
  });
});

describe("reading a child Devin ran", () => {
  it("reads the ACP protocol the Devin harness records", () => {
    const path = transcript([
      { jsonrpc: "2.0", id: 1, result: { protocolVersion: 1 } },
      {
        jsonrpc: "2.0",
        method: "session/update",
        params: {
          sessionId: "s",
          update: {
            sessionUpdate: "tool_call",
            toolCallId: "functions.exec:0",
            title: "Ran ls",
            kind: "execute",
          },
        },
      },
      {
        jsonrpc: "2.0",
        method: "session/update",
        params: {
          sessionId: "s",
          update: {
            sessionUpdate: "tool_call_update",
            toolCallId: "functions.exec:0",
            status: "in_progress",
            content: [{ type: "content", content: { type: "text", text: "a.txt" } }],
          },
        },
      },
      {
        jsonrpc: "2.0",
        method: "session/update",
        params: {
          sessionId: "s",
          update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "done" } },
        },
      },
    ]);

    expect(readChildTranscript(path)).toEqual([
      { kind: "tool", name: "execute", target: "Ran ls" },
      { kind: "output", text: "a.txt" },
      { kind: "text", text: "done" },
    ]);
  });

  it("shows none of the handshake, and none of the thinking a token at a time", () => {
    // A Devin child writes hundreds of messages and three kinds are worth
    // reading. Showing the rest would bury what it actually did.
    const path = transcript([
      { jsonrpc: "2.0", id: 2, result: { sessionId: "ses_x" } },
      {
        jsonrpc: "2.0",
        method: "session/update",
        params: {
          sessionId: "s",
          update: { sessionUpdate: "agent_thought_chunk", content: { type: "text", text: "hmm" } },
        },
      },
      { jsonrpc: "2.0", method: "_cognition.ai/output", params: { message: "Connecting to MCP" } },
    ]);

    expect(readChildTranscript(path)).toEqual([]);
  });
});
