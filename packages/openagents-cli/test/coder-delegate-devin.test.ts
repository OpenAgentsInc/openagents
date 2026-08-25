import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { DevinHarness, type DelegateEvent } from "../src/coder-delegate.js";

/**
 * A stand-in for `devin acp`, so the tests cost nothing and never call out.
 *
 * It speaks the protocol the harness speaks: newline-delimited JSON-RPC on
 * stdio, replying to `initialize`, `session/new`, and `session/prompt`, and
 * sending whatever `session/update` notifications the test asks for.
 */
const fakeAgent = (
  options: {
    readonly updates?: ReadonlyArray<Record<string, unknown>>;
    readonly answer?: string;
    readonly failPrompt?: string;
    readonly hang?: boolean;
    readonly recordArgsTo?: string;
  } = {},
): string => {
  const directory = mkdtempSync(join(tmpdir(), "devin-acp-"));
  const script = join(directory, "agent.mjs");

  writeFileSync(
    script,
    `
import { appendFileSync } from "node:fs";
const recordTo = ${JSON.stringify(options.recordArgsTo ?? null)};
if (recordTo) appendFileSync(recordTo, process.argv.slice(2).join(" ") + "\\n");
if (${options.hang === true}) { setInterval(() => {}, 1000); }

let buffer = "";
const write = (m) => process.stdout.write(JSON.stringify(m) + "\\n");
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => {
  buffer += chunk;
  for (;;) {
    const at = buffer.indexOf("\\n");
    if (at === -1) break;
    const line = buffer.slice(0, at).trim();
    buffer = buffer.slice(at + 1);
    if (!line) continue;
    const m = JSON.parse(line);
    if (recordTo) appendFileSync(recordTo, m.method + " " + JSON.stringify(m.params ?? {}) + "\\n");
    if (m.method === "initialize") write({ jsonrpc: "2.0", id: m.id, result: { protocolVersion: 1 } });
    else if (m.method === "session/new") write({ jsonrpc: "2.0", id: m.id, result: { sessionId: "ses_test" } });
    else if (m.method === "session/set_mode") write({ jsonrpc: "2.0", id: m.id, result: {} });
    else if (m.method === "session/prompt") {
      for (const update of ${JSON.stringify(options.updates ?? [])})
        write({ jsonrpc: "2.0", method: "session/update", params: { sessionId: "ses_test", update } });
      ${
        options.answer === undefined
          ? ""
          : `write({ jsonrpc: "2.0", method: "session/update", params: { sessionId: "ses_test", update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: ${JSON.stringify(options.answer)} } } } });`
      }
      ${
        options.failPrompt === undefined
          ? `write({ jsonrpc: "2.0", id: m.id, result: { stopReason: "end_turn" } });`
          : `write({ jsonrpc: "2.0", id: m.id, error: { code: -32000, message: ${JSON.stringify(options.failPrompt)} } });`
      }
      if (!${options.hang === true}) setTimeout(() => process.exit(0), 10);
    }
  }
});
`,
  );

  const shim = join(directory, "devin-stub");
  writeFileSync(shim, `#!/bin/sh\nexec ${process.execPath} ${script} "$@"\n`);
  chmodSync(shim, 0o755);
  return shim;
};

const collect = async (
  harness: DevinHarness,
  cwd = process.cwd(),
): Promise<ReadonlyArray<DelegateEvent>> => {
  const events: DelegateEvent[] = [];
  const transcriptPath = join(mkdtempSync(join(tmpdir(), "devin-t-")), "child.jsonl");
  for await (const event of harness.run(
    { prompt: "do the thing", cwd, transcriptPath },
    new AbortController().signal,
  )) {
    events.push(event);
  }
  return events;
};

describe("running children on the Devin CLI", () => {
  it("names itself in the fleet, so a Devin child is not mistaken for an opencode one", () => {
    // Its credentials and its billing are not this session's, which is why the
    // agent is named rather than left implicit.
    expect(new DevinHarness().agent).toBe("devin");
  });

  it("reports the child's answer as text", async () => {
    const events = await collect(new DevinHarness({ command: fakeAgent({ answer: "PONG" }) }));

    expect(events.at(-1)).toEqual({ type: "text", value: "PONG" });
  });

  it("reports what the child is doing while it does it", async () => {
    // The whole reason this harness speaks ACP. In print mode a child doing
    // four minutes of work reported nothing at all until it finished, so the
    // fleet showed `Initializing…` for the entire run and a reader could not
    // tell it from a hang.
    const events = await collect(
      new DevinHarness({
        command: fakeAgent({
          updates: [
            {
              sessionUpdate: "tool_call",
              toolCallId: "functions.exec:0",
              title: "Ran ls",
              kind: "execute",
            },
            {
              sessionUpdate: "usage_update",
              used: 16_086,
              _meta: { "cognition.ai/inputTokens": 16_020, "cognition.ai/outputTokens": 66 },
            },
          ],
          answer: "done",
        }),
      }),
    );

    expect(events).toContainEqual({
      type: "tool",
      callId: "functions.exec:0",
      name: "execute",
      target: "Ran ls",
    });
    expect(events).toContainEqual({ type: "tokens", input: 16_020, output: 66 });
    expect(events.at(-1)).toEqual({ type: "text", value: "done" });
  });

  it("reports the session, so a retry can resume it", async () => {
    const events = await collect(new DevinHarness({ command: fakeAgent({ answer: "x" }) }));

    expect(events[0]).toEqual({ type: "session", sessionId: "ses_test" });
  });

  it("writes the protocol to the transcript as it arrives", async () => {
    const transcriptPath = join(mkdtempSync(join(tmpdir(), "devin-t-")), "child.jsonl");
    const harness = new DevinHarness({
      command: fakeAgent({
        updates: [
          { sessionUpdate: "tool_call", toolCallId: "c1", title: "Read a.ts", kind: "read" },
        ],
        answer: "ok",
      }),
    });

    for await (const _event of harness.run(
      { prompt: "x", cwd: process.cwd(), transcriptPath },
      new AbortController().signal,
    )) {
      void _event;
    }

    // A child that is killed still leaves what it had done behind, and opening
    // a running child shows something rather than an empty screen.
    const written = readFileSync(transcriptPath, "utf8");
    expect(written).toContain("Read a.ts");
  });

  it("asks for the unattended session mode, in Devin's own word for it", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "devin-args-")), "log");
    const harness = new DevinHarness({
      command: fakeAgent({ answer: "x", recordArgsTo: log }),
      permissionMode: "dangerous",
    });

    await collect(harness);

    const said = readFileSync(log, "utf8");
    expect(said).toContain("acp");
    // `bypass` is ACP's name for what the flag called `dangerous`. A caller
    // that wrote the old name should not have to learn the new one.
    expect(said).toContain(`"modeId":"bypass"`);
    // The mode is what the fleet shows beside the agent, because neither print
    // mode nor ACP reports which model answered.
    expect(harness.model).toBe("dangerous");
  });

  it("carries the prompt through as a content block", async () => {
    const log = join(mkdtempSync(join(tmpdir(), "devin-args-")), "log");
    await collect(new DevinHarness({ command: fakeAgent({ answer: "x", recordArgsTo: log }) }));

    expect(readFileSync(log, "utf8")).toContain(`"text":"do the thing"`);
  });

  it("reports a refused prompt as an error rather than an empty child", async () => {
    const harness = new DevinHarness({
      command: fakeAgent({ failPrompt: "the model provider is unavailable" }),
    });

    await expect(collect(harness)).rejects.toThrow(/model provider is unavailable/);
  });

  it("throws when the binary is not on PATH, rather than reporting an empty child", async () => {
    // Refused once for the fleet, not once per child.
    await expect(collect(new DevinHarness({ command: "devin-does-not-exist" }))).rejects.toThrow(
      "not on PATH",
    );
  });

  it("stops when the fleet is stopped", async () => {
    const harness = new DevinHarness({ command: fakeAgent({ hang: true }) });
    const controller = new AbortController();
    const events: DelegateEvent[] = [];
    const transcriptPath = join(mkdtempSync(join(tmpdir(), "devin-t-")), "child.jsonl");

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

    // Killed rather than left running: a child holds a process, and a console
    // that exits while children keep spending leaves nothing to stop them with.
    expect(events.every((event) => event.type !== "text")).toBe(true);
  });
});
