#!/usr/bin/env bun
// Generate / verify the canonical OpenAgents repo study-packet artifact (SA-1, EPIC #5337).
//
// Runs the existing study-packet / corpus-manifest / study-graph / verification /
// eval-harness pipeline over the REAL openagents repo tree and derives a small,
// digest-pinned index. The full multi-MB packet/graph blobs are regenerated on
// demand and never committed (mirrors the #5334 "generate-at-build,
// regenerate-and-diff = benchmark-as-receipt" discipline).
//
// Modes:
//   --print            Generate the artifact and print the index JSON to stdout (default).
//   --write            Generate and write the committed index file (stable identity).
//   --check            Regenerate and diff against the committed index; non-zero exit on drift
//                      or a failed correctness gate. This is the SA-4 freshness receipt.
//
// Flags:
//   --root <dir>           Repo root to study (default: repo root inferred from this file).
//   --backroom <dir>       Backroom archive root (default: <root>/../backroom).
//   --commit-history <n>   Bound commit-history depth (default: 200).
//   --out <file>           Index file path (default: the committed study-packets index).

import { readFile, mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { Cause, Effect, Exit } from "effect";
import {
  decodeOpenAgentsRepoStudyArtifactIndex,
  generateOpenAgentsRepoStudyArtifact,
  OPENAGENTS_REPO_STUDY_ARTIFACT_DEFAULT_COMMIT_HISTORY_LIMIT,
  type OpenAgentsRepoStudyArtifactIndex,
} from "../src/benchmark/openagents-study-artifact";

const RUNTIME_ROOT = resolve(import.meta.dir, "..");
const DEFAULT_REPO_ROOT = resolve(RUNTIME_ROOT, "..", "..", "..", "..");
const DEFAULT_INDEX_RELATIVE =
  "docs/research/machine-studying/openagents-studybench/study-packets/openagents.study-artifact-index.json";

type Mode = "print" | "write" | "check";

interface Cli {
  readonly backroomRootDir?: string;
  readonly commitHistoryLimit: number;
  readonly mode: Mode;
  readonly outPath?: string;
  readonly rootDir: string;
}

function parseCli(argv: ReadonlyArray<string>): Cli {
  let mode: Mode = "print";
  let rootDir = DEFAULT_REPO_ROOT;
  let backroomRootDir: string | undefined;
  let commitHistoryLimit = OPENAGENTS_REPO_STUDY_ARTIFACT_DEFAULT_COMMIT_HISTORY_LIMIT;
  let outPath: string | undefined;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--write") {
      mode = "write";
    } else if (arg === "--check") {
      mode = "check";
    } else if (arg === "--print") {
      mode = "print";
    } else if (arg === "--root") {
      rootDir = resolve(argv[++i] ?? rootDir);
    } else if (arg === "--backroom") {
      backroomRootDir = resolve(argv[++i] ?? "");
    } else if (arg === "--commit-history") {
      commitHistoryLimit = Number.parseInt(argv[++i] ?? "", 10);
    } else if (arg === "--out") {
      outPath = resolve(argv[++i] ?? "");
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  return { backroomRootDir, commitHistoryLimit, mode, outPath, rootDir };
}

function indexPathFor(cli: Cli): string {
  return cli.outPath ?? resolve(cli.rootDir, DEFAULT_INDEX_RELATIVE);
}

function sortKeysDeep(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((entry) => sortKeysDeep(entry));
  }

  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, sortKeysDeep(entry)]),
    );
  }

  return value;
}

function stableIndexJson(index: OpenAgentsRepoStudyArtifactIndex): string {
  return `${JSON.stringify(sortKeysDeep(index), null, 2)}\n`;
}

const cli = parseCli(Bun.argv.slice(2));

const program = Effect.gen(function* () {
  const artifact = yield* generateOpenAgentsRepoStudyArtifact({
    backroomRootDir: cli.backroomRootDir,
    commitHistoryLimit: cli.commitHistoryLimit,
    rootDir: cli.rootDir,
  });
  return artifact.index;
});

const exit = await Effect.runPromiseExit(program);

if (Exit.isFailure(exit)) {
  console.error("study-packet generation failed:");
  console.error(Cause.pretty(exit.cause));
  process.exit(1);
}

const index = exit.value;
const indexPath = indexPathFor(cli);
const rendered = stableIndexJson(index);

if (cli.mode === "print") {
  process.stdout.write(rendered);
  process.exit(0);
}

if (cli.mode === "write") {
  await mkdir(dirname(indexPath), { recursive: true });
  await writeFile(indexPath, rendered, "utf8");
  console.log(`wrote ${indexPath}`);
  console.log(`indexHash=${index.indexHash}`);
  console.log(`packetHash=${index.packetHash} graphHash=${index.graphHash}`);
  console.log(
    `verification gate=${index.correctnessGatePassed} accepted=${index.acceptedClaimCount} rejected=${index.rejectedClaimCount}`,
  );
  console.log(
    `eval lift passRateBps=${index.evalLift.passRateLiftBps} rubricBps=${index.evalLift.rubricScoreLiftBps} firstDivergenceStep=${index.evalLift.firstDivergenceStepLift}`,
  );
  process.exit(0);
}

// --check: regenerate-and-diff against the committed index + correctness gate.
const committedText = await readFile(indexPath, "utf8").catch(() => undefined);

if (committedText === undefined) {
  console.error(`no committed index at ${indexPath}; run with --write first`);
  process.exit(1);
}

const committed = await Effect.runPromise(
  decodeOpenAgentsRepoStudyArtifactIndex(JSON.parse(committedText)),
).catch((error: unknown) => {
  console.error(`committed index failed to decode: ${String(error)}`);
  process.exit(1);
});

if (!index.correctnessGatePassed) {
  console.error("regenerated artifact failed the verification correctness gate");
  process.exit(1);
}

if (committed.indexHash !== index.indexHash) {
  console.error("study-packet index drift detected (regenerate-and-diff mismatch):");
  console.error(`  committed indexHash=${committed.indexHash} commit=${committed.commit}`);
  console.error(`  regenerated indexHash=${index.indexHash} commit=${index.commit}`);
  console.error("re-run with --write to refresh the committed index.");
  process.exit(2);
}

console.log(`study-packet index fresh: indexHash=${index.indexHash} commit=${index.commit}`);
process.exit(0);
