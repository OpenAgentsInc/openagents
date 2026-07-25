// Single source of truth for the repository's `node:test` suites.
//
// Some suites are written against Node's built-in test runner rather than Vite
// Plus / Vitest. Vitest's include glob still matches their filenames, so it
// loads them, finds no Vitest suite, and reports "No test suite found" — seven
// false failures for seven healthy suites whose 60 assertions never actually
// ran under `pnpm run test`.
//
// The obvious repair — excluding them from Vitest — is the exact defect this
// repository keeps getting bitten by: a suite that is quietly dropped from the
// sweep while the sweep still reports green. So discovery is dynamic and
// shared: `vite.config.ts` excludes exactly this set from Vitest, and
// `pnpm run test` hands exactly this set to `node --test`. A file cannot be
// excluded from one without being picked up by the other, and a new
// `node:test` suite is enrolled by existing, not by editing a list.
import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const SKIP_DIRECTORIES = new Set([
  ".claude",
  ".git",
  ".oa-launch",
  ".pylon-local",
  ".worktrees",
  "coverage",
  "dist",
  "dist-electron",
  "node_modules",
  "projects",
  "target",
  "var",
]);

const CANDIDATE = /\.(test|spec)\.(mjs|js|cjs)$/;
const IMPORTS_NODE_TEST = /from\s*["']node:test["']|require\(\s*["']node:test["']\s*\)/;

const walk = (absolute, found) => {
  for (const entry of fs.readdirSync(absolute, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (SKIP_DIRECTORIES.has(entry.name)) continue;
      walk(path.join(absolute, entry.name), found);
      continue;
    }
    if (!entry.isFile() || !CANDIDATE.test(entry.name)) continue;
    const file = path.join(absolute, entry.name);
    if (IMPORTS_NODE_TEST.test(fs.readFileSync(file, "utf8"))) {
      found.push(path.relative(repoRoot, file));
    }
  }
  return found;
};

/** Repo-relative paths of every suite written against Node's built-in runner. */
export const nodeTestSuites = () => walk(repoRoot, []).sort();

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const suites = nodeTestSuites();
  if (process.argv.includes("--list")) {
    for (const suite of suites) console.log(suite);
    process.exit(0);
  }
  if (suites.length === 0) {
    // Zero discovered suites means discovery broke, not that the repository
    // stopped having them. Never report that as a pass.
    console.error("[node-test-suites] discovered no node:test suites — discovery is broken.");
    process.exit(1);
  }
  console.log(`[node-test-suites] running ${suites.length} node:test suites`);
  const child = spawn(process.execPath, ["--test", ...suites], {
    cwd: repoRoot,
    stdio: "inherit",
  });
  child.on("close", (code) => process.exit(code ?? 1));
}
