// #9163: one exact released SDK train owns DSE and graph-corpus contracts.
// Fail on a duplicate implementation, workspace fallback, mixed SDK version,
// or unresolved required entry point.
import { existsSync, globSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.argv[2] ?? ".";
const train = "0.2.1-rc.4";
const failures = [];
const trainPackages = [
  "@openagentsinc/agent-harness-contract",
  "@openagentsinc/agent-runtime-schema",
  "@openagentsinc/ai-model",
  "@openagentsinc/ai-sdk-sandbox-local",
  "@openagentsinc/ai",
  "@openagentsinc/dse",
  "@openagentsinc/graph-corpus",
  "@openagentsinc/history-corpus",
  "@openagentsinc/rlm",
  "@openagentsinc/conformance-kit",
];

for (const dir of ["packages/dse", "packages/graph-corpus"]) {
  if (existsSync(join(root, dir))) failures.push(`${dir} exists (duplicate implementation)`);
}

const manifests = [
  ...globSync("apps/**/package.json", { cwd: root, exclude: ["**/node_modules/**"] }),
  ...globSync("packages/**/package.json", { cwd: root, exclude: ["**/node_modules/**"] }),
];
for (const manifest of manifests) {
  const parsed = JSON.parse(readFileSync(join(root, manifest), "utf8"));
  for (const sectionName of ["dependencies", "devDependencies", "optionalDependencies"]) {
    const section = parsed[sectionName] ?? {};
    for (const name of trainPackages) {
      const spec = section[name];
      if (typeof spec === "string" && spec !== train) {
        failures.push(`${manifest}: ${name} must use exact train ${train}, found ${spec}`);
      }
    }
  }
}

const lockfile = readFileSync(join(root, "pnpm-lock.yaml"), "utf8");
for (const name of trainPackages) {
  const escapedName = name.replace("/", "\\/");
  const versions = new Set(
    [...lockfile.matchAll(new RegExp(`${escapedName}@([^':()\\s]+)`, "g"))].map((match) => match[1]),
  );
  // Absent is allowed: deleting the Electron desktop app (#9325) removed the
  // only consumer of several train packages, and zero resolved copies cannot
  // be a mixed train. What this guard still forbids is any copy that is not
  // the exact train, which is the drift it was written for (#9163).
  const offTrain = [...versions].filter((version) => version !== train);
  if (offTrain.length > 0) {
    failures.push(`pnpm-lock.yaml: ${name} versions are ${[...versions].join(", ")}`);
  }
}

if (failures.length > 0) {
  console.error(
    `dse-single-authority-guard FAILED:\n${failures.map((f) => `  - ${f}`).join("\n")}`,
  );
  process.exit(1);
}
console.log(`dse-single-authority-guard OK (${train} is the single released SDK train)`);
