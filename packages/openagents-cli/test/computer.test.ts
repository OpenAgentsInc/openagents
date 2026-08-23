import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { mkdtemp, rm, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { runCliWith } from "../src/cli.js";
import {
  ComputerConfiguration,
  computerConfigurationLayer,
  computerPaths,
  type ComputerPaths,
} from "../src/computer-config.js";
import {
  ComputerJournal,
  computerJournalLayer,
  journalMaxBytes,
  journalReadTailBytes,
} from "../src/computer-journal.js";
import {
  curatedAllowlist,
  decide,
  resolveRoots,
  tierAllows,
  type PolicyConfig,
  withinRoot,
} from "../src/computer-policy.js";
import {
  boundedVersion,
  codingAgentCatalog,
  computerProbeLayer,
  ComputerProbe,
  toolchainCatalog,
} from "../src/computer-probe.js";
import { executeComputerCommand } from "../src/computer-executor.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { credentialStoreTestFileLayer } from "../src/credential-store.js";
import { pendingDeviceAuthorizationStoreTestLayer } from "../src/device-authorization-store.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";

const computerConfigurationTestLayer = (
  values: Partial<PolicyConfig> & { readonly paths?: ComputerPaths } = {},
): Layer.Layer<ComputerConfiguration> =>
  Layer.succeed(
    ComputerConfiguration,
    ComputerConfiguration.of({
      tier: values.tier ?? "probe",
      roots: resolveRoots(values.roots ?? []),
      preApproved: values.preApproved ?? [],
      paths: values.paths ?? computerPaths(),
    }),
  );

const computerJournalTestLayer = (path: string): Layer.Layer<ComputerJournal> =>
  computerJournalLayer.pipe(
    Layer.provide(
      computerConfigurationTestLayer({
        paths: { ...computerPaths(path), journal: path },
      }),
    ),
  );

describe("local Computer policy", () => {
  const root = "/workspace/project";
  const config = { tier: "probe" as const, roots: [root], preApproved: [] };

  it("defaults to probe with no reachable roots", async () => {
    const directory = await mkdtemp(join("/tmp", "openagents-cli-computer-"));
    try {
      const layer = computerConfigurationLayer.pipe(
        Layer.provide(environmentLayerFromValues({ configPath: join(directory, "config.json") })),
      );
      const value = await Effect.runPromise(ComputerConfiguration.pipe(Effect.provide(layer)));
      expect(value.tier).toBe("probe");
      expect(value.roots).toEqual([]);
      expect(value.paths.config).toBe(join(directory, "computer.json"));
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it("resolves roots and rejects textual-prefix siblings", () => {
    expect(resolveRoots(["~/work", "~/work/"])).toEqual([join(homedir(), "work")]);
    expect(withinRoot("/workspace/project/file.txt", root)).toBe(true);
    expect(withinRoot("/workspace/project-two", root)).toBe(false);
    if (process.platform === "win32") {
      expect(withinRoot("C:\\workspace\\project\\file.txt", "C:\\workspace\\project")).toBe(true);
      expect(withinRoot("C:\\workspace\\project-two", "C:\\workspace\\project")).toBe(false);
    }
  });

  it("keeps the tier as a ceiling and applies universal denials first", () => {
    expect(tierAllows("probe", "shell")).toBe(false);
    expect(tierAllows("shell", "curated")).toBe(true);
    expect(decide({ argv: ["git", "status"], cwd: root }, config)).toEqual({
      _tag: "Refused",
      reason: "tier_insufficient",
      detail: "probe tier permits fixed discovery only",
    });
    expect(
      decide({ argv: ["sudo", "git", "status"], cwd: root }, { ...config, tier: "shell" }),
    ).toMatchObject({ _tag: "Refused", reason: "denied_command" });
    expect(
      decide({ argv: ["cat", "/etc/passwd"], cwd: root }, { ...config, tier: "shell" }),
    ).toMatchObject({ _tag: "Refused", reason: "denied_argument" });
    expect(
      decide(
        { argv: ["cat", "/workspace/project/file.txt"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Allowed" });
    expect(
      decide({ argv: ["cat", "/tmp/outside"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "denied_argument" });
    expect(decide({ argv: ["git", "status"], cwd: root }, { ...config, roots: [] })).toMatchObject({
      _tag: "Refused",
      reason: "root_not_declared",
    });
    expect(
      decide({ argv: ["echo", "hello;whoami"], cwd: root }, { ...config, tier: "shell" }),
    ).toMatchObject({ _tag: "Refused", reason: "shell_metacharacter" });
    expect(
      decide(
        { argv: ["env", "FOO=1", "bash", "-c", "id"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["npm", "run", "build"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide(
        { argv: ["find", ".", "-exec", "sh", "-c", "id"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["find", ".", "-delete"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide(
        { argv: ["rg", "--pre", "bash", "pattern"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["git", "branch", "-D", "main"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide(
        { argv: ["git", "remote", "set-url", "origin", "evil"], cwd: root },
        { ...config, tier: "curated" },
      ),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["./opencode.exe", "acp"], cwd: root }, { ...config, tier: "shell" }),
    ).toEqual({ _tag: "Allowed", needsConfirmation: true });
    expect(
      decide({ argv: ["gh", "issue", "list"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Allowed" });
    expect(
      decide({ argv: ["gh", "issue", "close"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["date", "+%s"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
    expect(
      decide({ argv: ["ps", "aux"], cwd: root }, { ...config, tier: "curated" }),
    ).toMatchObject({ _tag: "Refused", reason: "not_allowlisted" });
  });

  it("requires confirmation for non-pre-approved shell commands", () => {
    expect(decide({ argv: ["echo", "hello"], cwd: root }, { ...config, tier: "shell" })).toEqual({
      _tag: "Allowed",
      needsConfirmation: true,
    });
    expect(
      decide(
        { argv: ["echo", "hello"], cwd: root },
        { ...config, tier: "shell", preApproved: ["echo"] },
      ),
    ).toEqual({ _tag: "Allowed", needsConfirmation: false });
  });

  it("exposes a versioned read-only allowlist", () => {
    expect(curatedAllowlist.git).toContain("status");
    expect(curatedAllowlist.sudo).toBeUndefined();
  });
});

describe("local Computer probe", () => {
  it("keeps known catalogs fixed and versions bounded", () => {
    expect(codingAgentCatalog.map((entry) => entry.name)).toContain("claude");
    expect(toolchainCatalog.find((entry) => entry.name === "go")?.versionArgv).toEqual(["version"]);
    expect(boundedVersion("x".repeat(200))).toHaveLength(120);
  });

  it("returns a complete report with missing tools as data", async () => {
    const layer = computerProbeLayer.pipe(
      Layer.provide(Layer.merge(NodeServices.layer, computerConfigurationTestLayer({ roots: [] }))),
    );
    const report = await Effect.runPromise(
      Effect.gen(function* () {
        const probe = yield* ComputerProbe;
        return yield* probe.probe();
      }).pipe(Effect.provide(layer)),
    );
    expect(report.schema).toBe("openagents.computer_probe.v1");
    expect(report.roots).toEqual([]);
    expect(report.host.platform).toBe(process.platform);
    expect(report.codingAgents.every((entry) => typeof entry.present === "boolean")).toBe(true);
    expect(report.toolchains.every((entry) => typeof entry.version === "string")).toBe(true);
    expect(report.worktrees).toEqual([]);
  });
});

describe("local Computer journal", () => {
  it("reads no entries when its file is absent and redacts credential-shaped values", async () => {
    const directory = await mkdtemp(join("/tmp", "openagents-cli-journal-"));
    const path = join(directory, "journal.ndjson");
    try {
      const layer = computerJournalTestLayer(path);
      const entries = await Effect.runPromise(
        Effect.gen(function* () {
          const journal = yield* ComputerJournal;
          expect(yield* journal.read(20)).toEqual([]);
          yield* journal.append({
            requestId: "request-1",
            argv: [
              "git",
              "status",
              "oa_pat_secret",
              "oa_agent_secret",
              "oa_assignment_secret",
              "smct_secret",
            ],
            cwd: "/workspace/project",
            decision: "refused",
            outcome: "refused",
            detail: "oa_assignment_secret smct_secret",
          });
          return yield* journal.read(20);
        }).pipe(Effect.provide(layer)),
      );
      expect(entries[0]).toMatchObject({
        requestId: "request-1",
        argv: ["git", "status", "[REDACTED]", "[REDACTED]", "[REDACTED]", "[REDACTED]"],
        decision: "refused",
        detail: "[REDACTED] [REDACTED]",
      });
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  it("bounds journal growth while retaining the newest entries", async () => {
    const directory = await mkdtemp(join("/tmp", "openagents-cli-journal-limit-"));
    const path = join(directory, "journal.ndjson");
    try {
      await Effect.runPromise(
        Effect.gen(function* () {
          const journal = yield* ComputerJournal;
          for (let index = 0; index < 800; index += 1) {
            yield* journal.append({
              requestId: `request-${index}`,
              argv: ["git", "status"],
              cwd: "/workspace/project",
              decision: "allowed",
              outcome: "completed",
              detail: "x".repeat(512),
            });
          }
        }).pipe(Effect.provide(computerJournalTestLayer(path))),
      );
      expect((await stat(path)).size).toBeLessThanOrEqual(journalMaxBytes);
      const entries = await Effect.runPromise(
        Effect.gen(function* () {
          const journal = yield* ComputerJournal;
          return yield* journal.read(20);
        }).pipe(Effect.provide(computerJournalTestLayer(path))),
      );
      expect(entries).toHaveLength(20);
      expect(entries.at(-1)?.requestId).toBe("request-799");
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});

describe("local Computer execution", () => {
  it("executes argv directly with scrubbed environment and bounded output", async () => {
    const chunks: string[] = [];
    const execution = executeComputerCommand(
      [
        process.execPath,
        "-e",
        "process.stdout.write((process.env.OPENAGENTS_COMPUTER_SECRET ?? 'missing') + 'x'.repeat(32))",
      ],
      process.cwd(),
      { timeoutMillis: 5_000, maximumOutputBytes: 8 },
      (chunk) => chunks.push(chunk),
    );
    const outcome = await execution.done;
    expect(outcome.exitCode).toBe(0);
    expect(outcome.truncated).toBe(true);
    expect(chunks.join("")).toBe("missingx");
    expect(chunks.join("")).not.toContain("oa_");
  });

  it("reports cancellation without fabricating an exit code", async () => {
    const execution = executeComputerCommand(
      [process.execPath, "-e", "setInterval(() => undefined, 10_000)"],
      process.cwd(),
      { timeoutMillis: 5_000, maximumOutputBytes: 64 },
      () => undefined,
    );
    execution.cancel();
    const outcome = await execution.done;
    expect(outcome.cancelled).toBe(true);
    expect(outcome.timedOut).toBe(false);
    expect(outcome.exitCode).toBe(null);
  });
});

describe("Computer CLI output", () => {
  const output = (
    documents: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }>,
  ) =>
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        documents.push({ document, mode });
      }),
    );

  it("prints stable JSON policy and status without auth or network", async () => {
    const documents: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }> = [];
    const credentialPath = join("/tmp", "openagents-cli-status-credentials.json");
    const layer = Layer.mergeAll(
      computerConfigurationTestLayer({ roots: [] }),
      output(documents),
      NodeServices.layer,
      environmentLayerFromValues({}),
      persistedConfigurationTestLayer({}),
      credentialStoreTestFileLayer(credentialPath),
      pendingDeviceAuthorizationStoreTestLayer(),
    );
    await Effect.runPromise(
      runCliWith(["--json", "computer", "policy"]).pipe(Effect.provide(layer)),
    );
    await Effect.runPromise(
      runCliWith(["--json", "computer", "status"]).pipe(Effect.provide(layer)),
    );
    expect(documents).toHaveLength(2);
    expect(documents[0]?.mode).toBe("json");
    expect(documents[0]?.document.value).toMatchObject({
      schema: "openagents.computer_policy.v1",
      tier: "probe",
      roots: [],
      authority: "local_machine",
      network: false,
    });
    expect(documents[1]?.document.value).toMatchObject({
      schema: "openagents.computer_status.v1",
      journal_retention_bytes: journalMaxBytes,
      journal_read_tail_bytes: journalReadTailBytes,
    });
    expect(JSON.stringify(documents)).not.toContain("oa_pat_");
    expect(JSON.stringify(documents)).not.toContain("oa_machine_");
    await rm(credentialPath, { force: true });
  });

  it("prints stable JSON probe output without auth or network", async () => {
    const documents: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }> = [];
    const report = {
      schema: "openagents.computer_probe.v1" as const,
      host: {
        platform: "test",
        release: "test",
        architecture: "test",
        hostname: "test",
        shell: "",
        cpuCount: 1,
        totalMemoryBytes: 1,
        uptimeSeconds: 1,
      },
      codingAgents: [],
      toolchains: [],
      roots: [],
      worktrees: [],
    };
    const layer = Layer.mergeAll(
      computerConfigurationTestLayer({ roots: [] }),
      Layer.succeed(ComputerProbe, ComputerProbe.of({ probe: () => Effect.succeed(report) })),
      output(documents),
      NodeServices.layer,
    );
    await Effect.runPromise(
      runCliWith(["--json", "computer", "probe"]).pipe(Effect.provide(layer)),
    );
    expect(documents[0]?.mode).toBe("json");
    expect(documents[0]?.document.value).toEqual(report);
  });

  it("prints an absent journal as stable JSON", async () => {
    const documents: Array<{ readonly document: OutputDocument; readonly mode: OutputMode }> = [];
    const directory = await mkdtemp(join("/tmp", "openagents-cli-journal-cli-"));
    try {
      const layer = Layer.mergeAll(
        computerConfigurationTestLayer({ paths: computerPaths(join(directory, "config.json")) }),
        computerJournalTestLayer(join(directory, "journal.ndjson")),
        output(documents),
        NodeServices.layer,
      );
      await Effect.runPromise(
        runCliWith(["--json", "computer", "journal"]).pipe(Effect.provide(layer)),
      );
      expect(documents[0]?.document.value).toEqual({
        schema: "openagents.computer_journal.v1",
        entries: [],
      });
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });
});
