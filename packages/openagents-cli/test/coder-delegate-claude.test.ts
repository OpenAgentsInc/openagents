import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import {
  ClaudeCodeHarness,
  parseClaudeEvent,
  type DelegateEvent,
} from "../src/coder-delegate.js";

/**
 * A stand-in for `claude -p --output-format stream-json`, so the tests cost
 * nothing and never call out.
 *
 * It emits the JSONL lines the harness expects and, optionally, records the
 * arguments it was called with before exiting.
 */
const fakeClaude = (options: {
  readonly lines?: ReadonlyArray<string>;
  readonly exitCode?: number;
  readonly hang?: boolean;
  readonly recordArgsTo?: string;
} = {}): string => {
  const directory = mkdtempSync(join(tmpdir(), "claude-code-"));
  const script = join(directory, "claude.mjs");

  writeFileSync(
    script,
    `
import { appendFileSync } from "node:fs";
const recordTo = ${JSON.stringify(options.recordArgsTo ?? null)};
if (recordTo) appendFileSync(recordTo, process.argv.slice(2).join(" ") + "\\n");
if (${options.hang === true}) { setInterval(() => {}, 1000); }
${(options.lines ?? [])
  .map((line) => `process.stdout.write(${JSON.stringify(`${line}\n`)});`)
  .join("\n")}
${options.hang === true ? "" : `process.exit(${options.exitCode ?? 0});`}
`,
  );

  const shim = join(directory, "claude-stub");
  writeFileSync(shim, `#!/bin/sh\nexec ${process.execPath} ${script} "$@"\n`);
  chmodSync(shim, 0o755);
  return shim;
};

const collect = async (
  harness: ClaudeCodeHarness,
  cwd = process.cwd(),
): Promise<ReadonlyArray<DelegateEvent>> => {
  const events: DelegateEvent[] = [];
  const transcriptPath = join(mkdtempSync(join(tmpdir(), "claude-t-")), "child.jsonl");
  for await (const event of harness.run(
    { prompt: "do the thing", cwd, transcriptPath },
    new AbortController().signal,
  )) {
    events.push(event);
  }
  return events;
};

describe("parseClaudeEvent", () => {
  it("ignores blank lines, prose, and malformed JSON", () => {
    expect(parseClaudeEvent("")).toBeUndefined();
    expect(parseClaudeEvent("starting…")).toBeUndefined();
    expect(parseClaudeEvent('{"type":"future"}')).toBeUndefined();
    expect(parseClaudeEvent('{"type":"assistan')).toBeUndefined();
  });

  it("reads a system init as a session", () => {
    expect(
      parseClaudeEvent(
        JSON.stringify({
          type: "system",
          subtype: "init",
          session_id: "sess_01",
          model: "sonnet",
          tools: ["Bash"],
        }),
      ),
    ).toEqual({ type: "session", sessionId: "sess_01" });
  });

  it("reads an assistant tool_use block", () => {
    const line = JSON.stringify({
      type: "assistant",
      session_id: "sess_01",
      message: {
        id: "msg_1",
        role: "assistant",
        content: [
          { type: "tool_use", id: "toolu_1", name: "Bash", input: { command: "ls -la" } },
        ],
      },
    });
    expect(parseClaudeEvent(line)).toEqual({
      type: "tool",
      callId: "toolu_1",
      name: "Bash",
      target: "ls -la",
    });
  });

  it("reads an assistant text block", () => {
    expect(
      parseClaudeEvent(
        JSON.stringify({
          type: "assistant",
          session_id: "sess_01",
          message: {
            id: "msg_2",
            role: "assistant",
            content: [{ type: "text", text: "Done." }],
          },
        }),
      ),
    ).toEqual({ type: "text", value: "Done." });
  });

  it("reads token usage from a result", () => {
    expect(
      parseClaudeEvent(
        JSON.stringify({
          type: "result",
          subtype: "success",
          session_id: "sess_01",
          result: "All done.",
          usage: { input_tokens: 150, output_tokens: 30 },
        }),
      ),
    ).toEqual({ type: "tokens", input: 150, output: 30 });
  });

  it("reads a result error as an error", () => {
    expect(
      parseClaudeEvent(
        JSON.stringify({
          type: "result",
          subtype: "error",
          is_error: true,
          error: "Permission denied",
        }),
      ),
    ).toEqual({ type: "error", message: "Permission denied" });
  });

  it("reads a stream_event text delta", () => {
    expect(
      parseClaudeEvent(
        JSON.stringify({
          type: "stream_event",
          event: { type: "content_block_delta", delta: { type: "text_delta", text: "hello" } },
        }),
      ),
    ).toEqual({ type: "text", value: "hello" });
  });

  it("reads a stream_event tool start", () => {
    const line = JSON.stringify({
      type: "stream_event",
      event: {
        type: "content_block_start",
        content_block: { type: "tool_use", id: "toolu_2", name: "Read", input: { file_path: "/tmp/x" } },
      },
    });
    expect(parseClaudeEvent(line)).toEqual({
      type: "tool",
      callId: "toolu_2",
      name: "Read",
      target: "/tmp/x",
    });
  });
});

describe("running children on the Claude Code CLI", () => {
  it("names itself in the fleet, so a Claude child is not mistaken for opencode", () => {
    const harness = new ClaudeCodeHarness();
    expect(harness.agent).toBe("claude");
    expect(harness.model).toBe("not reported");
  });

  it("reports the child's answer as text", async () => {
    const events = await collect(
      new ClaudeCodeHarness({
        command: fakeClaude({
          lines: [
            JSON.stringify({
              type: "system",
              subtype: "init",
              session_id: "sess_01",
              tools: ["Bash"],
            }),
            JSON.stringify({
              type: "assistant",
              session_id: "sess_01",
              message: {
                id: "msg_1",
                role: "assistant",
                content: [{ type: "text", text: "PONG" }],
              },
            }),
            JSON.stringify({
              type: "result",
              subtype: "success",
              session_id: "sess_01",
              result: "PONG",
              usage: { input_tokens: 10, output_tokens: 1 },
            }),
          ],
        }),
      }),
    );

    expect(events.at(-2)).toEqual({ type: "text", value: "PONG" });
    expect(events.at(-1)).toEqual({ type: "tokens", input: 10, output: 1 });
  });

  it("reports what the child is doing while it does it", async () => {
    const events = await collect(
      new ClaudeCodeHarness({
        command: fakeClaude({
          lines: [
            JSON.stringify({
              type: "system",
              subtype: "init",
              session_id: "sess_01",
              tools: ["Bash", "Read"],
            }),
            JSON.stringify({
              type: "assistant",
              session_id: "sess_01",
              message: {
                id: "msg_1",
                role: "assistant",
                content: [
                  { type: "tool_use", id: "toolu_1", name: "Bash", input: { command: "ls -la" } },
                ],
              },
            }),
            JSON.stringify({
              type: "result",
              subtype: "success",
              session_id: "sess_01",
              result: "Done.",
              usage: { input_tokens: 100, output_tokens: 20 },
            }),
          ],
        }),
      }),
    );

    expect(events).toContainEqual({
      type: "tool",
      callId: "toolu_1",
      name: "Bash",
      target: "ls -la",
    });
    expect(events).toContainEqual({ type: "tokens", input: 100, output: 20 });
  });

  it("reports the session, so a retry can resume it", async () => {
    const events = await collect(
      new ClaudeCodeHarness({
        command: fakeClaude({
          lines: [
            JSON.stringify({
              type: "system",
              subtype: "init",
              session_id: "sess_01",
              tools: ["Bash"],
            }),
            JSON.stringify({
              type: "result",
              subtype: "success",
              session_id: "sess_01",
              result: "ok",
              usage: { input_tokens: 5, output_tokens: 1 },
            }),
          ],
        }),
      }),
    );

    expect(events[0]).toEqual({ type: "session", sessionId: "sess_01" });
  });

  it("writes the raw event stream to the transcript as it arrives", async () => {
    const transcriptPath = join(mkdtempSync(join(tmpdir(), "claude-t-")), "child.jsonl");
    const harness = new ClaudeCodeHarness({
      command: fakeClaude({
        lines: [
          JSON.stringify({
            type: "assistant",
            session_id: "sess_01",
            message: {
              id: "msg_1",
              role: "assistant",
              content: [{ type: "text", text: "ok" }],
            },
          }),
        ],
      }),
    });

    for await (const _event of harness.run(
      { prompt: "x", cwd: process.cwd(), transcriptPath },
      new AbortController().signal,
    )) {
      void _event;
    }

    const written = readFileSync(transcriptPath, "utf8");
    expect(written).toContain("ok");
  });

  it("uses print, stream-json, verbose, and a permission mode for unattended runs", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "claude-args-")), "log");
    await collect(
      new ClaudeCodeHarness({
        command: fakeClaude({
          recordArgsTo: log,
          exitCode: 0,
          lines: [
            JSON.stringify({
              type: "result",
              subtype: "success",
              session_id: "sess_01",
              result: "ok",
              usage: { input_tokens: 1, output_tokens: 1 },
            }),
          ],
        }),
      }),
    );

    const said = readFileSync(log, "utf8");
    expect(said).toContain("-p");
    expect(said).toContain("--output-format stream-json");
    expect(said).toContain("--verbose");
    expect(said).toContain("--permission-mode auto");
  });

  it("passes the model through when the lane names one", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "claude-args-")), "log");
    const harness = new ClaudeCodeHarness({
      command: fakeClaude({
        recordArgsTo: log,
        exitCode: 0,
        lines: [
          JSON.stringify({
            type: "result",
            subtype: "success",
            session_id: "sess_01",
            result: "ok",
            usage: { input_tokens: 1, output_tokens: 1 },
          }),
        ],
      }),
      model: "fable",
    });

    await collect(harness);

    expect(harness.model).toBe("fable");
    expect(readFileSync(log, "utf8")).toContain("--model fable");
  });

  it("carries the prompt through as the first positional argument", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "claude-args-")), "log");
    await collect(
      new ClaudeCodeHarness({
        command: fakeClaude({
          recordArgsTo: log,
          exitCode: 0,
          lines: [
            JSON.stringify({
              type: "result",
              subtype: "success",
              session_id: "sess_01",
              result: "ok",
              usage: { input_tokens: 1, output_tokens: 1 },
            }),
          ],
        }),
      }),
    );

    const said = readFileSync(log, "utf8");
    expect(said).toContain("-p do the thing");
  });

  it("reports a Claude result error as a failed child", async () => {
    const harness = new ClaudeCodeHarness({
      command: fakeClaude({
        lines: [
          JSON.stringify({
            type: "result",
            subtype: "error",
            is_error: true,
            error: "the model provider is unavailable",
          }),
        ],
        exitCode: 1,
      }),
    });

    await expect(collect(harness)).rejects.toThrow(/model provider is unavailable/);
  });

  it("throws when the binary is not on PATH, rather than reporting an empty child", async () => {
    await expect(
      collect(new ClaudeCodeHarness({ command: "claude-does-not-exist" })),
    ).rejects.toThrow("not on the path");
  });

  it("stops when the fleet is stopped", async () => {
    const harness = new ClaudeCodeHarness({
      command: fakeClaude({
        hang: true,
        lines: [
          JSON.stringify({
            type: "system",
            subtype: "init",
            session_id: "sess_01",
            tools: ["Bash"],
          }),
        ],
      }),
    });
    const controller = new AbortController();
    const events: DelegateEvent[] = [];
    const transcriptPath = join(mkdtempSync(join(tmpdir(), "claude-t-")), "child.jsonl");

    const running = (async () => {
      for await (const event of harness.run(
        { prompt: "x", cwd: process.cwd(), transcriptPath },
        controller.signal,
      )) {
        events.push(event);
      }
    })();

    controller.abort();
    await running;

    expect(events.every((event) => event.type !== "text")).toBe(true);
  });
});
