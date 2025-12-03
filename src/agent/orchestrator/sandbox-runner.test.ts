import { describe, test, expect } from "bun:test";
import { Effect } from "effect";
import type { SandboxConfig } from "../../tasks/schema.js";
import {
  checkSandboxAvailable,
  buildContainerConfig,
  runCommand,
  runCommandString,
  runVerificationWithSandbox,
  type SandboxRunnerConfig,
  type SandboxRunnerEvent,
} from "./sandbox-runner.js";

// Default sandbox config values for tests
const DEFAULT_BACKEND = "auto" as const;
const DEFAULT_TIMEOUT_MS = 300_000;

describe("sandbox-runner", () => {
  describe("checkSandboxAvailable", () => {
    test("returns false when sandbox.enabled is false", async () => {
      const config: SandboxConfig = { enabled: false, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const events: SandboxRunnerEvent[] = [];
      const emit = (e: SandboxRunnerEvent) => events.push(e);

      const available = await Effect.runPromise(checkSandboxAvailable(config, emit));

      expect(available).toBe(false);
      expect(events).toContainEqual({
        type: "sandbox_unavailable",
        reason: "sandbox.enabled is false",
      });
    });

    test("returns false when sandbox.backend is 'none'", async () => {
      const config: SandboxConfig = { enabled: true, backend: "none", timeoutMs: DEFAULT_TIMEOUT_MS };
      const events: SandboxRunnerEvent[] = [];
      const emit = (e: SandboxRunnerEvent) => events.push(e);

      const available = await Effect.runPromise(checkSandboxAvailable(config, emit));

      expect(available).toBe(false);
      expect(events).toContainEqual({
        type: "sandbox_unavailable",
        reason: "sandbox.backend is 'none'",
      });
    });

    test("emits sandbox_check_start event", async () => {
      const config: SandboxConfig = { enabled: false, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const events: SandboxRunnerEvent[] = [];
      const emit = (e: SandboxRunnerEvent) => events.push(e);

      await Effect.runPromise(checkSandboxAvailable(config, emit));

      expect(events[0]).toEqual({ type: "sandbox_check_start" });
    });
  });

  describe("buildContainerConfig", () => {
    test("builds config with defaults", () => {
      const sandboxConfig: SandboxConfig = { enabled: true, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const cwd = "/test/project";

      const config = buildContainerConfig(sandboxConfig, cwd);

      expect(config.image).toBe("oven/bun:latest");
      expect(config.workspaceDir).toBe("/test/project");
      expect(config.workdir).toBe("/workspace");
      expect(config.autoRemove).toBe(true);
    });

    test("uses custom image from config", () => {
      const sandboxConfig: SandboxConfig = {
        enabled: true,
        backend: DEFAULT_BACKEND,
        timeoutMs: DEFAULT_TIMEOUT_MS,
        image: "custom:image",
      };
      const cwd = "/test/project";

      const config = buildContainerConfig(sandboxConfig, cwd);

      expect(config.image).toBe("custom:image");
    });

    test("passes resource limits", () => {
      const sandboxConfig: SandboxConfig = {
        enabled: true,
        backend: DEFAULT_BACKEND,
        memoryLimit: "4G",
        cpuLimit: 2,
        timeoutMs: 60000,
      };
      const cwd = "/test/project";

      const config = buildContainerConfig(sandboxConfig, cwd);

      expect(config.memoryLimit).toBe("4G");
      expect(config.cpuLimit).toBe(2);
      expect(config.timeoutMs).toBe(60000);
    });

    test("passes environment variables", () => {
      const sandboxConfig: SandboxConfig = { enabled: true, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const cwd = "/test/project";
      const env = { TEST_VAR: "test_value" };

      const config = buildContainerConfig(sandboxConfig, cwd, env);

      expect(config.env).toEqual({ TEST_VAR: "test_value" });
    });
  });

  describe("runCommand", () => {
    test("runs on host when sandbox is disabled", async () => {
      const sandboxConfig: SandboxConfig = { enabled: false, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const config: SandboxRunnerConfig = {
        sandboxConfig,
        cwd: process.cwd(),
      };

      const result = await Effect.runPromise(runCommand(["echo", "hello"], config));

      expect(result.sandboxed).toBe(false);
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("hello");
    });

    test("emits events during execution", async () => {
      const sandboxConfig: SandboxConfig = { enabled: false, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const events: SandboxRunnerEvent[] = [];
      const config: SandboxRunnerConfig = {
        sandboxConfig,
        cwd: process.cwd(),
        emit: (e) => events.push(e),
      };

      await Effect.runPromise(runCommand(["echo", "test"], config));

      expect(events.some((e) => e.type === "sandbox_check_start")).toBe(true);
      expect(events.some((e) => e.type === "sandbox_command_start")).toBe(true);
      expect(events.some((e) => e.type === "sandbox_command_complete")).toBe(true);
    });

    test("reports exit code on failure", async () => {
      const sandboxConfig: SandboxConfig = { enabled: false, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const config: SandboxRunnerConfig = {
        sandboxConfig,
        cwd: process.cwd(),
      };

      const result = await Effect.runPromise(runCommand(["false"], config));

      expect(result.sandboxed).toBe(false);
      expect(result.exitCode).not.toBe(0);
    });
  });

  describe("runCommandString", () => {
    test("parses command string and runs", async () => {
      const sandboxConfig: SandboxConfig = { enabled: false, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const config: SandboxRunnerConfig = {
        sandboxConfig,
        cwd: process.cwd(),
      };

      const result = await Effect.runPromise(runCommandString("echo hello world", config));

      expect(result.sandboxed).toBe(false);
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("hello world");
    });
  });

  describe("runVerificationWithSandbox", () => {
    test("runs verification commands on host when sandbox disabled", async () => {
      const sandboxConfig: SandboxConfig = { enabled: false, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const config: SandboxRunnerConfig = {
        sandboxConfig,
        cwd: process.cwd(),
      };
      const commands = ["echo test1", "echo test2"];
      const events: { type: string }[] = [];

      const result = await Effect.runPromise(
        runVerificationWithSandbox(commands, config, (e) => events.push(e))
      );

      expect(result.passed).toBe(true);
      expect(result.outputs.length).toBe(2);
      expect(result.sandboxed).toBe(false);
      expect(events.filter((e) => e.type === "verification_start").length).toBe(2);
      expect(events.filter((e) => e.type === "verification_complete").length).toBe(2);
    });

    test("returns failed when a command fails", async () => {
      const sandboxConfig: SandboxConfig = { enabled: false, backend: DEFAULT_BACKEND, timeoutMs: DEFAULT_TIMEOUT_MS };
      const config: SandboxRunnerConfig = {
        sandboxConfig,
        cwd: process.cwd(),
      };
      const commands = ["echo ok", "false", "echo after"];

      const result = await Effect.runPromise(
        runVerificationWithSandbox(commands, config)
      );

      expect(result.passed).toBe(false);
    });
  });
});
