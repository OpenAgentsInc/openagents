import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

import { evaluateAcpReleaseMatrixGate, validateAcpReleaseMatrix } from "../src/release.ts";
import {
  validateAcpDesktopReleaseArtifact,
  validateAcpLiveReleaseArtifact,
  type AcpDesktopReleaseArtifact,
  type AcpLiveReleaseArtifact,
} from "../src/live-release.ts";

const path = resolve(import.meta.dirname, "../compatibility/release-matrix.json");
const matrix = JSON.parse(readFileSync(path, "utf8")) as unknown;
const validation = validateAcpReleaseMatrix(matrix);
const requireFreshness = process.argv.includes("--require-freshness");
const gate = evaluateAcpReleaseMatrixGate(validation, requireFreshness ? "release" : "push");
const liveDirectory = resolve(import.meta.dirname, "../compatibility/live");
const liveArtifactErrors = readdirSync(liveDirectory)
  .filter((name) => name.startsWith("release-run-") && name.endsWith(".json"))
  .flatMap((name) => {
    try {
      const artifact = JSON.parse(readFileSync(resolve(liveDirectory, name), "utf8")) as AcpLiveReleaseArtifact;
      return validateAcpLiveReleaseArtifact(artifact).errors.map((error) => `${name}: ${error}`);
    } catch {
      return [`${name}: artifact is not valid JSON or does not match the closed schema`];
    }
  });
const desktopArtifactErrors = readdirSync(liveDirectory)
  .filter((name) => name.startsWith("desktop-") && name.includes("-release-run-") && name.endsWith(".json"))
  .flatMap((name) => {
    try {
      const artifact = JSON.parse(readFileSync(resolve(liveDirectory, name), "utf8")) as AcpDesktopReleaseArtifact;
      return validateAcpDesktopReleaseArtifact(artifact).errors.map(
        (error) => `${name}: ${error}`,
      );
    } catch {
      return [`${name}: artifact is not valid JSON or does not match the closed schema`];
    }
  });
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
  valid:
    gate.valid &&
    missingEvidence.length === 0 &&
    liveArtifactErrors.length === 0 &&
    desktopArtifactErrors.length === 0,
  errors: [
    ...gate.errors,
    ...missingEvidence.map((ref) => `missing evidence ref: ${ref}`),
    ...liveArtifactErrors,
    ...desktopArtifactErrors,
  ],
  ...(validation.expiry === undefined ? {} : { expiry: validation.expiry }),
};
if (!requireFreshness && gate.warning !== undefined)
  process.stderr.write(`WARNING: ${gate.warning}\n`);
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
if (!result.valid) process.exitCode = 1;
