#!/usr/bin/env node
// QA-5 (#8910): independent-verifier executor — the mechanical half of the
// no-self-acceptance recipe in docs/qa/verifier/README.md.
//
// Given a work unit (issue number + claimed commit) and a typed claims file
// (built by a DIFFERENT agent from the implementer's closing comment), it:
//   1. refuses to run when the verifier actor equals the implementer;
//   2. creates a clean scratch git worktree detached at the claimed commit;
//   3. installs dependencies and runs any declared setup steps;
//   4. re-runs each runnable claim (tests, smokes, artifact checks) and runs
//      the adversarial probes (deliberately break the guarded behavior in
//      the scratch copy, confirm the cited proof FAILS, restore);
//   5. writes a typed verdict artifact (accept | reject | unverifiable-here)
//      to docs/qa/verifier/results/ and prints the ready-to-post comment.
//
// Usage (from the repo root):
//   pnpm run qa:verify -- --issue 8907 --commit 08096cae24 \
//     --claims docs/qa/verifier/results/qa-verify-issue-8907-<sha12>.claims.json \
//     --actor <verifier session id> [--keep-scratch] [--scratch-dir <dir>] \
//     [--out-dir <dir>] [--skip-install]
//
// Honest states per claim: verified | failed | unverifiable_here(reason).
// Owner-gated or env-missing proofs are unverifiable, never auto-accepted.
//
// Exit codes: 0 accept, 1 reject, 2 usage/independence/decode error,
// 3 unverifiable-here.
//
// Secrets: the child environment is inherited (so tokens named by
// requiredEnv reach the re-run proof as env only). Env values are never
// printed and never written to artifacts; only claim command output tails
// are recorded, exactly like the QA observer's bounded evidence.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  QA_VERIFIER_ISSUE,
  QA_VERIFIER_VERDICT_SCHEMA,
  applyMutation,
  artifactFileName,
  boundTail,
  buildVerdictComment,
  computeVerdict,
  decodeVerifierClaims,
  independenceProblem,
  renderArgv,
  type ClaimResult,
  type SetupResult,
  type VerdictArtifact,
  type VerifierClaim,
  type VerifierClaimsFile,
} from "./qa-verify-registry.js";

const DEFAULT_CLAIM_TIMEOUT_MS = 900_000;
const INSTALL_TIMEOUT_MS = 900_000;
const MAX_BUFFER = 64 * 1024 * 1024;

type ExecOutcome = Readonly<{ exitCode: number | null; output: string }>;

const runArgv = (command: readonly string[], cwd: string, timeoutMs: number): ExecOutcome => {
  const [file, ...args] = command;
  try {
    const stdout = execFileSync(file as string, args, {
      cwd,
      encoding: "utf8",
      env: process.env,
      maxBuffer: MAX_BUFFER,
      stdio: ["ignore", "pipe", "pipe"],
      timeout: timeoutMs,
    });
    return { exitCode: 0, output: stdout };
  } catch (error) {
    const failure = error as {
      status?: number | null;
      stdout?: string;
      stderr?: string;
      signal?: string | null;
      code?: string;
      message?: string;
    };
    const output = `${failure.stdout ?? ""}${failure.stderr ?? ""}`;
    if (failure.code === "ENOENT" || (output === "" && failure.status === undefined)) {
      return {
        exitCode: null,
        output: `command could not be spawned: ${failure.message ?? String(error)}`,
      };
    }
    if (failure.signal === "SIGTERM" && failure.status == null) {
      return { exitCode: null, output: `${output}\n[timed out after ${timeoutMs}ms]` };
    }
    return { exitCode: failure.status ?? null, output };
  }
};

const missingEnv = (requiredEnv: readonly string[] | undefined): readonly string[] =>
  (requiredEnv ?? []).filter((name) => {
    const value = process.env[name];
    return value === undefined || value.trim() === "";
  });

const runClaim = (claim: VerifierClaim, scratch: string, installOk: boolean): ClaimResult => {
  const startedAt = Date.now();
  const base = { id: claim.id, kind: claim.kind, title: claim.title };
  if (claim.kind === "attested") {
    return {
      ...base,
      durationMs: Date.now() - startedAt,
      reason: claim.reason,
      status: "unverifiable_here",
    };
  }
  if (claim.kind === "file_exists") {
    const exists = existsSync(join(scratch, claim.path));
    return {
      ...base,
      durationMs: Date.now() - startedAt,
      ...(exists
        ? { reason: `${claim.path} exists at the claimed commit` }
        : { reason: `claimed artifact ${claim.path} does not exist at the claimed commit` }),
      status: exists ? "verified" : "failed",
    };
  }
  // command | adversarial from here on.
  const rendered = renderArgv(claim.command);
  const missing = missingEnv(claim.requiredEnv);
  if (missing.length > 0) {
    return {
      ...base,
      command: rendered,
      durationMs: Date.now() - startedAt,
      reason: `requires env ${missing.join(", ")} not present in the verifier environment`,
      status: "unverifiable_here",
    };
  }
  if (!installOk) {
    return {
      ...base,
      command: rendered,
      durationMs: Date.now() - startedAt,
      reason: "scratch checkout dependency install failed; the proof could not be re-run",
      status: "unverifiable_here",
    };
  }
  const cwd = claim.cwd === undefined ? scratch : join(scratch, claim.cwd);
  const timeoutMs = claim.timeoutMs ?? DEFAULT_CLAIM_TIMEOUT_MS;
  if (claim.kind === "command") {
    const outcome = runArgv(claim.command, cwd, timeoutMs);
    const verified = outcome.exitCode === 0;
    return {
      ...base,
      command: rendered,
      durationMs: Date.now() - startedAt,
      exitCode: outcome.exitCode,
      outputTail: boundTail(outcome.output),
      reason: verified
        ? "re-ran clean (exit 0) from the claimed commit"
        : `command exited ${outcome.exitCode ?? "null (unspawnable/timeout)"}`,
      status: verified ? "verified" : "failed",
    };
  }
  // adversarial: mutate, expect the cited proof to fail, restore.
  const filePath = join(scratch, claim.mutation.file);
  let original: string;
  try {
    original = readFileSync(filePath, "utf8");
  } catch {
    return {
      ...base,
      command: rendered,
      durationMs: Date.now() - startedAt,
      reason: `mutation target ${claim.mutation.file} is unreadable at the claimed commit`,
      status: "unverifiable_here",
    };
  }
  const mutated = applyMutation(original, claim.mutation.find, claim.mutation.replace);
  if (mutated === undefined) {
    return {
      ...base,
      command: rendered,
      durationMs: Date.now() - startedAt,
      reason: `mutation anchor not found in ${claim.mutation.file} at the claimed commit`,
      status: "unverifiable_here",
    };
  }
  let outcome: ExecOutcome;
  try {
    writeFileSync(filePath, mutated);
    outcome = runArgv(claim.command, cwd, timeoutMs);
  } finally {
    writeFileSync(filePath, original);
  }
  const caught = outcome.exitCode !== 0 && outcome.exitCode !== null;
  return {
    ...base,
    command: rendered,
    durationMs: Date.now() - startedAt,
    exitCode: outcome.exitCode,
    outputTail: boundTail(outcome.output),
    reason: caught
      ? `cited proof caught the deliberately broken behavior (exit ${outcome.exitCode}) — guarded behavior: ${claim.mutation.file}`
      : outcome.exitCode === 0
        ? "cited proof STILL PASSED with the guarded behavior deliberately broken — the claim's proof does not guard what it says"
        : "cited proof could not run against the mutated copy (unspawnable/timeout)",
    status: caught ? "verified" : outcome.exitCode === 0 ? "failed" : "unverifiable_here",
  };
};

const flagValue = (argv: readonly string[], flag: string): string | undefined => {
  const index = argv.indexOf(flag);
  return index >= 0 ? argv[index + 1] : undefined;
};

const usage = (): never => {
  console.error(
    "usage: pnpm run qa:verify -- --issue <n> --commit <sha> --claims <file.json> --actor <verifier> " +
      "[--out-dir <dir>] [--scratch-dir <dir>] [--keep-scratch] [--skip-install]",
  );
  process.exit(2);
};

export const runVerifier = (argv: readonly string[]): number => {
  const issueRaw = flagValue(argv, "--issue");
  const commitArg = flagValue(argv, "--commit");
  const claimsPath = flagValue(argv, "--claims");
  const actor = flagValue(argv, "--actor");
  if (
    issueRaw === undefined ||
    commitArg === undefined ||
    claimsPath === undefined ||
    actor === undefined
  ) {
    usage();
  }
  const issue = Number(issueRaw);
  if (!Number.isInteger(issue) || issue <= 0) usage();

  const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const outDir = flagValue(argv, "--out-dir") ?? join(repoRoot, "docs/qa/verifier/results");
  const keepScratch = argv.includes("--keep-scratch");
  const skipInstall = argv.includes("--skip-install");

  let decodedUnknown: unknown;
  try {
    decodedUnknown = JSON.parse(readFileSync(resolve(claimsPath as string), "utf8"));
  } catch (error) {
    console.error(
      `[qa-verify] cannot read claims file: ${error instanceof Error ? error.message : String(error)}`,
    );
    return 2;
  }
  const decoded = decodeVerifierClaims(decodedUnknown);
  if ("problems" in decoded) {
    console.error("[qa-verify] claims file is invalid:");
    for (const problem of decoded.problems) console.error(`  - ${problem}`);
    return 2;
  }
  const claimsFile: VerifierClaimsFile = decoded.file;
  if (claimsFile.issue !== issue) {
    console.error(
      `[qa-verify] --issue ${issue} does not match claims file issue ${claimsFile.issue}`,
    );
    return 2;
  }

  const independence = independenceProblem(claimsFile.implementer, actor as string);
  if (independence !== undefined) {
    console.error(`[qa-verify] REFUSED: ${independence}`);
    return 2;
  }

  // Resolve both the CLI commit and the claims-file commit; they must agree.
  const gitRevParse = (ref: string): string | undefined => {
    try {
      return execFileSync("git", ["-C", repoRoot, "rev-parse", "--verify", `${ref}^{commit}`], {
        encoding: "utf8",
      }).trim();
    } catch {
      return undefined;
    }
  };
  const resolvedCommit = gitRevParse(commitArg as string);
  const resolvedClaimsCommit = gitRevParse(claimsFile.commit);
  if (resolvedCommit === undefined) {
    console.error(
      `[qa-verify] --commit ${commitArg} does not resolve to a commit in this repository`,
    );
    return 2;
  }
  if (resolvedCommit !== resolvedClaimsCommit) {
    console.error(
      `[qa-verify] --commit resolves to ${resolvedCommit} but the claims file names ${claimsFile.commit} (${resolvedClaimsCommit ?? "unresolvable"})`,
    );
    return 2;
  }

  const scratch =
    flagValue(argv, "--scratch-dir") ?? mkdtempSync(join(tmpdir(), `qa-verify-${issue}-`));
  const runAt = new Date().toISOString();
  console.log(`qa-verify ${runAt} — work unit #${issue} @ ${resolvedCommit}`);
  console.log(
    `verifier: ${actor}${claimsFile.implementer === undefined ? "" : ` (implementer: ${claimsFile.implementer})`}`,
  );
  console.log(`scratch: ${scratch}`);

  let worktreeAdded = false;
  const results: ClaimResult[] = [];
  const setupResults: SetupResult[] = [];
  let installOk = false;
  let installTail: string | undefined;
  try {
    try {
      execFileSync(
        "git",
        ["-C", repoRoot, "worktree", "add", "--detach", scratch, resolvedCommit],
        {
          encoding: "utf8",
          stdio: ["ignore", "pipe", "pipe"],
        },
      );
      worktreeAdded = true;
    } catch (error) {
      console.error(
        `[qa-verify] scratch worktree creation failed: ${error instanceof Error ? error.message : String(error)}`,
      );
      return 2;
    }

    if (skipInstall) {
      installOk = true;
      installTail = "(install skipped by --skip-install)";
    } else {
      console.log("installing dependencies in the scratch checkout…");
      const install = runArgv(["pnpm", "install", "--prefer-offline"], scratch, INSTALL_TIMEOUT_MS);
      installOk = install.exitCode === 0;
      installTail = boundTail(install.output, 600);
      console.log(`install: ${installOk ? "ok" : `FAILED (exit ${install.exitCode})`}`);
    }

    for (const step of claimsFile.setup ?? []) {
      const startedAt = Date.now();
      const cwd = step.cwd === undefined ? scratch : join(scratch, step.cwd);
      const outcome = installOk
        ? runArgv(step.command, cwd, step.timeoutMs ?? DEFAULT_CLAIM_TIMEOUT_MS)
        : { exitCode: null, output: "skipped: install failed" };
      setupResults.push({
        command: renderArgv(step.command),
        durationMs: Date.now() - startedAt,
        exitCode: outcome.exitCode,
        outputTail: boundTail(outcome.output, 400),
        title: step.title,
      });
      console.log(`setup [exit ${outcome.exitCode}] ${step.title}`);
    }

    for (const claim of claimsFile.claims) {
      console.log(`claim ${claim.id} (${claim.kind}) — running…`);
      const result = runClaim(claim, scratch, installOk);
      results.push(result);
      console.log(
        `  [${result.status}] ${result.reason === undefined ? "" : result.reason.slice(0, 200)}`,
      );
    }
  } finally {
    if (worktreeAdded && !keepScratch) {
      try {
        execFileSync("git", ["-C", repoRoot, "worktree", "remove", "--force", scratch], {
          encoding: "utf8",
          stdio: ["ignore", "pipe", "pipe"],
        });
      } catch {
        rmSync(scratch, { force: true, recursive: true });
        try {
          execFileSync("git", ["-C", repoRoot, "worktree", "prune"], { encoding: "utf8" });
        } catch {
          // prune is best-effort
        }
      }
    }
  }

  const { verdict, reasons } = computeVerdict(results);
  const artifact: VerdictArtifact = {
    claims: results,
    ...(claimsFile.implementer === undefined ? {} : { implementer: claimsFile.implementer }),
    runAt,
    schemaVersion: QA_VERIFIER_VERDICT_SCHEMA,
    scratch: { installOk, ...(installTail === undefined ? {} : { installTail }) },
    setup: setupResults,
    verdict,
    verdictReasons: reasons,
    verifier: actor as string,
    verifierIssue: QA_VERIFIER_ISSUE,
    workUnit: {
      claimedCommit: claimsFile.commit,
      issue,
      resolvedCommit,
      source: claimsFile.source,
    },
  };
  mkdirSync(outDir, { recursive: true });
  const artifactPath = join(outDir, artifactFileName(issue, resolvedCommit));
  writeFileSync(artifactPath, `${JSON.stringify(artifact, null, 2)}\n`);

  console.log("");
  console.log(`VERDICT: ${verdict}`);
  for (const reason of reasons) console.log(`  - ${reason}`);
  console.log(`artifact: ${artifactPath}`);
  console.log("");
  console.log("ready-to-post comment (coordinator posts it, not the verifier):");
  console.log("-----8<-----");
  console.log(buildVerdictComment(artifact));
  console.log("-----8<-----");

  return verdict === "accept" ? 0 : verdict === "reject" ? 1 : 3;
};

if (import.meta.main) {
  process.exit(runVerifier(process.argv.slice(2)));
}
