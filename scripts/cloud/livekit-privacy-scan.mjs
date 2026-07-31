#!/usr/bin/env node

import { lstatSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  PRIVACY_SCOPES,
  scanSarahLiveKitPrivacy,
  unavailableSarahLiveKitPrivacyResult,
} from "./livekit-privacy-scan-lib.mjs";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-privacy-scan.mjs \\
    --source-base-revision <40-hex-git-revision> \\
    --openai-key-file <private-file> \\
    --sarah-private-key-file <private-file> \\
    --retention-canary-file <private-file> [--retention-canary-file <private-file> ...] \\
    --scope packaged_omega=<path> --scope packaged_clients=<path> \\
    --scope pods=<path> --scope logs=<path> --scope redis=<path> \\
    --scope object_storage=<path> --scope traces=<path> \\
    --scope crash_artifacts=<path> \\
    --output <private-observation.json> --apply

Each scope path must be a complete read-only export captured for the same
bounded acceptance window. Missing, empty, unreadable, oversized, symlinked,
or special-file inputs fail closed. Output contains counts and SHA-256
evidence digests only; matched values and object names are never emitted.
`);
};

const parseArgs = (args) => {
  const parsed = {
    apply: false,
    canaryFiles: [],
    openAiKeyFile: undefined,
    output: undefined,
    sarahPrivateKeyFile: undefined,
    scopeInputs: {},
    sourceBaseRevision: undefined,
  };
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    const value = args[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    if (argument === "--scope") {
      const separator = value.indexOf("=");
      if (separator <= 0) throw new Error("--scope must use scope=path");
      const scope = value.slice(0, separator);
      const path = value.slice(separator + 1);
      if (!PRIVACY_SCOPES.includes(scope) || !path || parsed.scopeInputs[scope]) {
        throw new Error("--scope is unsupported, empty, or duplicated");
      }
      parsed.scopeInputs[scope] = path;
    } else if (argument === "--retention-canary-file") {
      parsed.canaryFiles.push(value);
    } else if (argument === "--openai-key-file") {
      parsed.openAiKeyFile = value;
    } else if (argument === "--sarah-private-key-file") {
      parsed.sarahPrivateKeyFile = value;
    } else if (argument === "--source-base-revision") {
      parsed.sourceBaseRevision = value;
    } else if (argument === "--output") {
      parsed.output = value;
    } else {
      throw new Error(`unsupported argument ${argument}`);
    }
    index += 1;
  }
  if (!parsed.apply) throw new Error("privacy evidence collection requires --apply");
  if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`);
  }
  for (const key of ["openAiKeyFile", "output", "sarahPrivateKeyFile", "sourceBaseRevision"]) {
    if (!parsed[key]) throw new Error(`required privacy scan argument is missing: ${key}`);
  }
  if (parsed.canaryFiles.length === 0) {
    throw new Error("at least one --retention-canary-file is required");
  }
  return parsed;
};

const privateBytes = (path) => {
  const resolvedPath = resolve(path);
  const input = lstatSync(resolvedPath);
  if (!input.isFile() || input.isSymbolicLink()) {
    throw new Error("privacy scan secret inputs must be regular files");
  }
  if ((input.mode & 0o077) !== 0) {
    throw new Error("privacy scan secret inputs must have mode 0600 or stricter");
  }
  const bytes = readFileSync(resolvedPath);
  let end = bytes.length;
  while (end > 0 && [0x0a, 0x0d].includes(bytes[end - 1])) end -= 1;
  return bytes.subarray(0, end);
};

const writeExclusive = (path, value) => {
  writeFileSync(resolve(path), `${JSON.stringify(value, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
};

let parsed;
try {
  parsed = parseArgs(process.argv.slice(2));
  const result = scanSarahLiveKitPrivacy({
    scopeInputs: parsed.scopeInputs,
    openAiKey: privateBytes(parsed.openAiKeyFile),
    sarahPrivateKey: privateBytes(parsed.sarahPrivateKeyFile),
    canaries: parsed.canaryFiles.map(privateBytes),
    sourceBaseRevision: parsed.sourceBaseRevision,
  });
  writeExclusive(parsed.output, result);
  process.stdout.write(
    `${JSON.stringify({
      outcome: result.outcome,
      output: resolve(parsed.output),
      scopeCount: result.results.scopes.length,
      forbiddenPatternCount: result.results.forbiddenPatternCount,
      findings: result.results.findings,
      rawMediaObjects: result.results.rawMediaObjects,
      transcriptObjects: result.results.transcriptObjects,
    })}\n`,
  );
  if (result.outcome !== "passed") process.exitCode = 1;
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  if (parsed?.output && parsed?.sourceBaseRevision) {
    try {
      writeExclusive(
        parsed.output,
        unavailableSarahLiveKitPrivacyResult({
          sourceBaseRevision: parsed.sourceBaseRevision,
          unavailableScope: PRIVACY_SCOPES.find((scope) => !parsed.scopeInputs[scope]),
        }),
      );
    } catch {
      // The original error remains authoritative; never overwrite an existing receipt.
    }
  }
  process.stderr.write(`${message}\n`);
  usage();
  process.exitCode = 1;
}
