import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  classifyAppleFmAcceptanceStatus,
  makeAppleFmAcceptanceReceipt,
  makeAppleFmToolCallbackSession,
  RETAINED_APPLE_FM_ACCEPTANCE_CASES,
  type AppleFmAcceptanceCaseName,
  type AppleFmToolDefinition,
} from "../src";

const tools = (): ReadonlyArray<AppleFmToolDefinition> => [
  {
    name: "read_file",
    description: "Read a file.",
    inputSchema: { properties: { path: { type: "string" } } },
    policy: "allow",
    execute: (input) => Effect.succeed({ path: input.path, content: "export const value = 1;" }),
  },
  {
    name: "list_files",
    description: "List files.",
    inputSchema: { properties: { path: { type: "string" } } },
    policy: "allow",
    execute: () => Effect.succeed({ files: ["src/index.ts", "README.md"] }),
  },
  {
    name: "code_search",
    description: "Search code.",
    inputSchema: { properties: { query: { type: "string" } } },
    policy: "allow",
    execute: () => Effect.succeed({ matches: [{ path: "src/index.ts", line: 1 }] }),
  },
  {
    name: "shell",
    description: "Run shell.",
    inputSchema: { properties: { cmd: { type: "string" } } },
    policy: "allow",
    execute: () => Effect.succeed({ exitCode: 0, stdout: "ok" }),
  },
  {
    name: "apply_patch",
    description: "Apply patch.",
    inputSchema: { properties: { patch: { type: "string" } } },
    policy: "allow",
    execute: () => Effect.succeed({ applied: true }),
  },
];

async function runFakeCase(caseName: AppleFmAcceptanceCaseName) {
  const session = makeAppleFmToolCallbackSession({
    sessionId: `acceptance_${caseName}`,
    token: "acceptance-secret",
    tools: caseName === "approval_pause_or_refusal"
      ? [
          {
            name: "shell",
            description: "Run shell.",
            inputSchema: { properties: { cmd: { type: "string" } } },
            policy: "approval_required",
            execute: () => Effect.succeed({ skipped: true }),
          },
        ]
      : tools(),
    maxModelRoundTrips: 8,
    now: new Date("2026-06-07T00:00:00.000Z"),
  });

  if (caseName === "read_file_answer") {
    await Effect.runPromise(callback(session, "read_file", { path: "src/index.ts" }));
  }

  if (caseName === "list_then_read") {
    await Effect.runPromise(callback(session, "list_files", { path: "." }));
    await Effect.runPromise(callback(session, "read_file", { path: "src/index.ts" }));
  }

  if (caseName === "search_then_read") {
    await Effect.runPromise(callback(session, "code_search", { query: "value" }));
    await Effect.runPromise(callback(session, "read_file", { path: "src/index.ts" }));
  }

  if (caseName === "shell_then_summarize") {
    await Effect.runPromise(callback(session, "shell", { cmd: "bun test --filter fake" }));
  }

  if (caseName === "patch_then_verify") {
    await Effect.runPromise(callback(session, "apply_patch", { patch: "*** Begin Patch\n*** End Patch" }));
    await Effect.runPromise(callback(session, "shell", { cmd: "bun test" }));
  }

  if (caseName === "approval_pause_or_refusal") {
    await Effect.runPromise(callback(session, "shell", { cmd: "deploy" }));
  }

  const hasFailure = session.transcript.some((entry) =>
    entry.status === "tool_failed" || entry.status === "unknown_tool" || entry.status === "round_trip_limit"
  );
  const receipt = makeAppleFmAcceptanceReceipt({
    caseName,
    status: classifyAppleFmAcceptanceStatus({
      ready: true,
      status: "ready",
      passed: !hasFailure,
    }),
    availability: {
      ready: true,
      status: "ready",
    },
    usage: { truth: "estimated", promptTokens: 1, completionTokens: 1, totalTokens: 2 },
    toolFacts: session.transcript.map((entry) => ({
      toolName: entry.toolName,
      status: entry.status,
      message: entry.message,
    })),
    observedAt: "2026-06-07T00:00:00.000Z",
  });

  return { session, receipt };
}

function callback(
  session: ReturnType<typeof makeAppleFmToolCallbackSession>,
  toolName: string,
  input: Readonly<Record<string, unknown>>,
) {
  return session.handleCallback({
    token: session.token,
    toolCallId: `tool_call_${session.transcript.length + 1}`,
    toolName,
    input,
  });
}

describe("Apple FM retained acceptance cases", () => {
  test("fake acceptance covers all six retained cases", async () => {
    const results = await Promise.all(RETAINED_APPLE_FM_ACCEPTANCE_CASES.map(runFakeCase));

    expect(results.map((result) => result.receipt.caseName)).toEqual(RETAINED_APPLE_FM_ACCEPTANCE_CASES);
    expect(results.every((result) => result.receipt.backendKind === "apple_fm_bridge")).toBe(true);
    expect(results.every((result) => result.receipt.status === "passed")).toBe(true);
    expect(results.every((result) => result.receipt.usage.truth === "estimated")).toBe(true);
    expect(results.some((result) =>
      result.receipt.caseName === "approval_pause_or_refusal" &&
      result.receipt.toolFacts.some((fact) => fact.status === "approval_pending")
    )).toBe(true);
  });

  test("unsupported readiness is recorded separately from failure", () => {
    const receipt = makeAppleFmAcceptanceReceipt({
      caseName: "read_file_answer",
      status: classifyAppleFmAcceptanceStatus({
        ready: false,
        status: "unsupported",
      }),
      availability: {
        ready: false,
        status: "unsupported",
        unavailableReason: "unsupported_hardware",
      },
      observedAt: "2026-06-07T00:00:00.000Z",
    });

    expect(receipt.status).toBe("unsupported");
    expect(receipt.status).not.toBe("failed");
    expect(receipt.availability.unavailableReason).toBe("unsupported_hardware");
  });
});
