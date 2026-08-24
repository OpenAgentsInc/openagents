import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { childHarnessConfig, writeChildHarnessConfig } from "../src/coder-child-config.js";
import { flattenForProxy } from "../src/coder-child-gateway.js";
import { DelegateFleet, MAX_DELEGATE_COUNT, parseOpencodeEvent } from "../src/coder-delegate.js";
import type { DelegateEvent, DelegateHarness } from "../src/coder-delegate.js";
import type { CoderDelegation } from "../src/coder-session.js";
import { CoderTaskRegistry } from "../src/coder-tasks.js";
import { delegateTool } from "../src/coder-tools.js";

const harness = (answer: string, options: { readonly fail?: string } = {}): DelegateHarness => ({
  agent: "fake",
  model: "fake/model",
  async *run(_input, _signal) {
    const events: ReadonlyArray<DelegateEvent> = [{ type: "text", value: answer }];
    for (const event of events) yield event;
    if (options.fail !== undefined) throw new Error(options.fail);
  },
});

const delegationOf = (agent: DelegateHarness): CoderDelegation => {
  const registry = new CoderTaskRegistry();
  const transcriptDirectory = mkdtempSync(join(tmpdir(), "tools-test-"));
  return {
    registry,
    fleet: new DelegateFleet(registry, agent, { maxConcurrent: 4, transcriptDirectory }),
    label: "fake (fake/model)",
  };
};

describe("delegateTool", () => {
  it("declares a schema whose only required field is the prompt", () => {
    const tool = delegateTool(delegationOf(harness("done")));
    expect(tool.name).toBe("delegate");
    expect(tool.parameters["required"]).toEqual(["prompt"]);
    // The description is what makes the model reach for it unprompted, so it
    // has to say that children are parallel and start with no context.
    expect(tool.description).toMatch(/parallel/);
    expect(tool.description).toMatch(/no context/);
  });

  it("refuses without a prompt in a sentence the model can act on", async () => {
    const tool = delegateTool(delegationOf(harness("done")));
    const output = await tool.run({}, new AbortController().signal);
    expect(output).toMatch(/`prompt` is required/);
  });

  it("runs the requested number of children and reports each one", async () => {
    const delegation = delegationOf(harness("wrote the file"));
    const tool = delegateTool(delegation);

    const output = await tool.run(
      { prompt: "write a file", count: 3, description: "write files" },
      new AbortController().signal,
    );

    expect(output).toMatch(/^3 of 3 children completed on fake \(fake\/model\)\./);
    expect(output.match(/completed:/g)).toHaveLength(3);
    expect(output).toMatch(/wrote the file/);
    expect(delegation.registry.list()).toHaveLength(3);
  });

  it("tells each child of a fan-out which one it is", async () => {
    const prompts: string[] = [];
    const delegation = delegationOf({
      agent: "fake",
      model: "fake/model",
      async *run(input, _signal) {
        prompts.push(input.prompt);
        yield { type: "text", value: "ok" };
      },
    });

    await delegateTool(delegation).run(
      { prompt: "write your own numbered file", count: 2 },
      new AbortController().signal,
    );

    expect(prompts).toEqual([
      "You are child 1 of 2.\n\nwrite your own numbered file",
      "You are child 2 of 2.\n\nwrite your own numbered file",
    ]);
  });

  it("leaves a lone child's prompt exactly as the model wrote it", async () => {
    const prompts: string[] = [];
    const delegation = delegationOf({
      agent: "fake",
      model: "fake/model",
      async *run(input, _signal) {
        prompts.push(input.prompt);
        yield { type: "text", value: "ok" };
      },
    });

    await delegateTool(delegation).run({ prompt: "run the tests" }, new AbortController().signal);
    expect(prompts).toEqual(["run the tests"]);
  });

  it("gives the model the child's own failure sentence", async () => {
    const tool = delegateTool(delegationOf(harness("", { fail: "provider refused the key" })));
    const output = await tool.run({ prompt: "write a file" }, new AbortController().signal);
    expect(output).toMatch(/0 of 1 child completed/);
    expect(output).toMatch(/failed: provider refused the key/);
  });

  it("clamps a count the model overshot rather than launching it", async () => {
    const delegation = delegationOf(harness("ok"));
    await delegateTool(delegation).run(
      { prompt: "write a file", count: 999 },
      new AbortController().signal,
    );
    expect(delegation.registry.list().length).toBe(MAX_DELEGATE_COUNT);
  });

  it("stops the fleet when the turn is interrupted", async () => {
    const delegation = delegationOf({
      agent: "fake",
      model: "fake/model",
      async *run(_input, signal) {
        yield { type: "text", value: "starting" };
        await new Promise<void>((resolve) => {
          signal.addEventListener("abort", () => resolve(), { once: true });
        });
      },
    });
    const controller = new AbortController();
    const running = delegateTool(delegation).run({ prompt: "wait" }, controller.signal);
    // Give the child a turn of the loop to start before interrupting it.
    await new Promise((resolve) => setTimeout(resolve, 10));
    controller.abort();
    const output = await running;
    expect(output).toMatch(/stopped before finishing/);
  });
});

describe("harness error reporting", () => {
  it("reads the sentence opencode nests under error.data", () => {
    const event = parseOpencodeEvent(
      JSON.stringify({
        type: "error",
        error: {
          name: "UnknownError",
          data: { message: "Unexpected server error.", ref: "err_089" },
        },
      }),
    );
    expect(event).toEqual({
      type: "error",
      message: "UnknownError: Unexpected server error. (err_089)",
    });
  });

  it("falls back to a sentence rather than reporting an empty error", () => {
    expect(parseOpencodeEvent(JSON.stringify({ type: "error" }))).toEqual({
      type: "error",
      message: "the child agent reported an error",
    });
  });
});

describe("child harness config", () => {
  it("points the child at the gateway and carries no credential", () => {
    const config = childHarnessConfig({ baseUrl: "http://127.0.0.1:1/v1", model: "luna" });
    const provider = (config["provider"] as Record<string, Record<string, unknown>>)["openagents"];
    expect((provider?.["options"] as Record<string, unknown>)["baseURL"]).toBe(
      "http://127.0.0.1:1/v1",
    );
    expect(JSON.stringify(config)).not.toMatch(/Bearer|oa_/);
    expect(Object.keys(provider?.["models"] as Record<string, unknown>)).toEqual(["luna"]);
  });

  it("writes a private file and removes it again", () => {
    const file = writeChildHarnessConfig({ baseUrl: "http://127.0.0.1:1/v1", model: "luna" });
    expect(JSON.parse(readFileSync(file.path, "utf8"))).toEqual(
      childHarnessConfig({ baseUrl: "http://127.0.0.1:1/v1", model: "luna" }),
    );
    file.remove();
    expect(() => readFileSync(file.path, "utf8")).toThrow();
  });
});

describe("flattenForProxy", () => {
  it("turns a tool exchange into turns the proxy accepts", () => {
    expect(
      flattenForProxy([
        { role: "system", content: "be brief" },
        { role: "user", content: "weather in Tokyo" },
        {
          role: "assistant",
          content: "",
          tool_calls: [
            { id: "call_1", function: { name: "get_weather", arguments: '{"city":"Tokyo"}' } },
          ],
        },
        { role: "tool", tool_call_id: "call_1", content: "18C" },
      ]),
    ).toEqual([
      { role: "system", content: "be brief" },
      { role: "user", content: "weather in Tokyo" },
      { role: "assistant", content: '[tool call]\nget_weather({"city":"Tokyo"}) id=call_1' },
      { role: "user", content: "[tool result call_1]\n18C" },
    ]);
  });

  it("reads part arrays and drops the empty turns a harness emits", () => {
    expect(
      flattenForProxy([
        {
          role: "user",
          content: [
            { type: "text", text: "one" },
            { type: "text", text: " two" },
          ],
        },
        { role: "assistant", content: "   " },
      ]),
    ).toEqual([{ role: "user", content: "one two" }]);
  });
});
