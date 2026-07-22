// #9163: the released @openagentsinc/dse train is the single DSE authority.
// Fail when an in-tree duplicate implementation or a workspace fallback
// reappears for dse or graph-corpus.
import { existsSync, globSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join, resolve } from "node:path";

const root = process.argv[2] ?? ".";
const desktopRequire = createRequire(resolve(root, "apps/openagents-desktop/package.json"));
const train = "0.2.1-rc.2";
const failures = [];
const authorityPackages = ["@openagentsinc/dse", "@openagentsinc/graph-corpus"];
const desktopRuntimePackages = [
  "@openagentsinc/agent-harness-contract",
  "@openagentsinc/agent-runtime-schema",
  "@openagentsinc/ai",
  "@openagentsinc/dse",
  "@openagentsinc/graph-corpus",
  "@openagentsinc/history-corpus",
  "@openagentsinc/rlm",
];
const requiredSdkEntrypoints = [
  "@openagentsinc/dse",
  "@openagentsinc/dse/contract",
  "@openagentsinc/dse/optimizer",
  "@openagentsinc/dse/runtime",
  "@openagentsinc/graph-corpus",
  "@openagentsinc/graph-corpus/archive",
  "@openagentsinc/graph-corpus/ranking",
  "@openagentsinc/graph-corpus/rlm",
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
    for (const name of authorityPackages) {
      const spec = section[name];
      if (typeof spec === "string" && spec !== train) {
        failures.push(`${manifest}: ${name} must use exact train ${train}, found ${spec}`);
      }
    }
  }
}

const desktopManifest = JSON.parse(
  readFileSync(join(root, "apps/openagents-desktop/package.json"), "utf8"),
);
for (const name of desktopRuntimePackages) {
  const spec = desktopManifest.dependencies?.[name];
  if (spec !== train) failures.push(`apps/openagents-desktop/package.json: ${name} is ${spec}`);
}
if (desktopManifest.devDependencies?.["@openagentsinc/conformance-kit"] !== train) {
  failures.push(
    `apps/openagents-desktop/package.json: @openagentsinc/conformance-kit must use ${train}`,
  );
}

const lockfile = readFileSync(join(root, "pnpm-lock.yaml"), "utf8");
for (const name of authorityPackages) {
  const escapedName = name.replace("/", "\\/");
  const versions = new Set(
    [...lockfile.matchAll(new RegExp(`${escapedName}@([^':(\\s]+)`, "g"))].map((match) => match[1]),
  );
  if (versions.size !== 1 || !versions.has(train)) {
    failures.push(`pnpm-lock.yaml: ${name} versions are ${[...versions].join(", ") || "absent"}`);
  }
}

for (const entrypoint of requiredSdkEntrypoints) {
  try {
    desktopRequire.resolve(entrypoint);
  } catch {
    failures.push(`installed SDK entry point does not resolve: ${entrypoint}`);
  }
}

if (failures.length > 0) {
  console.error(
    `dse-single-authority-guard FAILED:\n${failures.map((f) => `  - ${f}`).join("\n")}`,
  );
  process.exit(1);
}
console.log(`dse-single-authority-guard OK (${train} is the single DSE/graph-corpus authority)`);
