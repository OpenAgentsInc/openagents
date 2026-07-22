/**
 * Deterministic conversation-coherence screening CLI.
 *
 * Grades local agent conversations with the machine-checkable layer of
 * docs/analysis/conversation-thread-coherence-rubric.md. See
 * docs/analysis/deterministic-coherence-screening.md for the metric
 * definition and docs/analysis/coherence-flywheel.md for the cadence.
 *
 * Usage:
 *   node --import tsx scripts/grade-conversation-coherence.ts \
 *     [--codex-root <dir>] [--claude-root <dir>] [--json <out-file>] \
 *     [--since YYYY-MM-DD] [--limit N] [--evidence] [--worst N] [paths...]
 *
 * Defaults scan ~/.codex/sessions and ~/.claude/projects. Explicit paths
 * are graded by extension of their location (codex vs claude layout).
 * Output stays local. Raw conversation content never leaves the machine.
 */

import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import {
  aggregateBySource,
  parseClaudeConversation,
  parseCodexConversation,
  scoreConversation,
  type ScoredConversation,
} from "./coherence-core";

const args = process.argv.slice(2);

const flagValue = (name: string): string | null => {
  const index = args.indexOf(name);
  if (index === -1 || index + 1 >= args.length) return null;
  return args[index + 1];
};

const codexRoot = flagValue("--codex-root") ?? join(homedir(), ".codex", "sessions");
const claudeRoot = flagValue("--claude-root") ?? join(homedir(), ".claude", "projects");
const jsonOut = flagValue("--json");
const since = flagValue("--since");
const limitRaw = flagValue("--limit");
const limit = limitRaw === null ? Number.POSITIVE_INFINITY : Number(limitRaw);
const includeEvidence = args.includes("--evidence");
const worstRaw = flagValue("--worst");
const worstCount = worstRaw === null ? 10 : Number(worstRaw);
const explicitPaths = args.filter(
  (arg, index) => !arg.startsWith("--") && (index === 0 || !args[index - 1].startsWith("--")),
);

const listJsonlFiles = (root: string, skipDirNames: readonly string[]): string[] => {
  const found: string[] = [];
  const walk = (dir: string): void => {
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        if (skipDirNames.includes(entry.name)) continue;
        walk(join(dir, entry.name));
      } else if (entry.isFile() && entry.name.endsWith(".jsonl")) {
        found.push(join(dir, entry.name));
      }
    }
  };
  walk(root);
  return found.sort();
};

const fileIsRecentEnough = (path: string): boolean => {
  if (since === null) return true;
  try {
    return statSync(path).mtime.toISOString().slice(0, 10) >= since;
  } catch {
    return false;
  }
};

const gradeFile = (path: string, source: "codex" | "claude-code"): ScoredConversation | null => {
  let content: string;
  try {
    content = readFileSync(path, "utf8");
  } catch {
    return null;
  }
  const parsed =
    source === "codex"
      ? parseCodexConversation(path, content)
      : parseClaudeConversation(path, content);
  return scoreConversation(parsed);
};

const conversations: ScoredConversation[] = [];

if (explicitPaths.length > 0) {
  for (const raw of explicitPaths) {
    const path = resolve(raw);
    const source = path.includes(".codex") ? "codex" : "claude-code";
    const scored = gradeFile(path, source);
    if (scored !== null) conversations.push(scored);
  }
} else {
  const codexFiles = listJsonlFiles(codexRoot, []).filter(fileIsRecentEnough).slice(0, limit);
  // Skip subagent sidechains: the main-thread lines carry the user turns.
  const claudeFiles = listJsonlFiles(claudeRoot, ["subagents", "tasks"])
    .filter(fileIsRecentEnough)
    .slice(0, limit);
  for (const path of codexFiles) {
    const scored = gradeFile(path, "codex");
    if (scored !== null) conversations.push(scored);
  }
  for (const path of claudeFiles) {
    const scored = gradeFile(path, "claude-code");
    if (scored !== null) conversations.push(scored);
  }
}

const aggregates = aggregateBySource(conversations);
const graded = conversations.filter((item) => item.disposition !== "skipped");
const worst = [...graded]
  .sort((left, right) => left.score - right.score)
  .slice(0, Math.max(0, worstCount));

console.log("conversation coherence screening (deterministic layer)");
console.log(`graded ${graded.length} conversations, skipped ${conversations.length - graded.length} without a full user/assistant turn`);
for (const aggregate of aggregates) {
  if (aggregate.graded === 0 && aggregate.skipped === 0) continue;
  console.log(
    `  ${aggregate.source}: graded=${aggregate.graded} mean=${aggregate.meanScore} median=${aggregate.medianScore} ` +
      `A=${aggregate.gradeCounts.A} B=${aggregate.gradeCounts.B} C=${aggregate.gradeCounts.C} ` +
      `D=${aggregate.gradeCounts.D} F=${aggregate.gradeCounts.F} needs_review=${aggregate.needsReview} ` +
      `signals[profanity=${aggregate.signalCounts.profanity} correction=${aggregate.signalCounts.correction} ` +
      `interrupt=${aggregate.signalCounts.interrupt}]`,
  );
  console.log(
    `    complexity: mean=${aggregate.meanComplexity} ` +
      `tiers[C0=${aggregate.tierCounts.C0} C1=${aggregate.tierCounts.C1} C2=${aggregate.tierCounts.C2} ` +
      `C3=${aggregate.tierCounts.C3} C4=${aggregate.tierCounts.C4}] ` +
      `complexity-weighted coherence=${aggregate.complexityWeightedCoherence}`,
  );
}
if (worst.length > 0) {
  console.log(`lowest-scoring conversations (${worst.length}):`);
  for (const item of worst) {
    console.log(
      `  ${item.score} ${item.grade} cx=${item.complexity.score} ${item.complexity.tier} ${item.source} ` +
        `turns=${item.userTurnCount} prof=${item.deductions.profanity} corr=${item.deductions.correction} ` +
        `int=${item.deductions.interrupt} ${item.path}`,
    );
  }
}

if (jsonOut !== null) {
  const records = conversations.map((item) => ({
    ...item,
    signals: includeEvidence
      ? item.signals.map((signal) => ({ ...signal, excerpt: signal.excerpt.slice(0, 120) }))
      : item.signals.map((signal) => ({ kind: signal.kind, userTurnIndex: signal.userTurnIndex })),
  }));
  writeFileSync(
    jsonOut,
    `${JSON.stringify({ generatedAt: new Date().toISOString(), aggregates, conversations: records }, null, 2)}\n`,
  );
  console.log(`wrote ${jsonOut}`);
}
