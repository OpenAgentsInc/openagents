#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptDir, "..");

const args = process.argv.slice(2);
const wantsJson = args.includes("--json");
const requirePackaged = args.includes("--require-packaged");

const readArgValue = (name) => {
  const index = args.indexOf(name);
  if (index === -1) return undefined;
  return args[index + 1];
};

const normalizeValue = (value) => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
};

const computeSha256 = (filePath) =>
  crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");

const runCommand = (command, argsList) => {
  const result = spawnSync(command, argsList, { cwd: appRoot, stdio: "pipe" });
  if (result.status !== 0) {
    throw new Error(
      `Command failed: ${command} ${argsList.join(" ")}\nstdout: ${result.stdout.toString()}\nstderr: ${result.stderr.toString()}`,
    );
  }
};

const resolveHostTarget = () => {
  switch (`${process.platform}:${process.arch}`) {
    case "darwin:arm64":
      return "darwin-arm64";
    case "darwin:x64":
      return "darwin-amd64";
    case "linux:x64":
      return "linux-amd64";
    case "linux:arm64":
      return "linux-arm64";
    case "win32:x64":
      return "windows-amd64";
    default:
      throw new Error(`Unsupported host platform for LND smoke: ${process.platform}/${process.arch}`);
  }
};

const resolveTargets = () => {
  const fromArg = normalizeValue(readArgValue("--targets"));
  const fromEnv = normalizeValue(process.env.OA_LND_TARGETS);
  const raw = fromArg ?? fromEnv;
  if (!raw) return [resolveHostTarget()];
  return raw
    .split(",")
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
};

const gatherRuntimeManifestPaths = (rootDir) => {
  const found = [];

  const visit = (currentDir) => {
    const entries = fs.readdirSync(currentDir, { withFileTypes: true });
    for (const entry of entries) {
      const fullPath = path.join(currentDir, entry.name);
      if (entry.isDirectory()) {
        visit(fullPath);
      } else if (entry.isFile() && entry.name === "runtime-manifest.json") {
        const normalized = fullPath.replace(/\\/g, "/");
        const normalizedLower = normalized.toLowerCase();
        if (
          normalizedLower.includes("/resources/lnd/runtime-manifest.json") ||
          normalizedLower.endsWith("/build-resources/lnd/runtime-manifest.json")
        ) {
          found.push(fullPath);
        }
      }
    }
  };

  if (fs.existsSync(rootDir)) {
    visit(rootDir);
  }
  return found;
};

const verifyRoot = (resourceRoot, targets) => {
  const runtimeManifestPath = path.join(resourceRoot, "runtime-manifest.json");
  if (!fs.existsSync(runtimeManifestPath)) {
    throw new Error(`Missing runtime manifest at ${runtimeManifestPath}`);
  }

  const runtimeManifest = JSON.parse(fs.readFileSync(runtimeManifestPath, "utf8"));
  const checks = [];

  for (const target of targets) {
    const targetManifest = runtimeManifest.targets?.[target];
    if (!targetManifest || typeof targetManifest.binaryFileName !== "string") {
      throw new Error(`Runtime manifest missing target entry for ${target} in ${runtimeManifestPath}`);
    }
    if (typeof targetManifest.sha256 !== "string") {
      throw new Error(`Runtime manifest missing checksum for ${target} in ${runtimeManifestPath}`);
    }

    const binaryPath = path.join(resourceRoot, target, targetManifest.binaryFileName);
    if (!fs.existsSync(binaryPath)) {
      throw new Error(`Missing binary for target ${target} at ${binaryPath}`);
    }

    const actualSha = computeSha256(binaryPath);
    if (actualSha.toLowerCase() !== targetManifest.sha256.toLowerCase()) {
      throw new Error(
        `Checksum mismatch for target ${target} at ${binaryPath}. expected=${targetManifest.sha256} actual=${actualSha}`,
      );
    }

    checks.push({
      target,
      binaryPath,
      sha256: actualSha,
      source: targetManifest.source,
    });
  }

  return {
    resourceRoot,
    runtimeManifestPath,
    checks,
  };
};

const targets = resolveTargets();
const stageRoot = normalizeValue(readArgValue("--stage-root"))
  ? path.resolve(readArgValue("--stage-root"))
  : path.join(appRoot, "build-resources", "lnd");
const outRoot = normalizeValue(readArgValue("--out-root"))
  ? path.resolve(readArgValue("--out-root"))
  : path.join(appRoot, "out");

const stageManifestPath = path.join(stageRoot, "runtime-manifest.json");
if (!fs.existsSync(stageManifestPath)) {
  const argsForPrepare = ["./scripts/prepare-lnd-binaries.mjs", "--targets", targets.join(",")];
  runCommand("node", argsForPrepare);
}

const stagedResult = verifyRoot(stageRoot, targets);

const packagedManifestPaths = gatherRuntimeManifestPaths(outRoot)
  .map((manifestPath) => path.dirname(manifestPath))
  .filter((resourceRoot) => resourceRoot !== stageRoot);

if (requirePackaged && packagedManifestPaths.length === 0) {
  throw new Error(`No packaged lnd runtime manifests found under ${outRoot}`);
}

const packagedResults = packagedManifestPaths.map((resourceRoot) => verifyRoot(resourceRoot, targets));

const summary = {
  targets,
  stagedResult,
  packagedCount: packagedResults.length,
  packagedResults,
};

if (wantsJson) {
  process.stdout.write(`${JSON.stringify(summary)}\n`);
} else {
  process.stdout.write(
    `LND binary smoke OK for targets: ${targets.join(", ")} (packaged manifests: ${packagedResults.length})\n`,
  );
}
