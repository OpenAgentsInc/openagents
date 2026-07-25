import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const packageRoot = resolve(import.meta.dirname, "..");
const repositoryRoot = resolve(packageRoot, "../..");
const matrixPath = resolve(packageRoot, "conformance/matrix.json");
const errors = [];

let matrix;
try {
  matrix = JSON.parse(readFileSync(matrixPath, "utf8"));
} catch {
  errors.push("conformance matrix is not valid JSON");
}

if (matrix !== undefined) {
  const evidenceRecords = [
    ...(Array.isArray(matrix.receipts) ? matrix.receipts : []),
    ...(Array.isArray(matrix.rows)
      ? matrix.rows.flatMap((row) =>
          (Array.isArray(row.evidence) ? row.evidence : []).map((evidence) =>
            Object.assign({}, evidence, { rowId: row.id }),
          ),
        )
      : []),
  ];
  for (const evidence of evidenceRecords) {
    const label = evidence.rowId ?? "suite receipt";
    const path = resolve(repositoryRoot, evidence.path);
    if (!existsSync(path)) {
      errors.push(`${label}: missing evidence ${evidence.path}`);
      continue;
    }
    const digest = createHash("sha256").update(readFileSync(path)).digest("hex");
    if (digest !== evidence.sha256)
      errors.push(`${label}: evidence digest does not match ${evidence.path}`);
  }
}

process.stdout.write(`${JSON.stringify({ valid: errors.length === 0, errors }, null, 2)}\n`);
if (errors.length > 0) process.exitCode = 1;
