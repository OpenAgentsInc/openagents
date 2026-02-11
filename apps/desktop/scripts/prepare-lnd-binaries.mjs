#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const appRoot = path.resolve(scriptDir, "..");
const manifestPath = path.join(appRoot, "lnd", "lnd-artifacts.json");

const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const knownTargets = Object.keys(manifest.targets);

const args = process.argv.slice(2);
const wantsJson = args.includes("--json");
const forceDownload = args.includes("--force");

const readArgValue = (name) => {
  const flagIndex = args.indexOf(name);
  if (flagIndex === -1) return undefined;
  return args[flagIndex + 1];
};

const normalizeValue = (value) => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
};

const resolveTargetFromHost = () => {
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
      throw new Error(`Unsupported host platform for LND artifacts: ${process.platform}/${process.arch}`);
  }
};

const resolveTargets = () => {
  const fromArg = normalizeValue(readArgValue("--targets"));
  const fromEnv = normalizeValue(process.env.OA_LND_TARGETS);
  const raw = fromArg ?? fromEnv;
  if (!raw) return [resolveTargetFromHost()];
  return raw
    .split(",")
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
};

const targets = resolveTargets();
for (const target of targets) {
  if (!knownTargets.includes(target)) {
    throw new Error(`Unsupported target '${target}'. Expected one of: ${knownTargets.join(", ")}`);
  }
}

const cacheRoot = path.join(appRoot, ".cache", "lnd", manifest.version);
const stageRoot = path.join(appRoot, "build-resources", "lnd");

const ensureDir = (dirPath) => {
  fs.mkdirSync(dirPath, { recursive: true });
};

const computeSha256 = (filePath) =>
  crypto.createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");

const assertChecksum = (filePath, expectedSha256, label) => {
  const actualSha256 = computeSha256(filePath);
  if (actualSha256.toLowerCase() !== expectedSha256.toLowerCase()) {
    throw new Error(
      `${label} checksum mismatch for ${filePath}. expected=${expectedSha256} actual=${actualSha256}`,
    );
  }
  return actualSha256;
};

const runCommand = (command, argsList) => {
  const result = spawnSync(command, argsList, { stdio: "pipe" });
  if (result.status !== 0) {
    throw new Error(
      `Command failed: ${command} ${argsList.join(" ")}\nstdout: ${result.stdout.toString()}\nstderr: ${result.stderr.toString()}`,
    );
  }
};

const downloadFile = async (url, outputPath) => {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to download ${url}: ${response.status} ${response.statusText}`);
  }

  const arrayBuffer = await response.arrayBuffer();
  fs.writeFileSync(outputPath, Buffer.from(arrayBuffer));
};

const extractArchive = ({ archivePath, outputDir }) => {
  ensureDir(outputDir);
  if (archivePath.endsWith(".tar.gz")) {
    runCommand("tar", ["-xzf", archivePath, "-C", outputDir]);
    return;
  }

  if (archivePath.endsWith(".zip")) {
    if (process.platform === "win32") {
      runCommand("powershell", [
        "-NoProfile",
        "-Command",
        `Expand-Archive -Path '${archivePath}' -DestinationPath '${outputDir}' -Force`,
      ]);
    } else {
      runCommand("unzip", ["-o", archivePath, "-d", outputDir]);
    }
    return;
  }

  throw new Error(`Unsupported archive format for ${archivePath}`);
};

ensureDir(cacheRoot);
ensureDir(stageRoot);

const runtimeManifest = {
  version: manifest.version,
  generatedAt: new Date().toISOString(),
  targets: {},
};

for (const target of targets) {
  const targetEntry = manifest.targets[target];
  const archiveUrl = `${manifest.releaseBaseUrl}/${targetEntry.archiveFileName}`;
  const archivePath = path.join(cacheRoot, targetEntry.archiveFileName);

  if (!fs.existsSync(archivePath) || forceDownload) {
    await downloadFile(archiveUrl, archivePath);
  }

  assertChecksum(archivePath, targetEntry.archiveSha256, "archive");

  const extractRoot = path.join(cacheRoot, `extract-${target}`);
  fs.rmSync(extractRoot, { recursive: true, force: true });
  ensureDir(extractRoot);
  extractArchive({ archivePath, outputDir: extractRoot });

  const extractedBinaryPath = path.join(extractRoot, ...targetEntry.binaryRelativePath.split("/"));
  if (!fs.existsSync(extractedBinaryPath)) {
    throw new Error(`Extracted binary missing for ${target}: ${extractedBinaryPath}`);
  }

  assertChecksum(extractedBinaryPath, targetEntry.binarySha256, "binary");

  const targetStageDir = path.join(stageRoot, target);
  ensureDir(targetStageDir);

  const stagedBinaryPath = path.join(targetStageDir, targetEntry.binaryFileName);
  fs.copyFileSync(extractedBinaryPath, stagedBinaryPath);
  if (!targetEntry.binaryFileName.endsWith(".exe")) {
    fs.chmodSync(stagedBinaryPath, 0o755);
  }

  const stagedSha256 = assertChecksum(stagedBinaryPath, targetEntry.binarySha256, "staged binary");
  runtimeManifest.targets[target] = {
    binaryFileName: targetEntry.binaryFileName,
    sha256: stagedSha256,
    source: "release",
  };
}

const runtimeManifestPath = path.join(stageRoot, "runtime-manifest.json");
fs.writeFileSync(runtimeManifestPath, `${JSON.stringify(runtimeManifest, null, 2)}\n`, "utf8");

const summary = {
  preparedTargets: targets,
  runtimeManifestPath,
  stageRoot,
};

if (wantsJson) {
  process.stdout.write(`${JSON.stringify(summary)}\n`);
} else {
  process.stdout.write(
    `Prepared LND binaries for targets: ${targets.join(", ")}\nRuntime manifest: ${runtimeManifestPath}\n`,
  );
}
