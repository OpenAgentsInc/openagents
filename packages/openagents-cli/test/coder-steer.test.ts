import { describe, expect, it } from "vitest";
import { CoderSession } from "../src/coder-session.js";
import { OllamaReplySource } from "../src/coder-ollama.js";

const chunk = (message: Record<string, unknown>, done = false) => ({ message, done });

describe("steering a running turn", () => {
  it("puts the message in front of the model at its next step", async () => {
    const requests: Record<string, unknown>[] = [];
    let round = 0;
    const gate: Array<() => void> = [];

    const source = new OllamaReplySource({ model: "m" });
    (source as unknown as { client: unknown }).client = {
      chat: async (request: Record<string, unknown>) => {
        requests.push(JSON.parse(JSON.stringify(request)) as Record<string, unknown>);
        const mine = round++;
        // Round 0 asks for a tool and waits, so a steer can arrive mid-turn.
        const pieces =
          mine === 0
            ? [chunk({ content: "", tool_calls: [{ function: { name: "t", arguments: {} } }] }), chunk({}, true)]
            : [chunk({ content: "done" }, true)];
        return Object.assign(
          (async function* () {
            if (mine === 0) await new Promise<void>((r) => gate.push(r));
            for (const p of pieces) yield p;
          })(),
          { abort: () => {} },
        );
      },
    };
    source.useTools([
      { name: "t", description: "d", parameters: {}, run: () => Promise.resolve("ok") },
    ]);

    const session = new CoderSession(source, "repo", "main");
    const turn = session.submit("original question");
    await new Promise((r) => setTimeout(r, 10));

    await session.submit("actually, do it the other way");
    // Not stopped: the turn is still running.
    expect(session.snapshot().running).toBe(true);

    gate.shift()?.();
    await turn;
    await new Promise((r) => setTimeout(r, 20));

    const later = requests.at(-1)?.["messages"] as ReadonlyArray<Record<string, unknown>>;
    const said = later.map((m) => m["content"]);
    expect(said).toContain("actually, do it the other way");
    // It arrived at a later step, not at the start.
    const first = requests[0]?.["messages"] as ReadonlyArray<Record<string, unknown>>;
    expect(first.map((m) => m["content"])).not.toContain("actually, do it the other way");
    expect(session.snapshot().entries.filter((e) => e.role === "notice").map((e) => e.text))
      .toContainEqual(expect.stringContaining("Steering"));
  });
});
