// #9163: the released @openagentsinc/dse train is the single DSE authority.
// Fail when an in-tree duplicate implementation or a workspace fallback
// reappears for dse or graph-corpus.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.argv[2] ?? ".";
const failures = [];
for (const dir of ["packages/dse", "packages/graph-corpus"]) {
  if (existsSync(join(root, dir))) failures.push(`${dir} exists (duplicate implementation)`);
}
for (const manifest of ["apps/openagents-desktop/package.json"]) {
  const parsed = JSON.parse(readFileSync(join(root, manifest), "utf8"));
  for (const section of [parsed.dependencies ?? {}, parsed.devDependencies ?? {}]) {
    for (const name of ["@openagentsinc/dse", "@openagentsinc/graph-corpus"]) {
      const spec = section[name];
      if (typeof spec === "string" && spec.startsWith("workspace:")) {
        failures.push(`${manifest}: ${name} uses a workspace fallback (${spec})`);
      }
    }
  }
}
if (failures.length > 0) {
  console.error(
    `dse-single-authority-guard FAILED:\n${failures.map((f) => `  - ${f}`).join("\n")}`,
  );
  process.exit(1);
}
console.log(
  "dse-single-authority-guard OK (released train is the single DSE/graph-corpus authority)",
);
