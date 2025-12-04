import * as BunContext from "@effect/platform-bun/BunContext";
import { Effect } from "effect";
import { spawnSync } from "node:child_process";
import * as nodePath from "node:path";
import { loadProjectConfig } from "../tasks/project.js";

export type HealthCommandKind = "typecheck" | "test" | "e2e";

export interface HealthCommandResult {
  kind: HealthCommandKind;
  command: string;
  exitCode: number;
  stdout: string;
  stderr: string;
}

export interface HealthReport {
  ok: boolean;
  results: HealthCommandResult[];
}

const runShell = (command: string, cwd: string, kind: HealthCommandKind): HealthCommandResult => {
  const proc = spawnSync(command, {
    cwd,
    shell: true,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  return {
    kind,
    command,
    exitCode: proc.status ?? 1,
    stdout: proc.stdout ?? "",
    stderr: proc.stderr ?? "",
  };
};

export const runHealthChecks = async (rootDir = "."): Promise<HealthReport> => {
  const project = await Effect.runPromise(
    loadProjectConfig(rootDir).pipe(Effect.provide(BunContext.layer)),
  );

  if (!project) {
    throw new Error(`Project config not found at ${nodePath.join(rootDir, ".openagents/project.json")}`);
  }

  const commands: Array<{ kind: HealthCommandKind; command: string }> = [
    ...(project.typecheckCommands ?? []).map((command) => ({ kind: "typecheck" as const, command })),
    ...(project.testCommands ?? []).map((command) => ({ kind: "test" as const, command })),
    ...(project.e2eCommands ?? []).map((command) => ({ kind: "e2e" as const, command })),
  ];

  const results = commands.map((cmd) => runShell(cmd.command, rootDir, cmd.kind));
  const ok = results.every((r) => r.exitCode === 0);

  return { ok, results };
};
