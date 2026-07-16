import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { validateAcpReleaseMatrix } from "../src/release.ts";

const path = resolve(import.meta.dirname, "../compatibility/release-matrix.json");
const matrix = JSON.parse(readFileSync(path, "utf8")) as unknown;
const validation = validateAcpReleaseMatrix(matrix);
process.stdout.write(`${JSON.stringify(validation, null, 2)}\n`);
if (!validation.valid) process.exitCode = 1;
