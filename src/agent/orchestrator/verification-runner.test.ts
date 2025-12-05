import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { runVerificationOnHost } from "./verification-runner.js";

describe("verification-runner", () => {
  test("streams stdout and returns structured results", async () => {
    const events: any[] = [];
    const result = await Effect.runPromise(
      runVerificationOnHost(
        ["echo hello"],
        process.cwd(),
        (event) => events.push(event),
      ),
    );

    expect(result.passed).toBe(true);
    expect(result.outputs[0].trim()).toBe("hello");
    expect(result.results[0].exitCode).toBe(0);

    const outputEvents = events.filter((e) => e.type === "verification_output");
    expect(outputEvents.length).toBeGreaterThan(0);
    expect(outputEvents[0].chunk).toContain("hello");
  });

  test("captures failures with exit codes", async () => {
    const result = await Effect.runPromise(
      runVerificationOnHost(
        ['sh -c "echo err 1>&2; exit 2"'],
        process.cwd(),
        () => {},
      ),
    );

    expect(result.passed).toBe(false);
    expect(result.results[0].exitCode).toBe(2);
    expect(result.results[0].stderr.trim()).toBe("err");
  });
});
