// Fail the build when code is proven real by a test and called by nothing in
// production.
//
// The defect class this exists for is not "unused code". It is narrower and far
// more expensive: code that exists, ships, passes its own tests, and is never
// reached from any production path. Ten instances were found in a single session
// (see docs/quality/uncalled-production-symbols.md, and the after-action audit
// docs/afteraction/2026-07-31-codex-019fb495-overnight-spend-audit.md for the
// session that surfaced them). Every one was green in CI. The passing test is
// what made them invisible: the test is the only caller, so coverage, types, and
// lint all agree the code is fine.
//
// `scripts/sarah-participant-join-authority-guard.mjs` is the hand-written
// ancestor of this guard. It names three participant-admission authorities and
// requires each to keep a production caller. This guard generalizes that rule to
// the whole TypeScript surface instead of growing a list of names.
//
// THE RULES
//
// 1. EXPORTED VALUE. An exported value declaration (function / const / let /
//    class) is flagged when
//      a. at least one test or fixture file references it — the behavior is real
//         and someone meant it to work; and
//      b. no production file other than the file that declares it references it;
//         and
//      c. its declaring file does not name it again — a module that wires its own
//         export into something else it exports is live, and only the declaration
//         itself appears exactly once.
//
// 2. SERVICE-INTERFACE MEMBER. A function-typed member of an exported interface
//    is flagged when a test dot-accesses it and no production file anywhere
//    does. Declaring a member and implementing it are both declarations; only a
//    `.member` access is a call. This is the shape rule 1 cannot see, because an
//    interface member and its class implementation live in one file and look like
//    two mentions of a live symbol.
//
// A symbol that nothing references at all — not even a test — is ordinary dead
// code and is deliberately NOT flagged: lower stakes, far noisier, and already
// the domain of unused-export tooling. The signal this guard trades on is exactly
// the contradiction in "tested but uncalled".
//
// ENFORCEMENT
//   Findings that predate the guard live in `inheritedDebt` in the baseline file.
//   That list is a dated record of a known defect, never an approval. Any NEW
//   finding fails the build. Any `inheritedDebt` entry that stops flagging must be
//   removed, so the ledger can only shrink. Intentional, permanent exceptions go
//   in `allowed`, and each one requires a written reason.
//
// Usage:
//   node scripts/uncalled-production-symbol-guard.mjs [root]
//   node scripts/uncalled-production-symbol-guard.mjs [root] --list
//   node scripts/uncalled-production-symbol-guard.mjs [root] --prune
import { globSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const argv = process.argv.slice(2);
const root = argv.find((arg) => !arg.startsWith("--")) ?? ".";
const flags = new Set(argv.filter((arg) => arg.startsWith("--")));
const baselinePath = "scripts/uncalled-production-symbol-baseline.json";

const SOURCE_GLOBS = [
  "apps/**/*.ts",
  "apps/**/*.tsx",
  "packages/**/*.ts",
  "packages/**/*.tsx",
  "clients/**/*.ts",
  "clients/**/*.tsx",
  "scripts/**/*.ts",
];

const EXCLUDED_GLOBS = [
  "**/node_modules/**",
  "**/dist/**",
  "**/build/**",
  "**/out/**",
  "**/.next/**",
  "**/.expo/**",
  "**/coverage/**",
  "**/generated/**",
  "**/_generated/**",
  "**/*.d.ts",
];

// Names a framework calls by convention rather than by import. Their caller is a
// router, bundler, or host process, so "no named reference" proves nothing.
const CONVENTION_EXPORTS = new Set([
  "default",
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
  "loader",
  "action",
  "meta",
  "links",
  "headers",
  "middleware",
  "onRequest",
  "ErrorBoundary",
  "Component",
]);

const isTestSource = (file) =>
  /(^|\/)(tests?|__tests__|__mocks__|__fixtures__|fixtures|test-fixtures|mocks|stubs|e2e)\//u.test(
    file,
  ) ||
  /\.(test|spec)\.[cm]?tsx?$/u.test(file) ||
  /(^|\/)test-[^/]*\.[cm]?tsx?$/u.test(file);

const identifierPattern = /[A-Za-z_$][A-Za-z0-9_$]*/gu;
const dotAccessPattern = /\.([A-Za-z_$][A-Za-z0-9_$]*)/gu;

// `export const x`, `export async function x`, `export abstract class x`, and the
// `export declare` variants. Deliberately not `type` / `interface` / `enum`: a
// type with no runtime caller cannot ship a silent behavioral hole.
const declarationPattern =
  /^export\s+(?:declare\s+)?(?:async\s+)?(?:function\s*\*?|const|let|var|class|abstract\s+class)\s+([A-Za-z_$][A-Za-z0-9_$]*)/gmu;

const interfaceBlockPattern =
  /^export\s+(?:declare\s+)?interface\s+([A-Za-z_$][A-Za-z0-9_$]*)[^\n]*\{\n([\s\S]*?)\n\}/gmu;
const interfaceMemberPattern =
  /^\s{2}(?:readonly\s+)?([A-Za-z_$][A-Za-z0-9_$]*)\s*\??\s*:\s*\(/gmu;

const index = (map, identifier, file, isTest) => {
  let entry = map.get(identifier);
  if (entry === undefined) map.set(identifier, (entry = { production: new Set(), test: new Set() }));
  (isTest ? entry.test : entry.production).add(file);
};

const collect = () => {
  const files = globSync(SOURCE_GLOBS, { cwd: root, exclude: EXCLUDED_GLOBS })
    .map((file) => file.split("\\").join("/"))
    .toSorted();

  const named = new Map();
  const dotted = new Map();
  const sources = new Map();

  for (const file of files) {
    let text;
    try {
      text = readFileSync(join(root, file), "utf8");
    } catch {
      continue;
    }
    sources.set(file, text);
    const test = isTestSource(file);
    const seenNamed = new Set();
    for (const match of text.matchAll(identifierPattern)) {
      if (seenNamed.has(match[0])) continue;
      seenNamed.add(match[0]);
      index(named, match[0], file, test);
    }
    const seenDotted = new Set();
    for (const match of text.matchAll(dotAccessPattern)) {
      if (seenDotted.has(match[1])) continue;
      seenDotted.add(match[1]);
      index(dotted, match[1], file, test);
    }
  }

  const findings = [];
  const push = (ref, file, subject, entry, rule) => {
    findings.push({ ref, file, subject, rule, tests: [...entry.test].slice(0, 3) });
  };

  for (const [file, text] of sources) {
    if (isTestSource(file)) continue;

    const declared = new Set();
    for (const match of text.matchAll(declarationPattern)) {
      const symbol = match[1];
      if (declared.has(symbol) || CONVENTION_EXPORTS.has(symbol)) continue;
      declared.add(symbol);

      const entry = named.get(symbol);
      if (entry === undefined || entry.test.size === 0) continue;
      if ([...entry.production].some((candidate) => candidate !== file)) continue;

      const occurrences = text.match(new RegExp(`\\b${symbol}\\b`, "gu"));
      if (occurrences !== null && occurrences.length > 1) continue;

      push(`${file}#${symbol}`, file, symbol, entry, "exported-value");
    }

    const members = new Set();
    for (const block of text.matchAll(interfaceBlockPattern)) {
      for (const match of block[2].matchAll(interfaceMemberPattern)) {
        const member = match[1];
        const ref = `${file}#${block[1]}.${member}`;
        if (members.has(ref) || CONVENTION_EXPORTS.has(member)) continue;
        members.add(ref);

        const entry = dotted.get(member);
        if (entry === undefined || entry.test.size === 0) continue;
        if (entry.production.size > 0) continue;

        push(ref, file, `${block[1]}.${member}`, entry, "interface-member");
      }
    }
  }

  return findings.toSorted((left, right) => left.ref.localeCompare(right.ref));
};

const readBaseline = () => {
  let raw;
  try {
    raw = readFileSync(join(root, baselinePath), "utf8");
  } catch {
    return { inheritedDebt: [], allowed: [] };
  }
  const parsed = JSON.parse(raw);
  return { ...parsed, inheritedDebt: parsed.inheritedDebt ?? [], allowed: parsed.allowed ?? [] };
};

const findings = collect();

if (flags.has("--list")) {
  for (const finding of findings) console.log(`${finding.rule}\t${finding.ref}`);
  console.log(`${findings.length} finding(s).`);
  process.exit(0);
}

const baseline = readBaseline();

if (flags.has("--prune") || flags.has("--seed")) {
  const allowedRefs = new Set(baseline.allowed.map((entry) => entry.ref));
  const currentRefs = new Set(findings.map((finding) => finding.ref));
  // `--prune` may only REMOVE entries. If it could add them, any agent could
  // launder a fresh uncalled symbol into the ledger with one command and the
  // guard would enforce nothing. Seeding a ledger is a separate, explicit act
  // that works only while no ledger exists.
  const seeding = flags.has("--seed") && baseline.inheritedDebt.length === 0;
  if (flags.has("--seed") && !seeding) {
    console.error(
      `Refusing to --seed: ${baselinePath} already lists ${baseline.inheritedDebt.length} ` +
        "entries. Use --prune, which can only remove entries.",
    );
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
          "Debt ledger for scripts/uncalled-production-symbol-guard.mjs.",
          "inheritedDebt: code that only a test calls, recorded when the guard landed.",
          "It records a known defect. It is never an approval, and it may only shrink.",
          "allowed: intentional, permanent exceptions. Each one needs a written reason.",
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
  console.log(
    `Wrote ${baselinePath}: ${inheritedDebt.length} inherited, ${baseline.allowed.length} allowed.`,
  );
  process.exit(0);
}

const flagged = new Map(findings.map((finding) => [finding.ref, finding]));
const inherited = new Set(baseline.inheritedDebt);
const allowed = new Map(baseline.allowed.map((entry) => [entry.ref, entry]));

const failures = [];

for (const finding of findings) {
  if (inherited.has(finding.ref) || allowed.has(finding.ref)) continue;
  const called =
    finding.rule === "interface-member"
      ? "no production source calls it"
      : "no production source references it — not another module, and not its own";
  failures.push(
    `${finding.subject} (${finding.file}) is referenced by ${finding.tests.join(", ")}, but ` +
      `${called}. A test is its only caller, so nothing it does can happen in production. ` +
      "Call it from the live path, or delete it together with its test. If it is deliberately " +
      `not called yet, add {"ref": "${finding.ref}", "reason": "..."} to "allowed" in ` +
      `${baselinePath}, naming who will call it and when.`,
  );
}

for (const ref of baseline.inheritedDebt) {
  if (flagged.has(ref)) continue;
  failures.push(
    `${ref} is listed in "inheritedDebt" but no longer flags — it was called, renamed, or ` +
      "deleted. Remove the entry so the ledger keeps shrinking: " +
      "node scripts/uncalled-production-symbol-guard.mjs . --prune",
  );
}

for (const entry of baseline.allowed) {
  if (typeof entry.reason !== "string" || entry.reason.trim().length < 20) {
    failures.push(
      `${entry.ref} is in "allowed" without a usable reason. An exception with no stated reason ` +
        "is indistinguishable from the defect it claims to be an exception to.",
    );
    continue;
  }
  if (!flagged.has(entry.ref)) {
    failures.push(
      `${entry.ref} is in "allowed" but no longer flags. Remove the exception so a future ` +
        "uncalled symbol at that path cannot inherit its permission.",
    );
  }
}

if (failures.length > 0) {
  console.error("Uncalled production symbol guard failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(
  "Uncalled production symbol guard: no new test-only code " +
    `(${inherited.size} inherited, ${allowed.size} allowed).`,
);
