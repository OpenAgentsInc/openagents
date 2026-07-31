// Fail the build when a CLI cannot survive the way pnpm actually invokes it.
//
// This is the sibling of scripts/uncalled-production-symbol-guard.mjs. That guard
// catches code no production path reaches. This one catches the entry point
// itself being unreachable — the shape that let `gate-observation-cli.ts` ship
// tested and documented and never once invocable, which is why the receipts
// directory it writes to did not exist. See
// docs/quality/uncalled-production-symbols.md.
//
// THE RULE
//   pnpm forwards the `--` separator into argv verbatim. Verified against pnpm 11:
//
//     $ pnpm run echoargs -- --row x
//     ["--", "--row", "x"]
//
//   So a package script whose CLI throws on an unrecognized argument must skip a
//   bare `--`, or every documented `pnpm run <script> -- --flag` invocation dies
//   before its first line of work. `--` is never a meaningful argument, so
//   skipping it is always safe:
//
//     if (value === "--") continue
//
// Scope is deliberately narrow: only files a package.json script actually
// executes. An Electron main process or a library that parses argv for its own
// reasons is never reached through `pnpm run`, so the separator cannot hurt it.
//
// Enforcement matches the sibling guard: findings that predate this guard sit in
// `inheritedDebt` and may only shrink, new findings fail, and an intentional
// exception in `allowed` needs a written reason.
//
// Usage:
//   node scripts/cli-invocability-guard.mjs [root] [--list|--prune|--seed]
import { existsSync, globSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const argv = process.argv.slice(2);
const root = argv.find((arg) => !arg.startsWith("--")) ?? ".";
const flags = new Set(argv.filter((arg) => arg.startsWith("--")));
const baselinePath = "scripts/cli-invocability-baseline.json";

const PACKAGE_GLOBS = [
  "package.json",
  "apps/**/package.json",
  "packages/**/package.json",
  "clients/**/package.json",
];
const PACKAGE_EXCLUDES = ["**/node_modules/**", "**/dist/**", "**/build/**"];

const scriptTargetPattern = /([\w./-]+\.[cm]?tsx?)\b/gu;
// CLIs word this rejection many ways — "unknown argument", "unsupported or
// incomplete argument", "invalid option". The historical proof against
// gate-observation-cli.ts found the narrower pattern missed the very defect this
// guard exists for, so match the family rather than one phrasing.
const rejectsUnknownArgument =
  /(unknown|unrecognized|unexpected|unsupported|invalid|incomplete)[^\n]{0,40}?\b(argument|flag|option|switch)/iu;
// Only an explicit equality against the bare token counts as tolerating it.
// `next.startsWith("--")` contains the same characters and proves the opposite,
// which is how the first draft of this guard read the pre-fix
// gate-observation-cli.ts as already safe.
const toleratesSeparator = /(?:===|!==)\s*["'`]--["'`]|["'`]--["'`]\s*(?:===|!==)/u;

const collect = () => {
  const targets = new Set();
  for (const manifest of globSync(PACKAGE_GLOBS, { cwd: root, exclude: PACKAGE_EXCLUDES })) {
    const file = manifest.split("\\").join("/");
    let scripts;
    try {
      scripts = JSON.parse(readFileSync(join(root, file), "utf8")).scripts ?? {};
    } catch {
      continue;
    }
    for (const body of Object.values(scripts)) {
      for (const match of String(body).matchAll(scriptTargetPattern)) {
        const target = join(dirname(file), match[1]).split("\\").join("/");
        if (existsSync(join(root, target))) targets.add(target);
      }
    }
  }

  const findings = [];
  for (const target of targets) {
    let source;
    try {
      source = readFileSync(join(root, target), "utf8");
    } catch {
      continue;
    }
    if (!source.includes("process.argv")) continue;
    if (!rejectsUnknownArgument.test(source)) continue;
    if (toleratesSeparator.test(source)) continue;
    findings.push({
      ref: target,
      detail:
        `${target} runs as a package script, reads process.argv, and rejects an unrecognized ` +
        "argument, but never skips a bare `--`. pnpm forwards the separator verbatim, so " +
        '`pnpm run <script> -- --flag` reaches this file as ["--", "--flag"] and it fails before ' +
        'doing any work. Skip the token: if (value === "--") continue',
    });
  }

  return findings.toSorted((left, right) => left.ref.localeCompare(right.ref));
};

const readBaseline = () => {
  try {
    const parsed = JSON.parse(readFileSync(join(root, baselinePath), "utf8"));
    return { ...parsed, inheritedDebt: parsed.inheritedDebt ?? [], allowed: parsed.allowed ?? [] };
  } catch {
    return { inheritedDebt: [], allowed: [] };
  }
};

const findings = collect();

if (flags.has("--list")) {
  for (const finding of findings) console.log(finding.ref);
  console.log(`${findings.length} finding(s).`);
  process.exit(0);
}

const baseline = readBaseline();

if (flags.has("--prune") || flags.has("--seed")) {
  const allowedRefs = new Set(baseline.allowed.map((entry) => entry.ref));
  const currentRefs = new Set(findings.map((finding) => finding.ref));
  const seeding = flags.has("--seed") && baseline.inheritedDebt.length === 0;
  if (flags.has("--seed") && !seeding) {
    console.error(`Refusing to --seed: ${baselinePath} already has entries. Use --prune.`);
    process.exit(1);
  }
  const inheritedDebt = (
    seeding ? [...currentRefs] : baseline.inheritedDebt.filter((ref) => currentRefs.has(ref))
  )
    .filter((ref) => !allowedRefs.has(ref))
    .toSorted();
  mkdirSync(dirname(join(root, baselinePath)), { recursive: true });
  writeFileSync(
    join(root, baselinePath),
    `${JSON.stringify(
      {
        $comment: baseline.$comment ?? [
          "Debt ledger for scripts/cli-invocability-guard.mjs.",
          "inheritedDebt: package-script CLIs that reject the `--` pnpm forwards.",
          "It records a known defect, never an approval, and it may only shrink.",
          "allowed: intentional exceptions. Each one needs a written reason.",
          "Rationale: docs/quality/uncalled-production-symbols.md",
        ],
        baselinedAt: baseline.baselinedAt ?? new Date().toISOString().slice(0, 10),
        inheritedDebt,
        allowed: baseline.allowed,
      },
      null,
      2,
    )}\n`,
  );
  console.log(`Wrote ${baselinePath}: ${inheritedDebt.length} inherited.`);
  process.exit(0);
}

const flagged = new Map(findings.map((finding) => [finding.ref, finding]));
const inherited = new Set(baseline.inheritedDebt);
const allowed = new Map(baseline.allowed.map((entry) => [entry.ref, entry]));
const failures = [];

for (const finding of findings) {
  if (inherited.has(finding.ref) || allowed.has(finding.ref)) continue;
  failures.push(
    `${finding.detail}. If this CLI is deliberately unreachable through pnpm, add ` +
      `{"ref": "${finding.ref}", "reason": "..."} to "allowed" in ${baselinePath}.`,
  );
}

for (const ref of baseline.inheritedDebt) {
  if (flagged.has(ref)) continue;
  failures.push(
    `${ref} is listed in "inheritedDebt" but no longer flags. Remove it: ` +
      "node scripts/cli-invocability-guard.mjs . --prune",
  );
}

for (const entry of baseline.allowed) {
  if (typeof entry.reason !== "string" || entry.reason.trim().length < 20) {
    failures.push(
      `${entry.ref} is in "allowed" without a usable reason. An exception with no stated reason ` +
        "is indistinguishable from the defect it claims to be an exception to.",
    );
  } else if (!flagged.has(entry.ref)) {
    failures.push(`${entry.ref} is in "allowed" but no longer flags. Remove the exception.`);
  }
}

if (failures.length > 0) {
  console.error("CLI invocability guard failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  `CLI invocability guard: every package-script CLI survives its own invocation ` +
    `(${inherited.size} inherited, ${allowed.size} allowed).`,
);
