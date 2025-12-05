import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  buildVerificationPlan,
  runVerificationPipeline,
  type VerificationPlan,
} from "./verification-pipeline.js";

describe("verification-pipeline", () => {
  test("buildVerificationPlan uses sandbox test commands and skips typecheck when sandboxed", () => {
    const plan = buildVerificationPlan({
      typecheckCommands: ["bun run typecheck"],
      testCommands: ["bun test"],
      sandboxTestCommands: ["bun run test:container"],
      useSandbox: true,
    });

    expect(plan.verificationCommands).toEqual(["bun run test:container"]);
    expect(plan.useSandbox).toBe(true);
  });

  test("buildVerificationPlan skips e2e when labels opt out", () => {
    const plan = buildVerificationPlan({
      testCommands: ["bun test"],
      e2eCommands: ["bun run e2e:test"],
      taskLabels: ["skip-e2e"],
    });

    expect(plan.runE2e).toBe(false);
  });

  test("runVerificationPipeline prefers sandbox runner when configured", async () => {
    const calls: Array<{ runner: string; commands: string[] }> = [];
    const plan: VerificationPlan = {
      verificationCommands: ["echo ok"],
      e2eCommands: [],
      runE2e: false,
      useSandbox: true,
    };

    const sandboxRun = () => {
      calls.push({ runner: "sandbox", commands: plan.verificationCommands });
      return Effect.succeed({ passed: true, outputs: ["ok"], sandboxed: true });
    };

    const hostRun = () => {
      calls.push({ runner: "host", commands: plan.verificationCommands });
      return Effect.succeed({ passed: true, results: [], outputs: [] });
    };

    const result = await Effect.runPromise(
      runVerificationPipeline(
        {
          plan,
          cwd: process.cwd(),
          emit: () => {},
          sandboxConfig: {
            sandboxConfig: {
              enabled: true,
              backend: "macos-container",
              memoryLimit: "8G",
              timeoutMs: 120000,
            },
            cwd: process.cwd(),
          },
        },
        {
          runSandbox: sandboxRun as any,
          runHost: hostRun as any,
        },
      ),
    );

    expect(result.verification.passed).toBe(true);
    expect(calls[0]?.runner).toBe("sandbox");
    expect(calls.some((c) => c.runner === "host")).toBe(false);
  });

  test("runVerificationPipeline runs verification and e2e commands on host", async () => {
    const plan = buildVerificationPlan({
      typecheckCommands: ["echo typecheck"],
      testCommands: ["echo test"],
      e2eCommands: ["echo e2e"],
    });

    const result = await Effect.runPromise(
      runVerificationPipeline({
        plan,
        cwd: process.cwd(),
        emit: () => {},
      })
    );

    expect(result.verification.passed).toBe(true);
    expect(result.verification.outputs).toHaveLength(plan.verificationCommands.length);
    expect(result.e2e?.ran).toBe(true);
    expect(result.e2e?.passed).toBe(true);
    expect(result.e2e?.outputs).toHaveLength(plan.e2eCommands.length);
  });
});
