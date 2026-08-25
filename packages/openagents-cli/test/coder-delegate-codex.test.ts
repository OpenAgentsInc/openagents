import { chmodSync, mkdtempSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import {
  CodexHarness,
  parseCodexEvent,
  type DelegateEvent,
} from "../src/coder-delegate.js";

/**
 * A stand-in for `codex exec --json`, so the tests cost nothing and never
 * call out.
 *
 * It emits the JSONL lines the harness expects and, optionally, records the
 * arguments or working directory it was called with before exiting.
 */
const fakeCodex = (options: {
  readonly lines?: ReadonlyArray<string>;
  readonly exitCode?: number;
  readonly hang?: boolean;
  readonly recordArgsTo?: string;
  readonly recordCwdTo?: string;
  readonly stderr?: string;
} = {}): string => {
  const directory = mkdtempSync(join(tmpdir(), "codex-"));
  const script = join(directory, "codex.mjs");

  writeFileSync(
    script,
    `
import { appendFileSync } from "node:fs";
const recordTo = ${JSON.stringify(options.recordArgsTo ?? null)};
const recordCwd = ${JSON.stringify(options.recordCwdTo ?? null)};
if (recordTo) appendFileSync(recordTo, process.argv.slice(2).join(" ") + "\\n");
if (recordCwd) appendFileSync(recordCwd, process.cwd() + "\\n");
if (${options.hang === true}) { setInterval(() => {}, 1000); }
${(options.lines ?? [])
  .map((line) => `process.stdout.write(${JSON.stringify(`${line}\n`)});`)
  .join("\n")}
${options.stderr === undefined ? "" : `process.stderr.write(${JSON.stringify(options.stderr)});`}
${options.hang === true ? "" : `process.exit(${options.exitCode ?? 0});`}
`,
  );

  const shim = join(directory, "codex-stub");
  writeFileSync(shim, `#!/bin/sh\nexec ${process.execPath} ${script} "$@"\n`);
  chmodSync(shim, 0o755);
  return shim;
};

const collect = async (
  harness: CodexHarness,
  cwd = process.cwd(),
): Promise<ReadonlyArray<DelegateEvent>> => {
  const events: DelegateEvent[] = [];
  const transcriptPath = join(mkdtempSync(join(tmpdir(), "codex-t-")), "child.jsonl");
  for await (const event of harness.run(
    { prompt: "do the thing", cwd, transcriptPath },
    new AbortController().signal,
  )) {
    events.push(event);
  }
  return events;
};

describe("parseCodexEvent", () => {
  it("ignores blank lines, prose, malformed JSON, and unknown types", () => {
    expect(parseCodexEvent("")).toBeUndefined();
    expect(parseCodexEvent("starting…")).toBeUndefined();
    expect(parseCodexEvent('{"type":"future"}')).toBeUndefined();
    expect(parseCodexEvent('{"type":"assistan')).toBeUndefined();
  });

  it("reads a thread.started as a session", () => {
    expect(
      parseCodexEvent(
        JSON.stringify({
          type: "thread.started",
          thread_id: "t_01",
          model: "o3",
        }),
      ),
    ).toEqual({ type: "session", sessionId: "t_01" });
  });

  it("falls back to id and session_id for the session", () => {
    expect(
      parseCodexEvent(JSON.stringify({ type: "thread.started", id: "t_02" })),
    ).toEqual({ type: "session", sessionId: "t_02" });
    expect(
      parseCodexEvent(
        JSON.stringify({ type: "thread.started", session_id: "t_03" }),
      ),
    ).toEqual({ type: "session", sessionId: "t_03" });
  });

  it("reads an assistant message as text", () => {
    expect(
      parseCodexEvent(
        JSON.stringify({
          type: "item.started",
          item: { role: "assistant", content: { text: "Done." } },
        }),
      ),
    ).toEqual({ type: "text", value: "Done." });
  });

  it("reads a completed assistant item output as text", () => {
    expect(
      parseCodexEvent(
        JSON.stringify({
          type: "item.completed",
          item: { role: "assistant", output: { text: "Done." } },
        }),
      ),
    ).toEqual({ type: "text", value: "Done." });
  });

  it("reads a function tool item", () => {
    const line = JSON.stringify({
      type: "item.started",
      item: {
        type: "function",
        id: "call_1",
        function: {
          name: "bash",
          arguments: { command: "ls -la" },
        },
      },
    });
    expect(parseCodexEvent(line)).toEqual({
      type: "tool",
      callId: "call_1",
      name: "bash",
      target: "ls -la",
    });
  });

  it("reads a file tool item", () => {
    const line = JSON.stringify({
      type: "item.completed",
      item: {
        type: "file",
        id: "f_1",
        file: "/tmp/x",
      },
    });
    expect(parseCodexEvent(line)).toEqual({
      type: "tool",
      callId: "f_1",
      name: "file",
      target: "/tmp/x",
    });
  });

  it("reads token usage from a turn.completed", () => {
    expect(
      parseCodexEvent(
        JSON.stringify({
          type: "turn.completed",
          turn_id: "turn_1",
          usage: { input_tokens: 150, output_tokens: 30 },
        }),
      ),
    ).toEqual({ type: "tokens", input: 150, output: 30 });
  });

  it("reads an error event", () => {
    expect(
      parseCodexEvent(
        JSON.stringify({
          type: "error",
          error: { message: "Rate limit" },
        }),
      ),
    ).toEqual({ type: "error", message: "Rate limit" });
  });
});

describe("running children on the Codex CLI", () => {
  it("names itself in the fleet, so a Codex child is not mistaken for the others", () => {
    const harness = new CodexHarness();
    expect(harness.agent).toBe("codex");
    expect(harness.model).toBe("not reported");
  });

  it("reports the child's answer as text", async () => {
    const events = await collect(
      new CodexHarness({
        command: fakeCodex({
          lines: [
            JSON.stringify({
              type: "thread.started",
              thread_id: "t_01",
              model: "o3",
            }),
            JSON.stringify({
              type: "item.started",
              item: { role: "assistant", content: { text: "PONG" } },
            }),
            JSON.stringify({
              type: "turn.completed",
              turn_id: "turn_1",
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
      new CodexHarness({
        command: fakeCodex({
          lines: [
            JSON.stringify({
              type: "thread.started",
              thread_id: "t_01",
            }),
            JSON.stringify({
              type: "item.started",
              item: {
                type: "function",
                id: "call_1",
                function: {
                  name: "bash",
                  arguments: { command: "ls -la" },
                },
              },
            }),
            JSON.stringify({
              type: "turn.completed",
              turn_id: "turn_1",
              usage: { input_tokens: 100, output_tokens: 20 },
            }),
          ],
        }),
      }),
    );

    expect(events).toContainEqual({
      type: "tool",
      callId: "call_1",
      name: "bash",
      target: "ls -la",
    });
    expect(events).toContainEqual({ type: "tokens", input: 100, output: 20 });
  });

  it("reports the session, so a retry can resume it", async () => {
    const events = await collect(
      new CodexHarness({
        command: fakeCodex({
          lines: [
            JSON.stringify({
              type: "thread.started",
              thread_id: "t_01",
            }),
            JSON.stringify({
              type: "turn.completed",
              turn_id: "turn_1",
              usage: { input_tokens: 5, output_tokens: 1 },
            }),
          ],
        }),
      }),
    );

    expect(events[0]).toEqual({ type: "session", sessionId: "t_01" });
  });

  it("writes the raw stdout and stderr to the transcript as they arrive", async () => {
    const transcriptPath = join(mkdtempSync(join(tmpdir(), "codex-t-")), "child.jsonl");
    const harness = new CodexHarness({
      command: fakeCodex({
        lines: [
          JSON.stringify({
            type: "item.started",
            item: { role: "assistant", content: { text: "ok" } },
          }),
        ],
        stderr: "noisy",
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
    expect(written).toContain("noisy");
  });

  it("uses exec, json, the prompt, and the workspace-write sandbox by default", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "codex-args-")), "log");
    await collect(
      new CodexHarness({
        command: fakeCodex({
          recordArgsTo: log,
          exitCode: 0,
          lines: [
            JSON.stringify({
              type: "turn.completed",
              turn_id: "turn_1",
              usage: { input_tokens: 1, output_tokens: 1 },
            }),
          ],
        }),
      }),
    );

    const said = readFileSync(log, "utf8");
    expect(said).toContain("exec");
    expect(said).toContain("--json");
    expect(said).toContain("do the thing");
    expect(said).toContain("--sandbox workspace-write");
  });

  it("passes the model through when the lane names one", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "codex-args-")), "log");
    const harness = new CodexHarness({
      command: fakeCodex({
        recordArgsTo: log,
        exitCode: 0,
        lines: [
          JSON.stringify({
            type: "turn.completed",
            turn_id: "turn_1",
            usage: { input_tokens: 1, output_tokens: 1 },
          }),
        ],
      }),
      model: "o3",
    });

    await collect(harness);

    expect(harness.model).toBe("o3");
    expect(readFileSync(log, "utf8")).toContain("-m o3");
  });

  it("passes the cwd through as the child's working directory", async () => {
    const cwdLog = join(mkdtempSync(join(tmpdir(), "codex-cwd-")), "log");
    const otherCwd = mkdtempSync(join(tmpdir(), "codex-cwd-"));
    await collect(
      new CodexHarness({
        command: fakeCodex({
          recordCwdTo: cwdLog,
          exitCode: 0,
          lines: [
            JSON.stringify({
              type: "turn.completed",
              turn_id: "turn_1",
              usage: { input_tokens: 1, output_tokens: 1 },
            }),
          ],
        }),
      }),
      otherCwd,
    );

    expect(readFileSync(cwdLog, "utf8").trim()).toBe(realpathSync(otherCwd));
  });

  it("resumes with exec resume and the session id", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "codex-resume-")), "log");
    const harness = new CodexHarness({
      command: fakeCodex({
        recordArgsTo: log,
        exitCode: 0,
        lines: [
          JSON.stringify({
            type: "turn.completed",
            turn_id: "turn_1",
            usage: { input_tokens: 1, output_tokens: 1 },
          }),
        ],
      }),
    });

    const events: DelegateEvent[] = [];
    const transcriptPath = join(mkdtempSync(join(tmpdir(), "codex-t-")), "child.jsonl");
    for await (const event of harness.run(
      {
        prompt: "do the thing",
        cwd: process.cwd(),
        transcriptPath,
        resumeSessionId: "t_resume",
      },
      new AbortController().signal,
    )) {
      events.push(event);
    }

    expect(events.length).toBeGreaterThan(0);
    const said = readFileSync(log, "utf8");
    expect(said).toContain("exec resume");
    expect(said).toContain("t_resume");
    expect(said).toContain("do the thing");
  });

  it("reports a Codex result error as a failed child", async () => {
    const harness = new CodexHarness({
      command: fakeCodex({
        lines: [
          JSON.stringify({
            type: "error",
            error: { message: "the model provider is unavailable" },
          }),
        ],
        exitCode: 1,
      }),
    });

    await expect(collect(harness)).rejects.toThrow(/model provider is unavailable/);
  });

  it("throws when the binary is not on PATH, rather than reporting an empty child", async () => {
    await expect(
      collect(new CodexHarness({ command: "codex-does-not-exist" })),
    ).rejects.toThrow("not on the path");
  });

  it("stops when the fleet is stopped", async () => {
    const harness = new CodexHarness({
      command: fakeCodex({
        hang: true,
        lines: [
          JSON.stringify({
            type: "thread.started",
            thread_id: "t_01",
          }),
        ],
      }),
    });
    const controller = new AbortController();
    const events: DelegateEvent[] = [];
    const transcriptPath = join(mkdtempSync(join(tmpdir(), "codex-t-")), "child.jsonl");

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
