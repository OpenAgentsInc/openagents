import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import {
  buildCompatibilityMatrix,
  buildCoverageReport,
  buildFaultMatrix,
} from "../src/artifacts.ts";

const outputs: ReadonlyArray<readonly [string, unknown]> = [
  ["coverage.json", buildCoverageReport()],
  ["compatibility-matrix.json", buildCompatibilityMatrix()],
  ["fault-matrix.json", buildFaultMatrix()],
];
const check = process.argv.includes("--check");
let drift = false;
for (const [name, value] of outputs) {
  const path = resolve(import.meta.dirname, "../compatibility", name);
  const encoded = `${JSON.stringify(value, null, 2)}\n`;
  if (check) {
    const current = JSON.parse(readFileSync(path, "utf8")) as unknown;
    if (JSON.stringify(current) !== JSON.stringify(value)) {
      process.stderr.write(
        `${name} is stale; run pnpm --dir packages/agent-client-protocol-conformance generate\n`,
      );
      drift = true;
    }
  } else writeFileSync(path, encoded);
}
if (drift) process.exitCode = 1;
