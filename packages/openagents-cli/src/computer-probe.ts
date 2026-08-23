import { Effect, Layer, Stream } from "effect";
import * as Context from "effect/Context";
import { ChildProcess, ChildProcessSpawner } from "effect/unstable/process";
import * as os from "node:os";
import { existsSync, statSync } from "node:fs";
import { join } from "node:path";

import { ComputerConfiguration } from "./computer-config.js";
import { buildAgentCatalog, type AcpAgentInventoryEntry } from "./computer-agents.js";

export interface ToolReport {
  readonly name: string;
  readonly present: boolean;
  readonly path: string;
  readonly version: string;
}

export interface HostReport {
  readonly platform: string;
  readonly release: string;
  readonly architecture: string;
  readonly hostname: string;
  readonly shell: string;
  readonly cpuCount: number;
  readonly totalMemoryBytes: number;
  readonly uptimeSeconds: number;
}

export interface WorktreeReport {
  readonly root: string;
  readonly exists: boolean;
  readonly git: boolean;
}

export interface ProbeReport {
  readonly schema: "openagents.computer_probe.v1";
  readonly host: HostReport;
  readonly codingAgents: ReadonlyArray<ToolReport>;
  readonly toolchains: ReadonlyArray<ToolReport>;
  readonly roots: ReadonlyArray<string>;
  readonly worktrees: ReadonlyArray<WorktreeReport>;
  readonly acp_agents?: ReadonlyArray<AcpAgentInventoryEntry>;
}

interface Probed {
  readonly name: string;
  readonly versionArgv: ReadonlyArray<string>;
}

export const codingAgentCatalog: ReadonlyArray<Probed> = [
  { name: "claude", versionArgv: ["--version"] },
  { name: "codex", versionArgv: ["--version"] },
  { name: "devin", versionArgv: ["--version"] },
  { name: "gemini", versionArgv: ["--version"] },
  { name: "cursor-agent", versionArgv: ["--version"] },
  { name: "aider", versionArgv: ["--version"] },
  { name: "goose", versionArgv: ["--version"] },
  { name: "opencode", versionArgv: ["--version"] },
  { name: "amp", versionArgv: ["--version"] },
  { name: "copilot", versionArgv: ["--version"] },
  { name: "crush", versionArgv: ["--version"] },
];

export const toolchainCatalog: ReadonlyArray<Probed> = [
  { name: "git", versionArgv: ["--version"] },
  { name: "gh", versionArgv: ["--version"] },
  { name: "node", versionArgv: ["--version"] },
  { name: "npm", versionArgv: ["--version"] },
  { name: "pnpm", versionArgv: ["--version"] },
  { name: "bun", versionArgv: ["--version"] },
  { name: "deno", versionArgv: ["--version"] },
  { name: "python3", versionArgv: ["--version"] },
  { name: "uv", versionArgv: ["--version"] },
  { name: "cargo", versionArgv: ["--version"] },
  { name: "go", versionArgv: ["version"] },
  { name: "elixir", versionArgv: ["--version"] },
  { name: "docker", versionArgv: ["--version"] },
  { name: "tmux", versionArgv: ["-V"] },
];

export const boundedVersion = (value: string): string => value.slice(0, 120);

export interface ComputerProbeInterface {
  readonly probe: (roots?: ReadonlyArray<string>) => Effect.Effect<ProbeReport>;
}

export class ComputerProbe extends Context.Service<ComputerProbe, ComputerProbeInterface>()(
  "@openagentsinc/cli/ComputerProbe",
) {}

const hostReport = (): HostReport => ({
  platform: process.platform,
  release: os.release(),
  architecture: process.arch,
  hostname: os.hostname(),
  shell: process.env["SHELL"] ?? "",
  cpuCount: os.cpus().length,
  totalMemoryBytes: os.totalmem(),
  uptimeSeconds: Math.round(os.uptime()),
});

const worktreeReport = (root: string): WorktreeReport => {
  try {
    const stats = statSync(root);
    return { root, exists: stats.isDirectory(), git: existsSync(join(root, ".git")) };
  } catch {
    return { root, exists: false, git: false };
  }
};

export const computerProbeLayer = Layer.effect(
  ComputerProbe,
  Effect.gen(function* () {
    const config = yield* ComputerConfiguration;
    const spawner = yield* ChildProcessSpawner.ChildProcessSpawner;
    const probe = Effect.fn("ComputerProbe.probe")(function* (roots = config.roots) {
      const resolvedRoots = roots;
      const cwd = resolvedRoots[0] ?? process.cwd();
      const runQuietly = (argv: ReadonlyArray<string>) =>
        Effect.gen(function* () {
          const executable = argv[0];
          if (executable === undefined) return "";
          const handle = yield* spawner.spawn(
            ChildProcess.make(executable, argv.slice(1), {
              cwd,
              shell: false,
              stdin: "ignore",
              stdout: "pipe",
              stderr: "ignore",
            }),
          );
          let output = "";
          const decoder = new TextDecoder();
          yield* Stream.runForEach(handle.stdout, (chunk) =>
            Effect.sync(() => {
              if (output.length < 512 && chunk instanceof Uint8Array) {
                output += decoder.decode(chunk, { stream: true });
              }
            }),
          );
          const exitCode = yield* handle.exitCode;
          return Number(exitCode) === 0 ? output.trim() : "";
        }).pipe(
          Effect.scoped,
          Effect.orElseSucceed(() => ""),
        );
      const probeOne = (probed: Probed): Effect.Effect<ToolReport> =>
        Effect.gen(function* () {
          const resolver = process.platform === "win32" ? "where" : "which";
          const resolved = yield* runQuietly([resolver, probed.name]);
          if (resolved === "") return { name: probed.name, present: false, path: "", version: "" };
          const version = yield* runQuietly([probed.name, ...probed.versionArgv]);
          return {
            name: probed.name,
            present: true,
            path: resolved.split("\n")[0] ?? "",
            version: boundedVersion(version),
          };
        });
      const codingAgents = yield* Effect.forEach(codingAgentCatalog, probeOne, { concurrency: 4 });
      const toolchains = yield* Effect.forEach(toolchainCatalog, probeOne, { concurrency: 4 });
      const acpAgents = buildAgentCatalog(config, codingAgents).map((entry) => ({
        id: entry.id,
        source: entry.source,
        version: entry.version,
      }));
      return {
        schema: "openagents.computer_probe.v1" as const,
        host: hostReport(),
        codingAgents,
        toolchains,
        roots: resolvedRoots,
        worktrees: resolvedRoots.map(worktreeReport),
        acp_agents: acpAgents,
      };
    });
    return ComputerProbe.of({ probe });
  }),
);
