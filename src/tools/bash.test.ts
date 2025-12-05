import * as BunContext from "@effect/platform-bun/BunContext";
import * as CommandExecutor from "@effect/platform/CommandExecutor";
import { describe, expect, it } from "bun:test";
import { Effect } from "effect";
import { bashTool } from "./bash.js";
import { runTool, ToolExecutionError, isTextContent } from "./schema.js";

const runWithBun = <A, E>(program: Effect.Effect<A, E, CommandExecutor.CommandExecutor>) =>
  Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

describe("bashTool", () => {
  it("runs a simple command", async () => {
    const result = await runWithBun(runTool(bashTool, { command: "echo hello" }));
    const first = result.content.find(isTextContent);
    expect(first?.text.trim()).toBe("hello");
    expect(result.details?.exitCode).toBe(0);
    expect(result.details?.command).toContain("echo hello");
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

  it("starts background command when requested", async () => {
    const result = await runWithBun(runTool(bashTool, { command: "sleep 0.1", run_in_background: true }));
    const text = result.content.find(isTextContent)?.text ?? "";
    expect(text).toContain("background process");
  });
});
