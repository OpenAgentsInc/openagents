import * as BunContext from "@effect/platform-bun/BunContext";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { bashTool } from "./bash.js";
import { runTool, ToolExecutionError } from "./schema.js";

const runWithBun = <A>(program: Effect.Effect<A>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("bashTool", () => {
  it("runs a simple command", async () => {
    const result = await runWithBun(runTool(bashTool, { command: "echo hello" }));
    expect(result.content[0]?.text.trim()).toBe("hello");
  });

  it("fails on non-zero exit", async () => {
    const error = await runWithBun(runTool(bashTool, { command: "exit 3" }).pipe(Effect.flip));
    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("command_failed");
  });

  it("times out long commands", async () => {
    const error = await runWithBun(
      runTool(bashTool, { command: "sleep 2", timeout: 0.1 }).pipe(Effect.flip),
    );
    expect(error).toBeInstanceOf(ToolExecutionError);
    expect((error as ToolExecutionError).reason).toBe("aborted");
  });
});
