import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";
import { makeLocalSandboxProvider } from "./local-sandbox-provider.ts";
import { makeLocalProcessSandboxProvider } from "./local-process-sandbox-provider.ts";
import { join } from "node:path";
import { tmpdir } from "node:os";

describe("agent-harness-contract sandbox environments", () => {
  const testEnvironments = [
    {
      id: "env-development",
      env: {
        APP_STAGE: "development",
        REUSABLE_VAR: "base-value",
        SHARED_SECRET: "reusable-secret",
      },
    },
    {
      id: "env-production",
      env: {
        APP_STAGE: "production",
        REUSABLE_VAR: "prod-value",
        SHARED_SECRET: "production-secret",
      },
    },
  ];

  describe("makeLocalSandboxProvider", () => {
    test("inherits from selected environment", () => {
      const provider = makeLocalSandboxProvider();
      const run = provider.createSession({
        sessionId: "test-session-dev",
        environmentId: "env-development",
        environments: testEnvironments,
      });

      const session = Effect.runSync(run);
      expect(session.env).toBeDefined();
      expect(session.env?.APP_STAGE).toBe("development");
      expect(session.env?.REUSABLE_VAR).toBe("base-value");
      expect(session.env?.SHARED_SECRET).toBe("reusable-secret");
    });

    test("inline env overrides selected environment", () => {
      const provider = makeLocalSandboxProvider();
      const run = provider.createSession({
        sessionId: "test-session-override",
        environmentId: "env-development",
        environments: testEnvironments,
        env: {
          REUSABLE_VAR: "overridden-value",
          INLINE_VAR: "inline-only",
        },
      });

      const session = Effect.runSync(run);
      expect(session.env).toBeDefined();
      expect(session.env?.APP_STAGE).toBe("development");
      expect(session.env?.REUSABLE_VAR).toBe("overridden-value");
      expect(session.env?.SHARED_SECRET).toBe("reusable-secret");
      expect(session.env?.INLINE_VAR).toBe("inline-only");
    });

    test("resuming preserves environment variables", () => {
      const provider = makeLocalSandboxProvider();
      const createRun = provider.createSession({
        sessionId: "test-session-resume",
        environmentId: "env-production",
        environments: testEnvironments,
        env: {
          EXTRA_VAR: "hello",
        },
      });

      const createdSession = Effect.runSync(createRun);
      expect(createdSession.env?.APP_STAGE).toBe("production");

      const resumeRun = provider.resumeSession!({
        sessionId: "test-session-resume",
      });

      const resumedSession = Effect.runSync(resumeRun);
      expect(resumedSession.env).toBeDefined();
      expect(resumedSession.env?.APP_STAGE).toBe("production");
      expect(resumedSession.env?.EXTRA_VAR).toBe("hello");
    });
  });

  describe("makeLocalProcessSandboxProvider", () => {
    const baseDir = join(tmpdir(), "harness-env-tests-" + Date.now());

    test("executes process with merged environment variables", async () => {
      const provider = makeLocalProcessSandboxProvider({ baseDir });
      const createRun = provider.createSession({
        sessionId: "proc-session-dev",
        environmentId: "env-development",
        environments: testEnvironments,
        env: {
          CUSTOM_PROC_VAR: "proc-inline-value",
          REUSABLE_VAR: "proc-overridden",
        },
      });

      const session = await Effect.runPromise(createRun);
      expect(session.env).toBeDefined();
      expect(session.env?.APP_STAGE).toBe("development");
      expect(session.env?.CUSTOM_PROC_VAR).toBe("proc-inline-value");
      expect(session.env?.REUSABLE_VAR).toBe("proc-overridden");

      // We run a command in the host process and print the environment variables
      // to stdout so we can verify they are genuinely passed during execution.
      // On Windows, use 'echo %APP_STAGE%' etc. On Unix/Linux, use 'echo $APP_STAGE'.
      const isWindows = process.platform === "win32";
      const command = isWindows
        ? "echo %APP_STAGE%-%CUSTOM_PROC_VAR%-%REUSABLE_VAR%"
        : "echo $APP_STAGE-$CUSTOM_PROC_VAR-$REUSABLE_VAR";

      const runCommandEffect = session.run({ command });
      const result = await Effect.runPromise(runCommandEffect);

      expect(result.exitCode).toBe(0);
      const output = result.stdout.trim();
      expect(output).toBe("development-proc-inline-value-proc-overridden");

      // Verify that run-level overrides also work and take precedence
      const runWithOverrideEffect = session.run({
        command,
        env: {
          APP_STAGE: "run-level-stage",
        },
      });
      const overrideResult = await Effect.runPromise(runWithOverrideEffect);
      expect(overrideResult.exitCode).toBe(0);
      expect(overrideResult.stdout.trim()).toBe(
        "run-level-stage-proc-inline-value-proc-overridden",
      );

      // Stop session (cleanup)
      await Effect.runPromise(session.stop());
    });
  });
});
