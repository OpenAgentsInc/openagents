/**
 * Headless per-harness smoke runner (#9161 slice 1).
 *
 * Drives one real coding-agent turn programmatically — no Electron, no
 * Playwright — and grades the resulting conversation with the coherence
 * screening grader (#9160). Slice 1 supports the Codex harness only, over
 * `codex exec --json` with the owner's live default Codex home (exec is
 * ordinary owner usage; login flows against `~/.codex` remain forbidden).
 *
 * Usage:
 *   pnpm run headless:harness                       # codex, gpt-5.6-terra, medium
 *   pnpm run headless:harness -- --prompt "hey who are you" --grade
 *   pnpm run headless:harness -- --model gpt-5.6-terra --effort medium \
 *     --workdir /tmp/my-disposable-dir --json /tmp/run.json --grade
 *
 * The workdir defaults to a fresh disposable directory under the OS temp
 * root. The sandbox is read-only, so a smoke turn cannot write into the
 * workspace. Raw provider text stays local.
 */

import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import {
  codexBinaryCandidates,
  codexExecArgs,
  parseCodexExecOutput,
  summarizeCodexRun,
} from "./headless-harness-core";

const args = process.argv.slice(2);

const flagValue = (name: string): string | null => {
  const index = args.indexOf(name);
  if (index === -1 || index + 1 >= args.length) return null;
  return args[index + 1];
};

const harness = flagValue("--harness") ?? "codex";
const model = flagValue("--model") ?? "gpt-5.6-terra";
const effort = flagValue("--effort") ?? "medium";
const prompt = flagValue("--prompt") ?? "hey who are you";
const workdir = flagValue("--workdir") ?? mkdtempSync(join(tmpdir(), "oa-headless-"));
const jsonOut = flagValue("--json");
const grade = args.includes("--grade");
const sandboxFlag = flagValue("--sandbox") ?? "read-only";
if (sandboxFlag !== "read-only" && sandboxFlag !== "workspace-write") {
  console.error(`invalid --sandbox "${sandboxFlag}" (read-only | workspace-write)`);
  process.exit(2);
}
const sandbox = sandboxFlag as "read-only" | "workspace-write";
const timeoutMs = Number(flagValue("--timeout-ms") ?? 180_000);

if (harness !== "codex") {
  console.error(
    `harness_not_supported: "${harness}" — slice 1 implements the codex lane only`,
  );
  process.exit(2);
}

const binary =
  flagValue("--binary") ?? codexBinaryCandidates(homedir()).find((path) => existsSync(path));
if (binary === undefined || binary === null) {
  console.error("spawn_failed: no codex binary found in the known candidate paths");
  process.exit(2);
}

console.log(`headless harness smoke: ${harness} model=${model} effort=${effort} sandbox=${sandboxFlag}`);
console.log(`binary: ${binary}`);
console.log(`workdir: ${workdir}`);

const startedAt = new Date().toISOString();
const result = spawnSync(binary, [...codexExecArgs({ model, effort, workdir, prompt, sandbox })], {
  stdio: ["ignore", "pipe", "pipe"],
  encoding: "utf8",
  timeout: timeoutMs,
});

if (result.error !== undefined) {
  console.error(`spawn_failed: ${String(result.error)}`);
  process.exit(2);
}

const events = parseCodexExecOutput(result.stdout ?? "");
const summary = summarizeCodexRun(events);

console.log(`status: ${summary.status}`);
if (summary.threadId !== null) console.log(`threadId: ${summary.threadId}`);
if (summary.finalAnswer !== null) console.log(`answer: ${summary.finalAnswer}`);
if (summary.usage !== null) {
  console.log(
    `usage: input=${summary.usage.inputTokens} cached=${summary.usage.cachedInputTokens} ` +
      `output=${summary.usage.outputTokens} reasoning=${summary.usage.reasoningOutputTokens}`,
  );
}
if (summary.failureClass !== null) {
  console.error(`failureClass: ${summary.failureClass}`);
  console.error(`failureMessage: ${summary.failureMessage ?? ""}`);
}

/** Locate the rollout transcript for the thread under the live Codex home. */
const findRollout = (threadId: string): string | null => {
  const sessionsRoot = join(homedir(), ".codex", "sessions");
  const stack = [sessionsRoot];
  while (stack.length > 0) {
    const dir = stack.pop();
    if (dir === undefined) continue;
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) stack.push(full);
      else if (entry.isFile() && entry.name.includes(threadId)) return full;
    }
  }
  return null;
};

const rollout = summary.threadId === null ? null : findRollout(summary.threadId);
if (rollout !== null) {
  console.log(`rollout: ${rollout}`);
  try {
    console.log(`rollout bytes: ${statSync(rollout).size}`);
  } catch {
    // size is informational only
  }
}

if (jsonOut !== null) {
  writeFileSync(
    jsonOut,
    `${JSON.stringify({ startedAt, harness, model, effort, prompt, workdir, binary, summary, events, rollout }, null, 2)}\n`,
  );
  console.log(`wrote ${jsonOut}`);
}

if (grade && rollout !== null) {
  console.log("--- coherence screening ---");
  const graded = spawnSync(
    process.execPath,
    ["--import", "tsx", join(import.meta.dirname, "grade-conversation-coherence.ts"), rollout],
    { stdio: "inherit", timeout: 120_000 },
  );
  if (graded.status !== 0) {
    console.error("grading failed");
    process.exit(1);
  }
} else if (grade) {
  console.error("grading skipped: no rollout transcript located");
}

process.exit(summary.status === "completed" ? 0 : 1);
