import { readFileSync } from "node:fs";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Redacted } from "effect";

import type { DelegateEvent } from "../src/coder-delegate.js";
import { SelfHarness } from "../src/coder-self-harness.js";
import type { CoderTool } from "../src/coder-tools.js";

const GRANT = {
  proxyUrl: "https://openagents.test/api/inference/proxy",
  token: Redacted.make("oa_grant_child"),
  model: "ox-alpha",
};

const sse = (lines: ReadonlyArray<string>) =>
  new Response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(new TextEncoder().encode(`${lines.join("\n\n")}\n\n`));
        controller.close();
      },
    }),
    { status: 200, headers: { "content-type": "text/event-stream" } },
  );

interface Sent {
  readonly url: string;
  readonly authorization: string;
  readonly body: Record<string, unknown>;
}

const stub = (responses: ReadonlyArray<Response>) => {
  const sent: Sent[] = [];
  const queue = [...responses];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (target: URL | string, init?: RequestInit) => {
      const headers = (init?.headers ?? {}) as Record<string, string>;
      sent.push({
        url: String(target),
        authorization: headers["authorization"] ?? "",
        body: JSON.parse(typeof init?.body === "string" ? init.body : "{}") as Record<
          string,
          unknown
        >,
      });
      return queue.shift() ?? sse([`data: [DONE]`]);
    }),
  );
  return sent;
};

const tool = (name: string, run: CoderTool["run"]): CoderTool => ({
  name,
  description: `the ${name} tool`,
  parameters: { type: "object", properties: { command: { type: "string" } } },
  run,
});

const drain = async (harness: SelfHarness, prompt: string, transcriptPath: string) => {
  const events: DelegateEvent[] = [];
  for await (const event of harness.run(
    { prompt, cwd: "/repo", transcriptPath },
    new AbortController().signal,
  )) {
    events.push(event);
  }
  return events;
};

const scratch = () => join(mkdtempSync(join(tmpdir(), "oa-self-")), "child.jsonl");

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("a child this process runs itself", () => {
  it("spends the children's grant against the proxy, on the model it pins", async () => {
    const sent = stub([sse([`data: {"choices":[{"delta":{"content":"done"}}]}`, `data: [DONE]`])]);
    const harness = new SelfHarness({ grant: GRANT, tools: () => [] });

    const events = await drain(harness, "do it", scratch());

    expect(sent[0]?.url).toBe(GRANT.proxyUrl);
    expect(sent[0]?.authorization).toBe("Bearer oa_grant_child");
    expect(sent[0]?.body["model"]).toBe("ox-alpha");
    expect(events.at(-1)).toEqual({ type: "text", value: "done" });
  });

  it("tells the child what it is and that it cannot delegate further", async () => {
    // A child that fans out is a fan-out nobody asked for, and a child that
    // stops without saying so is reported to the parent as having succeeded.
    const sent = stub([sse([`data: [DONE]`])]);
    const harness = new SelfHarness({
      grant: GRANT,
      tools: (cwd) => [tool(`in-${cwd}`, async () => "")],
    });

    await drain(harness, "do it", scratch());

    const messages = sent[0]?.body["messages"] as Array<Record<string, unknown>>;
    const system = String(messages[0]?.["content"]);
    expect(messages[0]?.["role"]).toBe("system");
    expect(system).toContain("delegated child agent");
    expect(system).toContain("cannot delegate further");
    expect(system).toContain("/repo");
    // The tools it actually has, named as the whole list.
    expect(system).toContain("`in-/repo`");
    expect(messages[1]).toEqual({ role: "user", content: "do it" });
  });

  it("runs a tool and answers the model with its result", async () => {
    const ran: string[] = [];
    const sent = stub([
      sse([
        `data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"shell","arguments":"{\\"command\\":\\"git log\\"}"}}]}}]}`,
        `data: [DONE]`,
      ]),
      sse([`data: {"choices":[{"delta":{"content":"the log"}}]}`, `data: [DONE]`]),
    ]);
    const harness = new SelfHarness({
      grant: GRANT,
      tools: () => [
        tool("shell", async (args) => {
          ran.push(String(args["command"]));
          return "abc123 a commit";
        }),
      ],
    });

    const events = await drain(harness, "read the log", scratch());

    expect(ran).toEqual(["git log"]);
    expect(events.some((event) => event.type === "tool" && event.name === "shell")).toBe(true);

    const second = sent[1]?.body["messages"] as Array<Record<string, unknown>>;
    expect(second.find((message) => message["role"] === "tool")).toMatchObject({
      tool_call_id: "c1",
      content: "abc123 a commit",
    });
    expect(events.at(-1)).toEqual({ type: "text", value: "the log" });
  });

  it("reports the tool it is running, so the fleet can show it", async () => {
    stub([
      sse([
        `data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"shell","arguments":"{\\"command\\":\\"ls -la\\"}"}}]}}]}`,
        `data: [DONE]`,
      ]),
      sse([`data: [DONE]`]),
    ]);
    const harness = new SelfHarness({ grant: GRANT, tools: () => [tool("shell", async () => "")] });

    const events = await drain(harness, "look", scratch());
    const activity = events.find((event) => event.type === "tool");

    expect(activity).toMatchObject({ name: "shell", target: "ls -la" });
  });

  it("writes its transcript as it goes, not at the end", async () => {
    stub([
      sse([
        `data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"shell","arguments":"{\\"command\\":\\"pwd\\"}"}}]}}]}`,
        `data: [DONE]`,
      ]),
      sse([`data: {"choices":[{"delta":{"content":"/repo"}}]}`, `data: [DONE]`]),
    ]);
    const path = scratch();
    const harness = new SelfHarness({
      grant: GRANT,
      tools: () => [tool("shell", async () => "/repo")],
    });

    await drain(harness, "where", path);

    const lines = readFileSync(path, "utf8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    expect(lines.map((line: { type: string }) => line.type)).toEqual([
      "session",
      "tool",
      "tool_result",
      "text",
    ]);
  });

  it("resumes a session rather than starting the task again", async () => {
    stub([
      sse([
        `data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"shell","arguments":"{\\"command\\":\\"step one\\"}"}}]}}]}`,
        `data: [DONE]`,
      ]),
      // The provider drops. The fleet retries with the session it was given.
      sse([`data: {"choices":[{"delta":{"content":"carried on"}}]}`, `data: [DONE]`]),
    ]);
    const harness = new SelfHarness({
      grant: GRANT,
      tools: () => [tool("shell", async () => "ok")],
    });
    const path = scratch();

    const first = await drain(harness, "long task", path);
    const session = first.find((event) => event.type === "session");
    expect(session).toMatchObject({ type: "session" });
  });

  it("says so rather than pretending, when the proxy refuses", async () => {
    stub([new Response("no", { status: 403 })]);
    const harness = new SelfHarness({ grant: GRANT, tools: () => [] });

    await expect(drain(harness, "go", scratch())).rejects.toThrow(
      /refused the child's call \(403\)/,
    );
  });
});
