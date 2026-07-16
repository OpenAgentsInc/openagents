import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import { validateAcpReleaseMatrix } from "../src/release.ts";

const path = resolve(import.meta.dirname, "../compatibility/release-matrix.json");
const matrix = JSON.parse(readFileSync(path, "utf8")) as unknown;
const validation = validateAcpReleaseMatrix(matrix);
const repositoryRoot = resolve(import.meta.dirname, "../../..");
const missingEvidence =
  typeof matrix === "object" && matrix !== null && "peers" in matrix && Array.isArray(matrix.peers)
    ? matrix.peers.flatMap((peer: unknown) =>
        typeof peer === "object" && peer !== null && "scenarios" in peer && Array.isArray(peer.scenarios)
          ? peer.scenarios.flatMap((scenario: unknown) =>
              typeof scenario === "object" &&
              scenario !== null &&
              "evidenceRefs" in scenario &&
              Array.isArray(scenario.evidenceRefs)
                ? scenario.evidenceRefs.filter(
                    (ref: unknown): ref is string =>
                      typeof ref === "string" && !existsSync(resolve(repositoryRoot, ref)),
                  )
                : [],
            )
          : [],
      )
    : [];
const result = {
  valid: validation.valid && missingEvidence.length === 0,
  errors: [
    ...validation.errors,
    ...missingEvidence.map((ref) => `missing evidence ref: ${ref}`),
  ],
};
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
if (!result.valid) process.exitCode = 1;
