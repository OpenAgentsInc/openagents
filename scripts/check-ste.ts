import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  applyScreeningReview,
  agentCompactLineNumbers,
  countDiagnostics,
  dictionaryWords,
  extractProse,
  inspectStructure,
  isGovernedPath,
  readCheckerConfig,
  validateGlossary,
  validateAgentCompactTerms,
  type SteDiagnostic,
  type SteProfile,
} from "./ste-core";

const root = resolve(import.meta.dirname, "..");
const args = process.argv.slice(2);
const strict = args.includes("--strict");
const all = args.includes("--all");
const explicitPaths = args.filter((arg) => !arg.startsWith("--"));
const config = readCheckerConfig(root);

const ledger = JSON.parse(readFileSync(`${root}/docs/ste/migration-ledger.v1.json`, "utf8")) as {
  profiles: SteProfile[];
  steIssue: number;
  glossaryRevision: string;
};
const baseline = JSON.parse(
  readFileSync(`${root}/docs/ste/structural-baseline.v1.json`, "utf8"),
) as { files: Record<string, Record<string, number>> };
const glossaryValue = JSON.parse(readFileSync(`${root}/docs/ste/glossary.v1.json`, "utf8")) as {
  revision: string;
  terms: Array<{ permittedForms: string[]; prohibitedSynonyms: string[] }>;
};
const agentCompactValue = JSON.parse(
  readFileSync(`${root}/docs/ste/agent-compact-terms.v1.json`, "utf8"),
) as {
  revision: string;
  baseGlossaryRevision: string;
  terms: Array<{ term: string; permittedForms: string[] }>;
};
const configurationErrors = [
  ...validateGlossary(glossaryValue),
  ...validateAgentCompactTerms(agentCompactValue),
];
if (
  ledger.steIssue !== 9 ||
  ledger.glossaryRevision !== config.glossaryRevision ||
  glossaryValue.revision !== config.glossaryRevision
)
  configurationErrors.push("ledger, checker, and glossary revisions must agree");
if (
  agentCompactValue.revision !== config.agentCompactRevision ||
  agentCompactValue.baseGlossaryRevision !== config.glossaryRevision
)
  configurationErrors.push("agent compact, checker, and glossary revisions must agree");
if (configurationErrors.length > 0) {
  for (const error of configurationErrors) console.error(`STE-CONFIG: ${error}`);
  process.exit(1);
}

const tracked = (): string[] =>
  execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"], {
    cwd: root,
  })
    .toString("utf8")
    .split("\0")
    .filter(Boolean);
const changed = (): string[] => {
  const candidates = new Set<string>();
  const commands = [
    ["diff", "--name-only", "--diff-filter=ACMR", "HEAD"],
    ["diff", "--name-only", "--cached", "--diff-filter=ACMR"],
  ];
  for (const command of commands) {
    for (const path of execFileSync("git", command, { cwd: root })
      .toString("utf8")
      .split(/\r?\n/)
      .filter(Boolean))
      candidates.add(path);
  }
  if (candidates.size === 0) {
    try {
      for (const path of execFileSync(
        "git",
        ["diff", "--name-only", "--diff-filter=ACMR", "@{upstream}...HEAD"],
        { cwd: root },
      )
        .toString("utf8")
        .split(/\r?\n/)
        .filter(Boolean))
        candidates.add(path);
    } catch {}
  }
  return [...candidates];
};
const selected = (explicitPaths.length > 0 ? explicitPaths : all ? tracked() : changed())
  .filter((path) => isGovernedPath(path, config) && existsSync(`${root}/${path}`))
  .toSorted();
const profiles = new Map(ledger.profiles.map((profile) => [profile.path, profile]));
const diagnostics: SteDiagnostic[] = [];
const errors: string[] = [];

let approvedWords: ReadonlySet<string> | undefined;
if (strict) {
  const dictionaryPath = process.env.ASD_STE100_DICTIONARY;
  if (!dictionaryPath) errors.push("strict mode requires ASD_STE100_DICTIONARY");
  else approvedWords = dictionaryWords(dictionaryPath);
}
const companyForms = new Set(
  glossaryValue.terms
    .flatMap((term) => term.permittedForms)
    .flatMap((form) => form.toLowerCase().split(/\s+/)),
);
const agentCompactForms = new Set(
  agentCompactValue.terms
    .flatMap((term) => term.permittedForms)
    .flatMap((form) => form.toLowerCase().split(/\s+/)),
);

for (const path of selected) {
  const profile = profiles.get(path);
  if (!profile) {
    errors.push(`${path}: STE-PROFILE: add the file to the migration ledger`);
    continue;
  }
  if (profile.ste_issue !== 9 || profile.ste_glossary_revision !== config.glossaryRevision)
    errors.push(`${path}: STE-PROFILE: use Issue 9 and ${config.glossaryRevision}`);
  if (
    profile.ste_agent_compact_revision &&
    profile.ste_agent_compact_revision !== config.agentCompactRevision
  )
    errors.push(`${path}: STE-PROFILE: use ${config.agentCompactRevision}`);
  if (profile.ste_agent_compact_revision && !["agent", "dual"].includes(profile.ste_audience ?? ""))
    errors.push(`${path}: STE-PROFILE: identify an agent or dual audience`);
  if (
    (profile.ste_status === "inspected" || profile.ste_status === "source-data") &&
    (!profile.ste_reviewer || !profile.ste_reviewed_at) &&
    profile.ste_status !== "source-data"
  )
    errors.push(`${path}: STE-PROFILE: add the reviewer and review time`);
  if (profile.ste_mode === "source-data") continue;
  const text = readFileSync(`${root}/${path}`, "utf8");
  const current = applyScreeningReview(inspectStructure(path, text, profile.ste_mode), profile);
  if (strict || profile.ste_status !== "migration") diagnostics.push(...current);
  else {
    const currentCounts = countDiagnostics(current);
    const allowed = baseline.files[path] ?? {};
    for (const item of current)
      if ((currentCounts[item.rule] ?? 0) > (allowed[item.rule] ?? 0)) diagnostics.push(item);
  }

  if (strict && approvedWords) {
    const agentLines = profile.ste_agent_compact_revision
      ? agentCompactLineNumbers(text, profile.ste_audience)
      : new Set<number>();
    for (const line of extractProse(text)) {
      for (const match of line.text.matchAll(/\b[A-Za-z]+(?:-[A-Za-z0-9]+)*\b/g)) {
        const word = match[0].toLowerCase();
        if (
          approvedWords.has(word) ||
          companyForms.has(word) ||
          (agentLines.has(line.number) && agentCompactForms.has(word))
        )
          continue;
        diagnostics.push({
          rule: "STE-1.1",
          path,
          line: line.number,
          column: (match.index ?? 0) + 1,
          action: "Use an approved dictionary word or add an inspected technical term.",
          text: match[0],
        });
      }
    }
  }
  if (strict || profile.ste_status !== "migration")
    for (const term of glossaryValue.terms) {
      for (const synonym of term.prohibitedSynonyms) {
        const escaped = synonym.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        const match = text.match(new RegExp(`\\b${escaped}\\b`, "i"));
        if (match)
          diagnostics.push({
            rule: "OA-STE-TERM",
            path,
            line: text.slice(0, match.index).split(/\r?\n/).length,
            column: 1,
            action: `Replace the prohibited synonym with ${term.permittedForms[0]}.`,
            text: synonym,
          });
      }
    }
}

for (const error of errors) console.error(error);
const unique = [
  ...new Map(
    diagnostics.map((item) => [`${item.rule}:${item.path}:${item.line}:${item.column}`, item]),
  ).values(),
];
for (const item of unique)
  console.error(
    `${item.path}:${item.line}:${item.column} ${item.rule} ${item.action} [${item.text}]`,
  );
if (errors.length > 0 || unique.length > 0) process.exitCode = 1;
else
  console.log(
    `check:ste OK (${selected.length} governed files, ${strict ? "strict" : "migration-ratchet"} mode, Issue 9, ${config.glossaryRevision})`,
  );
