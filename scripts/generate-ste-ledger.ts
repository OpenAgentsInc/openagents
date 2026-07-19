import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import {
  countDiagnostics,
  deriveProfile,
  inspectStructure,
  isGovernedPath,
  readCheckerConfig,
  type SteProfile,
} from "./ste-core";

const root = resolve(import.meta.dirname, "..");
const check = process.argv.includes("--check");
const refreshBaseline = process.argv.includes("--refresh-baseline");
const refreshPaths = new Set(
  process.argv
    .filter((argument) => argument.startsWith("--refresh-path="))
    .map((argument) => argument.slice("--refresh-path=".length)),
);
const config = readCheckerConfig(root);
const overrides = JSON.parse(
  readFileSync(`${root}/docs/ste/profile-overrides.v1.json`, "utf8"),
) as { profiles: Record<string, Partial<SteProfile>> };
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
const derivedProfiles = paths.map((path) => ({
  ...deriveProfile(path, config),
  ...overrides.profiles[path],
  path,
}));
for (const path of Object.keys(overrides.profiles)) {
  if (!paths.includes(path))
    throw new Error(`STE profile override does not match a governed file: ${path}`);
}
const finalInventoryPath = `${root}/docs/ste/final-inventory.v1.json`;
type FinalInventory = {
  steIssue: number;
  glossaryRevision: string;
  entries: Array<{ path: string; sha256: string; profile: Partial<SteProfile> }>;
};
let finalEntries = new Map<string, FinalInventory["entries"][number]>();
if (existsSync(finalInventoryPath)) {
  const inventory = JSON.parse(readFileSync(finalInventoryPath, "utf8")) as FinalInventory;
  if (inventory.steIssue !== 9 || inventory.glossaryRevision !== config.glossaryRevision) {
    throw new Error("Final STE inventory revision does not match the checker configuration");
  }
  finalEntries = new Map(inventory.entries.map((entry) => [entry.path, entry]));
  const missing = paths.filter((path) => !finalEntries.has(path));
  const extra = [...finalEntries.keys()].filter((path) => !paths.includes(path));
  if (missing.length > 0 || extra.length > 0) {
    throw new Error(
      `Final STE inventory path mismatch (missing=${missing.join(",") || "none"}; extra=${extra.join(",") || "none"})`,
    );
  }
  for (const entry of inventory.entries) {
    const digest = createHash("sha256").update(readFileSync(`${root}/${entry.path}`)).digest("hex");
    if (digest !== entry.sha256) {
      throw new Error(`Final STE inventory digest is stale: ${entry.path}`);
    }
  }
}
const profiles = derivedProfiles.map((profile) => ({
  ...profile,
  ...(finalEntries.get(profile.path)?.profile ?? {}),
  path: profile.path,
}));
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
        refreshBaseline || refreshPaths.has(profile.path)
          ? current
          : (previousBaseline[profile.path] ?? current),
      ];
    }),
);
const migrationPaths = new Set(
  profiles
    .filter((profile) => profile.ste_status === "migration")
    .map((profile) => profile.path),
);
for (const path of refreshPaths) {
  if (!migrationPaths.has(path)) {
    throw new Error(`STE baseline refresh path is not a migration file: ${path}`);
  }
}

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
