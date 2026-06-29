import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  makeAppleFmToolCallbackSession,
  startAppleFmToolCallbackServer,
  type AppleFmToolDefinition,
} from "../src";

const readFileTool: AppleFmToolDefinition = {
  name: "read_file",
  description: "Read a repository file.",
  inputSchema: {
    properties: {
      path: { type: "string" },
    },
    required: ["path"],
  },
  policy: "allow",
  execute: (input) => Effect.succeed({ path: input.path, content: "hello from README" }),
};

const shellTool: AppleFmToolDefinition = {
  name: "shell",
  description: "Run a shell command.",
  inputSchema: {
    properties: {
      cmd: { type: "string" },
    },
    required: ["cmd"],
  },
  policy: "approval_required",
  execute: () => Effect.succeed({ skipped: true }),
};

describe("Apple FM tool callback session", () => {
  test("projects tools and serves a read-only callback through loopback", async () => {
    const session = makeAppleFmToolCallbackSession({
      sessionId: "session_1",
      token: "test-token-secret",
      callbackUrl: "http://127.0.0.1:0/apple-fm/tool-callback/session_1",
      tools: [readFileTool],
      now: new Date("2026-06-07T00:00:00.000Z"),
    });
    const server = startAppleFmToolCallbackServer(session);

    try {
      const response = await fetch(server.callbackUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${session.token}`,
        },
        body: JSON.stringify({
          toolCallId: "tool_call_1",
          toolName: "read_file",
          input: {
            path: "README.md",
          },
        }),
      });
      const body = await response.json() as { readonly status: string; readonly output?: { readonly content?: string } };

      expect(response.status).toBe(200);
      expect(body.status).toBe("success");
      expect(body.output?.content).toBe("hello from README");
      expect(session.projectedTools[0]?.inputSchema.type).toBe("object");
      expect(session.transcript[0]?.status).toBe("success");
      expect(JSON.stringify(session.publicDescriptor())).not.toContain("test-token-secret");
      expect(JSON.stringify(session.publicDescriptor())).not.toContain(server.callbackUrl);
    } finally {
      server.stop();
    }
  });

  test("serves the Swift bridge callback payload shape", async () => {
    const session = makeAppleFmToolCallbackSession({
      sessionId: "session_swift",
      token: "swift-secret",
      tools: [readFileTool],
      now: new Date("2026-06-07T00:00:00.000Z"),
    });
    const server = startAppleFmToolCallbackServer(session);

    try {
      const response = await fetch(server.callbackUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          session_token: "swift-secret",
          tool_name: "read_file",
          arguments: {
            generation_id: "gen_1",
            content: {
              path: "README.md",
            },
            is_complete: true,
          },
        }),
      });
      const body = await response.json() as { readonly output?: string };

      expect(response.status).toBe(200);
      expect(body.output).toContain("hello from README");
      expect(session.transcript[0]?.toolCallId).toBe("gen_1");
      expect(session.transcript[0]?.input).toEqual({ path: "README.md" });
      expect(JSON.stringify(body)).not.toContain("swift-secret");
    } finally {
      server.stop();
    }
  });

  test("approval-required tools persist approval-pending transcript truth", async () => {
    const session = makeAppleFmToolCallbackSession({
      sessionId: "session_approval",
      token: "approval-secret",
      tools: [shellTool],
      now: new Date("2026-06-07T00:00:00.000Z"),
    });
    const result = await Effect.runPromise(
      session.handleCallback({
        token: "approval-secret",
        toolCallId: "tool_call_shell",
        toolName: "shell",
        input: {
          cmd: "rm -rf /tmp/not-running",
        },
      }),
    );

    expect(result.status).toBe("approval_pending");
    expect(result.transcriptEntry.status).toBe("approval_pending");
    expect(session.transcript).toHaveLength(1);
    expect(JSON.stringify(session.transcript)).not.toContain("approval-secret");
  });

  test("max model round trips refuses unbounded callback loops", async () => {
    const session = makeAppleFmToolCallbackSession({
      sessionId: "session_limited",
      token: "limited-secret",
      tools: [readFileTool],
      maxModelRoundTrips: 1,
      now: new Date("2026-06-07T00:00:00.000Z"),
    });

    await Effect.runPromise(
      session.handleCallback({
        token: "limited-secret",
        toolCallId: "tool_call_1",
        toolName: "read_file",
        input: {
          path: "README.md",
        },
      }),
    );
    const refused = await Effect.runPromise(
      session.handleCallback({
        token: "limited-secret",
        toolCallId: "tool_call_2",
        toolName: "read_file",
        input: {
          path: "README.md",
        },
      }),
    );

    expect(refused.status).toBe("round_trip_limit");
    expect(session.transcript.map((entry) => entry.status)).toEqual(["success", "round_trip_limit"]);
  });

  test("resume rebuilds Probe session state from transcript without backend session authority", async () => {
    const session = makeAppleFmToolCallbackSession({
      sessionId: "session_resume",
      token: "resume-secret",
      tools: [readFileTool],
      now: new Date("2026-06-07T00:00:00.000Z"),
    });
    await Effect.runPromise(
      session.handleCallback({
        token: "resume-secret",
        toolCallId: "tool_call_1",
        toolName: "read_file",
        input: {
          path: "README.md",
        },
      }),
    );
    const resumed = session.resumeFromTranscript(session.transcript);

    expect(resumed.sessionId).toBe("session_resume");
    expect(resumed.transcript).toEqual(session.transcript);
    expect(JSON.stringify(resumed.publicDescriptor())).not.toContain("resume-secret");
  });
});
