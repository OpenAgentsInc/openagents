import { mkdtempSync, readFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { exportTrajectory } from "../src/coder-export.js";
import type { CoderEntry, CoderSnapshot } from "../src/coder-session.js";

const AT = Date.parse("2026-08-24T14:00:00.000Z");

const entry = (partial: Partial<CoderEntry> & Pick<CoderEntry, "role">): CoderEntry => ({
  text: "",
  settled: true,
  at: AT,
  ...partial,
});

const snapshot = (entries: ReadonlyArray<CoderEntry>): CoderSnapshot =>
  ({
    entries,
    repository: "openagents.com",
    branch: "main",
    model: "Ollama qwen",
    turns: 1,
    running: false,
    tasks: [],
  }) as unknown as CoderSnapshot;

const write = (entries: ReadonlyArray<CoderEntry>) => {
  const directory = mkdtempSync(join(tmpdir(), "coder-export-"));
  const result = exportTrajectory(snapshot(entries), {
    model: "Ollama qwen",
    version: "0.3.5",
    now: new Date(AT),
    directory,
    // A suite must not take the reader's clipboard.
    copy: false,
  });
  return {
    result,
    directory,
    document: JSON.parse(readFileSync(result.path, "utf8")) as Record<string, unknown>,
  };
};

describe("exporting a conversation as ATIF", () => {
  it("writes the envelope the rest of the system reads", () => {
    const { document, result } = write([entry({ role: "you", text: "hello" })]);

    expect(document["schema_version"]).toBe("ATIF-v1.7");
    expect(document["agent"]).toMatchObject({ name: "openagents-coder", version: "0.3.5" });
    expect(document["final_metrics"]).toMatchObject({ total_steps: 1 });
    expect(result.path).toContain("openagents.com-atif.json");
  });

  it("numbers steps from one and names each source", () => {
    const { document } = write([
      entry({ role: "you", text: "ask" }),
      entry({ role: "assistant", text: "answer" }),
    ]);

    expect(document["steps"]).toEqual([
      expect.objectContaining({ step_id: 1, source: "user", message: "ask" }),
      expect.objectContaining({ step_id: 2, source: "agent", message: "answer" }),
    ]);
  });

  it("carries a tool call and its result on one agent step", () => {
    const { document } = write([
      entry({
        role: "tool",
        text: "skill",
        tool: {
          callId: "call-1",
          name: "skill",
          arguments: '{"name":"house-style"}',
          output: "Use sentence case.",
          error: undefined,
          status: "succeeded",
        },
      }),
    ]);

    const [step] = document["steps"] as ReadonlyArray<Record<string, unknown>>;
    expect(step).toMatchObject({
      source: "agent",
      message: "",
      tool_calls: [
        { tool_call_id: "call-1", function_name: "skill", arguments: { name: "house-style" } },
      ],
      observation: { results: [{ source_call_id: "call-1", content: "Use sentence case." }] },
    });
  });

  it("reports a failed call by its error, not as an empty result", () => {
    const { document } = write([
      entry({
        role: "tool",
        text: "delegate",
        tool: {
          callId: "call-2",
          name: "delegate",
          arguments: "{}",
          output: undefined,
          error: "the fleet is full",
          status: "failed",
        },
      }),
    ]);

    const [step] = document["steps"] as ReadonlyArray<Record<string, unknown>>;
    expect(step).toMatchObject({
      observation: { results: [{ content: "the fleet is full" }] },
    });
  });

  it("keeps arguments it cannot parse rather than dropping them", () => {
    const { document } = write([
      entry({
        role: "tool",
        text: "skill",
        tool: {
          callId: "call-3",
          name: "skill",
          arguments: "{not json",
          output: "",
          error: undefined,
          status: "succeeded",
        },
      }),
    ]);

    const [step] = document["steps"] as ReadonlyArray<Record<string, unknown>>;
    // A trajectory that silently loses what a model asked for is worse than one
    // that says it could not read it.
    expect(step).toMatchObject({
      tool_calls: [{ arguments: { unparsed_arguments: "{not json" } }],
    });
  });

  it("attaches reasoning to the step it preceded", () => {
    const { document } = write([
      entry({ role: "reasoning", text: "weighing it" }),
      entry({ role: "assistant", text: "answer" }),
    ]);

    expect(document["steps"]).toEqual([
      expect.objectContaining({ source: "agent", reasoning_content: "weighing it" }),
    ]);
  });

  it("keeps notices out of the steps and in the record", () => {
    const { document } = write([
      entry({ role: "you", text: "ask" }),
      entry({ role: "notice", text: "Delegation refused (quota)." }),
    ]);

    // A notice is the interface talking to the reader. It never reached the
    // model, so it is not a step, but it explains the steps either side of it.
    expect(document["steps"]).toHaveLength(1);
    expect((document["extra"] as Record<string, unknown>)["notices"]).toEqual([
      expect.objectContaining({ text: "Delegation refused (quota)." }),
    ]);
  });

  it("leaves out the commands the interface answers itself", () => {
    const { document } = write([
      entry({ role: "you", text: "/export" }),
      entry({ role: "you", text: "/system" }),
      entry({ role: "you", text: "/skills" }),
      entry({ role: "you", text: "a real question" }),
    ]);

    expect(document["steps"]).toEqual([
      expect.objectContaining({ message: "a real question" }),
    ]);
  });

  it("leaves out the empty entry the interface opens for the caret", () => {
    const { document } = write([
      entry({ role: "assistant", text: "", settled: false }),
      entry({ role: "assistant", text: "real" }),
    ]);

    expect(document["steps"]).toHaveLength(1);
  });


  it("records what a turn cost, and how many calls that was", () => {
    const { document } = write([
      entry({ role: "you", text: "ask" }),
      entry({
        role: "assistant",
        text: "answer",
        metrics: { promptTokens: 2334, completionTokens: 114, calls: 2 },
      }),
    ]);

    const steps = document["steps"] as ReadonlyArray<Record<string, unknown>>;
    expect(steps[1]).toMatchObject({
      metrics: { prompt_tokens: 2334, completion_tokens: 114 },
      // A turn that asked for a tool and then answered is two calls, not one.
      llm_call_count: 2,
    });
    expect(document["final_metrics"]).toMatchObject({
      total_prompt_tokens: 2334,
      total_completion_tokens: 114,
      total_steps: 2,
    });
  });

  it("reports no totals rather than zero when nothing measured any", () => {
    const { document } = write([entry({ role: "assistant", text: "answer" })]);

    // A total of 0 on a session that never measured would be a measurement.
    expect(document["final_metrics"]).toEqual({ total_steps: 1 });
  });


  it("takes the clipboard only when asked to", () => {
    const directory = mkdtempSync(join(tmpdir(), "coder-export-"));

    // The default is a person exporting, and they want the path. Anything else
    // — a test above all — must say so: a suite that took the clipboard once
    // replaced a reader's own export path with one pointing at a file the suite
    // then deleted, and the reader pasted it and was told it did not exist.
    const quiet = exportTrajectory(snapshot([entry({ role: "you", text: "hi" })]), {
      model: "m",
      version: "0",
      now: new Date(AT),
      directory,
      copy: false,
    });

    expect(quiet.copied).toBe(false);
  });

  it("writes one file per export, named so they sort by time", () => {
    const { directory } = write([entry({ role: "you", text: "one" })]);

    const [name] = readdirSync(directory);
    expect(name).toMatch(/^2026-08-24T14-00-00-000Z-openagents\.com-atif\.json$/);
  });
});
