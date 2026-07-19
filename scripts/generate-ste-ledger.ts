import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  countDiagnostics,
  deriveProfile,
  inspectStructure,
  isGovernedPath,
  readCheckerConfig,
} from "./ste-core";

const root = resolve(import.meta.dirname, "..");
const check = process.argv.includes("--check");
const refreshBaseline = process.argv.includes("--refresh-baseline");
const config = readCheckerConfig(root);
const paths = execFileSync(
  "git",
  ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
  { cwd: root },
)
  .toString("utf8")
  .split("\0")
  .filter(Boolean)
  .filter((path) => isGovernedPath(path, config))
  .toSorted();
const profiles = paths.map((path) => deriveProfile(path, config));
let previousBaseline: Record<string, Record<string, number>> = {};
try {
  previousBaseline =
    (
      JSON.parse(readFileSync(`${root}/docs/ste/structural-baseline.v1.json`, "utf8")) as {
        files?: Record<string, Record<string, number>>;
      }
    ).files ?? {};
} catch {}
const baseline = Object.fromEntries(
  profiles
    .filter((profile) => profile.ste_status === "migration")
    .map((profile) => {
      const text = readFileSync(`${root}/${profile.path}`, "utf8");
      const current = countDiagnostics(inspectStructure(profile.path, text, profile.ste_mode));
      return [
        profile.path,
        refreshBaseline ? current : (previousBaseline[profile.path] ?? current),
      ];
    }),
);

const ledger = `${JSON.stringify(
  {
    schema: "openagents-ste-ledger-v1",
    generatedFrom: "git ls-files",
    steIssue: 9,
    glossaryRevision: config.glossaryRevision,
    profiles,
  },
  null,
  2,
)}\n`;
const baselineText = `${JSON.stringify(
  {
    schema: "openagents-ste-baseline-v1",
    policyRevision: config.policyRevision,
    note: "A baseline is a migration ratchet. It is not proof of conformance.",
    files: baseline,
  },
  null,
  2,
)}\n`;
const outputs = [
  ["docs/ste/migration-ledger.v1.json", ledger],
  ["docs/ste/structural-baseline.v1.json", baselineText],
] as const;

let stale = false;
for (const [path, output] of outputs) {
  const absolute = `${root}/${path}`;
  if (check) {
    let current = "";
    try {
      current = readFileSync(absolute, "utf8");
    } catch {}
    if (current !== output) {
      console.error(`${path} is stale; run pnpm run generate:ste-ledger`);
      stale = true;
    }
  } else {
    writeFileSync(absolute, output);
    console.log(`wrote ${path}`);
  }
}
if (stale) process.exitCode = 1;
