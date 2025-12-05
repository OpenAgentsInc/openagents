import { describe, expect, test, vi } from "bun:test";
import { Effect } from "effect";
import {
  buildVerificationCommands,
  runVerificationCommands,
  runVerificationPipeline,
  shouldRunE2e,
} from "./verification-pipeline.js";

describe("verification-pipeline", () => {
  test("buildVerificationCommands prefers sandbox commands and skips typecheck in sandbox", () => {
    const commands = buildVerificationCommands(
      ["bun run typecheck"],
      ["bun test"],
      ["bun run test:container"],
      true
    );

    expect(commands).toEqual(["bun run test:container"]);
  });

  test("runVerificationCommands returns structured results on host", async () => {
    const emit = vi.fn();
    const commands = ["echo hello", "echo world"];

    const result = await Effect.runPromise(
      runVerificationCommands(commands, { cwd: process.cwd(), emit })
    );

    expect(result.passed).toBe(true);
    expect(result.outputs).toHaveLength(2);
    expect(result.results).toHaveLength(2);
    expect(result.results[0]?.command).toBe("echo hello");
    expect(emit).toHaveBeenCalledWith(expect.objectContaining({ type: "verification_start" }));
  });

  test("runVerificationPipeline runs verification and e2e when configured", async () => {
    const emit = vi.fn();
    const result = await Effect.runPromise(
      runVerificationPipeline({
        typecheckCommands: ["echo typecheck"],
        testCommands: ["echo test"],
        e2eCommands: ["echo e2e"],
        cwd: process.cwd(),
        emit,
        taskLabels: [],
      })
    );

    expect(result.verification.passed).toBe(true);
    expect(result.verification.outputs).toHaveLength(2);
    expect(result.e2e.ran).toBe(true);
    expect(result.e2e.passed).toBe(true);
    expect(result.e2e.outputs).toHaveLength(1);
  });

  test("shouldRunE2e skips when skip label present", () => {
    expect(shouldRunE2e(["skip-e2e"], true)).toBe(false);
    expect(shouldRunE2e(["no-e2e"], true)).toBe(false);
    expect(shouldRunE2e(["unit-only"], true)).toBe(false);
  });

  test("runVerificationPipeline runs verification and e2e commands on host", async () => {
    const result = await Effect.runPromise(
      runVerificationPipeline({
        verificationCommands: ["echo typecheck", "echo test"],
        e2eCommands: ["echo e2e"],
        testCommands: ["echo test"],
        cwd: process.cwd(),
        emit: () => {},
      })
    );

    expect(result.verification.passed).toBe(true);
    expect(result.verification.outputs).toHaveLength(2);
    expect(result.e2e?.ran).toBe(true);
    expect(result.e2e?.passed).toBe(true);
    expect(result.e2e?.outputs).toHaveLength(1);
  });
});
