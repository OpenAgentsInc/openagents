import { Effect, Layer } from "effect";
import { randomUUID } from "node:crypto";
import { execFileSync, spawn } from "node:child_process";
import * as fs from "node:fs";
import {
  ContainerBackendTag,
  type ContainerBackend,
} from "./backend.js";
import { ContainerError, type ContainerRunResult } from "./schema.js";

const DEFAULT_TIMEOUT_MS = 120_000;

const hasDockerCommand = (): boolean => {
  try {
    if (process.env.OPENAGENTS_DOCKER_AVAILABLE === "0") return false;
    if (process.env.OPENAGENTS_DOCKER_AVAILABLE === "1") return true;
    execFileSync("docker", ["--version"], { stdio: "ignore", timeout: 2000 });
    return true;
  } catch {
    return false;
  }
};

const buildDockerArgs = (
  command: string[],
  config: {
    image: string;
    workspaceDir: string;
    workdir?: string;
    memoryLimit?: string;
    cpuLimit?: number;
    env?: Record<string, string>;
    volumeMounts?: string[];
    autoRemove?: boolean;
  },
  containerName: string,
) => {
  const args: string[] = ["run", "-i", "--name", containerName];

  if (config.autoRemove ?? true) {
    args.push("--rm");
  }

  args.push("-v", `${config.workspaceDir}:/workspace`);

  for (const mount of config.volumeMounts ?? []) {
    args.push("-v", mount);
  }

  args.push("-w", config.workdir ?? "/workspace");

  if (config.memoryLimit) {
    args.push("--memory", config.memoryLimit);
  }
  if (config.cpuLimit) {
    args.push("--cpus", String(config.cpuLimit));
  }

  for (const [key, value] of Object.entries(config.env ?? {})) {
    args.push("-e", `${key}=${value}`);
  }

  args.push(config.image);
  args.push(...command);

  return args;
};

const dockerRun: ContainerBackend["run"] = (command, config, options) =>
  Effect.async<ContainerRunResult, ContainerError>((resume) => {
    if (!fs.existsSync(config.workspaceDir)) {
      resume(
        Effect.fail(
          new ContainerError(
            "start_failed",
            `workspaceDir does not exist: ${config.workspaceDir}`,
          ),
        ),
      );
      return;
    }

    const name = `oa-sbx-${randomUUID()}`;
    const runConfig = {
      image: config.image,
      workspaceDir: config.workspaceDir,
      ...(config.workdir ? { workdir: config.workdir } : {}),
      ...(config.memoryLimit ? { memoryLimit: config.memoryLimit } : {}),
      ...(config.cpuLimit !== undefined ? { cpuLimit: config.cpuLimit } : {}),
      ...(config.env ? { env: config.env } : {}),
      ...(config.volumeMounts ? { volumeMounts: [...config.volumeMounts] } : {}),
      ...(config.autoRemove !== undefined ? { autoRemove: config.autoRemove } : {}),
    };
    const args = buildDockerArgs(command, runConfig, name);
    const proc = spawn("docker", args, { stdio: ["ignore", "pipe", "pipe"] });

    let stdout = "";
    let stderr = "";
    let finished = false;

    const cleanup = () => {
      finished = true;
      clearTimeout(timeoutHandle);
    };

    const finish = (result: Effect.Effect<ContainerRunResult, ContainerError>) => {
      cleanup();
      resume(result);
    };

    const timeoutMs = config.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    const timeoutHandle = setTimeout(() => {
      if (finished) return;
      proc.kill("SIGKILL");
      finish(
        Effect.fail(
          new ContainerError(
            "timeout",
            `Container execution timed out after ${timeoutMs}ms`,
          ),
        ),
      );
    }, timeoutMs);

    proc.on("error", (error) => {
      if (finished) return;
      finish(
        Effect.fail(
          new ContainerError(
            "start_failed",
            `Failed to start docker: ${error.message}`,
          ),
        ),
      );
    });

    proc.stdout?.on("data", (data: Buffer) => {
      const chunk = data.toString("utf-8");
      stdout += chunk;
      options?.onStdout?.(chunk);
    });

    proc.stderr?.on("data", (data: Buffer) => {
      const chunk = data.toString("utf-8");
      stderr += chunk;
      options?.onStderr?.(chunk);
    });

    if (options?.signal) {
      if (options.signal.aborted) {
        proc.kill("SIGKILL");
      } else {
        options.signal.addEventListener("abort", () => proc.kill("SIGKILL"), {
          once: true,
        });
      }
    }

    proc.on("close", (code) => {
      if (finished) return;
      finish(
        Effect.succeed({
          exitCode: code ?? 1,
          stdout,
          stderr,
          containerId: name,
        }),
      );
    });
  });

const dockerBuild: ContainerBackend["build"] = (contextDir, tag, options) =>
  Effect.async<void, ContainerError>((resume) => {
    const args: string[] = ["build", "-t", tag];
    if (options?.file) {
      args.push("-f", options.file);
    }
    if (options?.memoryLimit) {
      args.push("--memory", options.memoryLimit);
    }
    if (options?.cpuLimit) {
      args.push("--cpus", String(options.cpuLimit));
    }
    args.push(contextDir);

    const proc = spawn("docker", args, { stdio: "ignore" });

    const finish = (result: Effect.Effect<void, ContainerError>) => resume(result);

    proc.on("error", (error) =>
      finish(
        Effect.fail(
          new ContainerError(
            "start_failed",
            `Failed to start docker build: ${error.message}`,
          ),
        ),
      ),
    );

    proc.on("close", (code) => {
      if ((code ?? 1) !== 0) {
        finish(
          Effect.fail(
            new ContainerError(
              "execution_failed",
              `docker build exited with code ${code ?? 1}`,
              code ?? undefined,
            ),
          ),
        );
        return;
      }
      finish(Effect.succeed(undefined));
    });
  });

const dockerBackend: ContainerBackend = {
  name: "docker",
  isAvailable: () =>
    Effect.try(() => hasDockerCommand()).pipe(Effect.catchAll(() => Effect.succeed(false))),
  run: dockerRun,
  build: dockerBuild,
};

export const dockerBackendLive = Layer.succeed(ContainerBackendTag, dockerBackend);
export const dockerBackendLayer = dockerBackendLive;
