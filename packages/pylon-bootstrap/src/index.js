import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

export const DEFAULT_RELEASE_REPO = "OpenAgentsInc/openagents";
export const DEFAULT_RELEASE_API_BASE = "https://api.github.com";
export const DEFAULT_MODEL_ID = "gemma-4-e4b";
export const DEFAULT_DIAGNOSTIC_REPEATS = 3;
export const DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS = 96;
const PYLON_RELEASE_TAG_PREFIX = "pylon-v";

function normalizeVersion(value) {
  return value.replace(/^pylon-v/, "").replace(/^v/, "");
}

async function pathExists(value) {
  try {
    await fs.access(value);
    return true;
  } catch {
    return false;
  }
}

function defaultInstallRoot() {
  return path.join(os.homedir(), ".openagents", "pylon", "bootstrap");
}

function requestHeaders() {
  const headers = {
    accept: "application/vnd.github+json",
    "user-agent": "@openagentsinc/pylon bootstrap",
  };
  if (process.env.GITHUB_TOKEN) {
    headers.authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  }
  return headers;
}

export function resolvePlatformTarget(
  platform = process.platform,
  arch = process.arch,
) {
  const osLabel =
    {
      darwin: "darwin",
      linux: "linux",
    }[platform] ?? null;
  if (!osLabel) {
    throw new Error(
      `Unsupported platform \`${platform}\`. The npm launcher only supports darwin and linux in v1.`,
    );
  }

  const archLabel =
    {
      arm64: "arm64",
      aarch64: "arm64",
      x64: "x86_64",
      x86_64: "x86_64",
    }[arch] ?? null;
  if (!archLabel) {
    throw new Error(
      `Unsupported architecture \`${arch}\`. The npm launcher only supports arm64 and x64 in v1.`,
    );
  }

  return { os: osLabel, arch: archLabel };
}

export function buildArchiveBasename(version, target) {
  const normalizedVersion = normalizeVersion(version);
  return `pylon-v${normalizedVersion}-${target.os}-${target.arch}`;
}

export function buildAssetNames(version, target) {
  const archiveBasename = buildArchiveBasename(version, target);
  return {
    archiveBasename,
    archiveName: `${archiveBasename}.tar.gz`,
    checksumName: `${archiveBasename}.tar.gz.sha256`,
  };
}

export function parseSha256File(payload, expectedAssetName) {
  const match = payload
    .trim()
    .match(/^([a-fA-F0-9]{64})\s+\*?([^\r\n]+)$/m);
  if (!match) {
    throw new Error("Release checksum file did not contain a valid SHA-256 line.");
  }

  const [, sha256, filename] = match;
  if (
    expectedAssetName &&
    path.basename(filename.trim()) !== path.basename(expectedAssetName)
  ) {
    throw new Error(
      `Release checksum file was for \`${filename.trim()}\`, expected \`${expectedAssetName}\`.`,
    );
  }

  return sha256.toLowerCase();
}

async function fetchJson(fetchImpl, url) {
  const response = await fetchImpl(url, {
    headers: requestHeaders(),
  });
  if (!response.ok) {
    throw new Error(
      `GitHub release lookup failed for ${url} (${response.status} ${response.statusText}).`,
    );
  }
  return response.json();
}

async function fetchText(fetchImpl, url) {
  const response = await fetchImpl(url, {
    headers: requestHeaders(),
  });
  if (!response.ok) {
    throw new Error(
      `Download failed for ${url} (${response.status} ${response.statusText}).`,
    );
  }
  return response.text();
}

async function downloadFile(fetchImpl, url, destination) {
  const response = await fetchImpl(url, {
    headers: requestHeaders(),
  });
  if (!response.ok) {
    throw new Error(
      `Download failed for ${url} (${response.status} ${response.statusText}).`,
    );
  }
  const payload = Buffer.from(await response.arrayBuffer());
  await fs.mkdir(path.dirname(destination), { recursive: true });
  await fs.writeFile(destination, payload);
}

async function sha256File(filePath) {
  const hash = createHash("sha256");
  const stream = createReadStream(filePath);
  return new Promise((resolve, reject) => {
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolve(hash.digest("hex")));
    stream.on("error", reject);
  });
}

export function normalizeRequestedVersion(value) {
  if (!value || value === "latest") {
    return null;
  }
  return normalizeVersion(value);
}

export function isPylonReleaseTag(tagName) {
  return (
    typeof tagName === "string" &&
    tagName.startsWith(PYLON_RELEASE_TAG_PREFIX)
  );
}

export function selectLatestPylonRelease(releases) {
  if (!Array.isArray(releases)) {
    throw new Error("GitHub release lookup did not return a release list.");
  }

  const release = releases.find(
    (candidate) => !candidate?.draft && isPylonReleaseTag(candidate?.tag_name),
  );
  if (!release) {
    throw new Error(
      `GitHub release lookup did not find any published ${PYLON_RELEASE_TAG_PREFIX} releases.`,
    );
  }

  return release;
}

export async function fetchReleaseMetadata({
  fetchImpl = globalThis.fetch,
  apiBase = DEFAULT_RELEASE_API_BASE,
  repo = DEFAULT_RELEASE_REPO,
  version = null,
} = {}) {
  const normalizedVersion = normalizeRequestedVersion(version);
  const endpoint = normalizedVersion
    ? `/repos/${repo}/releases/tags/${encodeURIComponent(
        `${PYLON_RELEASE_TAG_PREFIX}${normalizedVersion}`,
      )}`
    : `/repos/${repo}/releases?per_page=100`;
  const url = `${apiBase.replace(/\/$/, "")}${endpoint}`;
  const payload = await fetchJson(fetchImpl, url);
  return normalizedVersion ? payload : selectLatestPylonRelease(payload);
}

export function selectReleaseAssets(release, target) {
  const tagName = release?.tag_name;
  if (!tagName) {
    throw new Error("GitHub release metadata did not include a tag name.");
  }
  const version = normalizeVersion(tagName);
  const { archiveBasename, archiveName, checksumName } = buildAssetNames(
    version,
    target,
  );
  const assets = Array.isArray(release.assets) ? release.assets : [];
  const archiveAsset = assets.find((asset) => asset.name === archiveName);
  const checksumAsset = assets.find((asset) => asset.name === checksumName);

  if (!archiveAsset || !checksumAsset) {
    throw new Error(
      `Release ${tagName} is missing ${archiveName} or ${checksumName}.`,
    );
  }

  return {
    tagName,
    version,
    archiveBasename,
    archiveAsset: {
      name: archiveAsset.name,
      url: archiveAsset.browser_download_url,
    },
    checksumAsset: {
      name: checksumAsset.name,
      url: checksumAsset.browser_download_url,
    },
  };
}

export function buildInstallPaths(installRoot, version, target) {
  const { archiveBasename, archiveName, checksumName } = buildAssetNames(
    version,
    target,
  );
  const normalizedRoot = path.resolve(installRoot ?? defaultInstallRoot());
  const versionsDir = path.join(normalizedRoot, "versions");
  const downloadsDir = path.join(normalizedRoot, "downloads", `pylon-v${normalizeVersion(version)}`);
  const installDir = path.join(versionsDir, archiveBasename);

  return {
    installRoot: normalizedRoot,
    versionsDir,
    downloadsDir,
    installDir,
    archiveBasename,
    archivePath: path.join(downloadsDir, archiveName),
    checksumPath: path.join(downloadsDir, checksumName),
    manifestPath: path.join(installDir, "install.json"),
    pylonPath: path.join(installDir, "pylon"),
    pylonTuiPath: path.join(installDir, "pylon-tui"),
  };
}

export async function runProcess(
  command,
  args,
  { cwd, env } = {},
) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.on("error", (error) => {
      reject(
        new Error(
          `Failed to start ${command}: ${error instanceof Error ? error.message : String(error)}`,
        ),
      );
    });
    child.on("close", (code) => {
      if (code === 0) {
        resolve({ stdout, stderr });
        return;
      }
      reject(
        new Error(
          `${command} ${args.join(" ")} exited with code ${code}${
            stderr.trim() ? `: ${stderr.trim()}` : ""
          }`,
        ),
      );
    });
  });
}

async function extractArchive(archivePath, destinationDir, runProcessImpl) {
  await fs.mkdir(destinationDir, { recursive: true });
  await runProcessImpl("tar", ["-xzf", archivePath, "-C", destinationDir]);
}

function buildPylonEnv({ configPath, pylonHome } = {}) {
  const env = { ...process.env };
  if (configPath) {
    env.OPENAGENTS_PYLON_CONFIG_PATH = path.resolve(configPath);
  }
  if (pylonHome) {
    env.OPENAGENTS_PYLON_HOME = path.resolve(pylonHome);
  }
  return env;
}

async function runPylonCommand(pylonPath, args, options, runProcessImpl) {
  return runProcessImpl(pylonPath, args, {
    env: buildPylonEnv(options),
  });
}

async function runPylonJson(pylonPath, args, options, runProcessImpl) {
  const { stdout } = await runPylonCommand(
    pylonPath,
    args,
    options,
    runProcessImpl,
  );
  try {
    return JSON.parse(stdout);
  } catch (error) {
    throw new Error(
      `Pylon returned invalid JSON for \`${[path.basename(pylonPath), ...args].join(" ")}\`: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
}

export async function ensureReleaseInstall(
  options = {},
  {
    fetchImpl = globalThis.fetch,
    runProcessImpl = runProcess,
  } = {},
) {
  if (typeof fetchImpl !== "function") {
    throw new Error("A global fetch implementation is required to bootstrap Pylon.");
  }

  const target = resolvePlatformTarget(options.platform, options.arch);
  const installRoot = options.installRoot ?? defaultInstallRoot();
  const release = await fetchReleaseMetadata({
    fetchImpl,
    apiBase: options.apiBase ?? DEFAULT_RELEASE_API_BASE,
    repo: options.repo ?? DEFAULT_RELEASE_REPO,
    version: options.version ?? null,
  });
  const selected = selectReleaseAssets(release, target);
  const paths = buildInstallPaths(installRoot, selected.version, target);

  const binariesPresent =
    (await pathExists(paths.pylonPath)) && (await pathExists(paths.pylonTuiPath));
  if (binariesPresent) {
    return {
      ...selected,
      ...paths,
      target,
      expectedSha256: await fs
        .readFile(paths.manifestPath, "utf8")
        .then((payload) => JSON.parse(payload).sha256)
        .catch(() => null),
      cached: true,
    };
  }

  const checksumPayload = await fetchText(fetchImpl, selected.checksumAsset.url);
  const expectedSha256 = parseSha256File(
    checksumPayload,
    selected.archiveAsset.name,
  );
  await fs.mkdir(paths.downloadsDir, { recursive: true });
  await fs.writeFile(paths.checksumPath, `${checksumPayload.trim()}\n`);

  let archiveReady = false;
  if (await pathExists(paths.archivePath)) {
    archiveReady = (await sha256File(paths.archivePath)) === expectedSha256;
  }
  if (!archiveReady) {
    await downloadFile(fetchImpl, selected.archiveAsset.url, paths.archivePath);
  }

  const actualSha256 = await sha256File(paths.archivePath);
  if (actualSha256 !== expectedSha256) {
    throw new Error(
      `SHA-256 verification failed for ${selected.archiveAsset.name}: expected ${expectedSha256}, got ${actualSha256}.`,
    );
  }

  await fs.rm(paths.installDir, { recursive: true, force: true });
  await extractArchive(paths.archivePath, paths.versionsDir, runProcessImpl);

  if (!(await pathExists(paths.pylonPath)) || !(await pathExists(paths.pylonTuiPath))) {
    throw new Error(
      `Release archive extracted without the expected pylon binaries at ${paths.installDir}.`,
    );
  }

  await Promise.allSettled([
    fs.chmod(paths.pylonPath, 0o755),
    fs.chmod(paths.pylonTuiPath, 0o755),
  ]);

  await fs.writeFile(
    paths.manifestPath,
    `${JSON.stringify(
      {
        version: selected.version,
        tagName: selected.tagName,
        target,
        archive: selected.archiveAsset.name,
        sha256: expectedSha256,
      },
      null,
      2,
    )}\n`,
  );

  return {
    ...selected,
    ...paths,
    target,
    expectedSha256,
    cached: false,
  };
}

export async function bootstrapInstalledPylon(
  options,
  {
    runProcessImpl = runProcess,
  } = {},
) {
  const pylonPath = path.resolve(options.pylonPath);
  const pylonTuiPath = path.resolve(options.pylonTuiPath);
  const model = options.model ?? DEFAULT_MODEL_ID;
  const diagnosticRepeats =
    options.diagnosticRepeats ?? DEFAULT_DIAGNOSTIC_REPEATS;
  const diagnosticMaxOutputTokens =
    options.diagnosticMaxOutputTokens ?? DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS;

  await runPylonCommand(pylonPath, ["--help"], options, runProcessImpl);
  const init = await runPylonJson(pylonPath, ["init"], options, runProcessImpl);
  const status = await runPylonJson(
    pylonPath,
    ["status", "--json"],
    options,
    runProcessImpl,
  );
  const inventory = await runPylonJson(
    pylonPath,
    ["inventory", "--json"],
    options,
    runProcessImpl,
  );

  let download = null;
  if (!options.skipModelDownload) {
    download = await runPylonJson(
      pylonPath,
      ["gemma", "download", model, "--json"],
      options,
      runProcessImpl,
    );
  }

  let diagnostic = null;
  if (!options.skipDiagnostics) {
    diagnostic = await runPylonJson(
      pylonPath,
      [
        "gemma",
        "diagnose",
        model,
        "--max-output-tokens",
        String(diagnosticMaxOutputTokens),
        "--repeats",
        String(diagnosticRepeats),
        "--json",
      ],
      options,
      runProcessImpl,
    );
  }

  const diagnosticResult =
    diagnostic?.results?.find((result) => result.model_id === model) ??
    diagnostic?.results?.[0] ??
    null;

  return {
    version: options.version,
    tagName: options.tagName ?? `pylon-v${options.version}`,
    target: options.target,
    cached: Boolean(options.cached),
    binaries: {
      pylon: pylonPath,
      pylonTui: pylonTuiPath,
    },
    configPath: init?.config_path ?? options.configPath ?? null,
    pylonHome: options.pylonHome ? path.resolve(options.pylonHome) : null,
    init,
    status,
    inventory,
    model,
    download,
    diagnostic,
    diagnosticResult,
  };
}

export function renderBootstrapSummary(summary) {
  const lines = [
    `Pylon release: ${summary.version} (${summary.target.os}-${summary.target.arch})`,
    `Archive source: ${summary.tagName}`,
    `Installed from cache: ${summary.cached ? "yes" : "no"}`,
    `Pylon binary: ${summary.binaries.pylon}`,
    `Pylon TUI: ${summary.binaries.pylonTui}`,
    `Config path: ${summary.configPath ?? "unknown"}`,
  ];

  const statusState =
    summary.status?.snapshot?.runtime?.authoritative_status ?? "unknown";
  const inventoryRows = Array.isArray(summary.inventory?.rows)
    ? summary.inventory.rows.length
    : 0;
  lines.push(`Status state: ${statusState}`);
  lines.push(`Inventory rows: ${inventoryRows}`);

  if (summary.download) {
    const result =
      summary.download?.results?.find((row) => row.model_id === summary.model) ??
      summary.download?.results?.[0] ??
      null;
    lines.push(
      `Model download (${summary.model}): ${result?.status ?? "completed"}`,
    );
  }

  if (summary.diagnostic) {
    const result = summary.diagnosticResult;
    lines.push(
      `Diagnostic (${summary.model}): ${result?.status ?? "unknown"}`,
    );
    if (result?.receipt?.mean_total_s != null) {
      lines.push(
        `Mean total latency: ${result.receipt.mean_total_s.toFixed(3)}s`,
      );
    }
    if (result?.receipt?.mean_ttft_s != null) {
      lines.push(
        `Mean first token latency: ${result.receipt.mean_ttft_s.toFixed(3)}s`,
      );
    }
    if (result?.receipt?.mean_decode_tok_s != null) {
      lines.push(
        `Mean decode throughput: ${result.receipt.mean_decode_tok_s.toFixed(2)} tok/s`,
      );
    }
    if (summary.diagnostic.report_path) {
      lines.push(`Diagnostic report: ${summary.diagnostic.report_path}`);
    }
    if (result?.reason) {
      lines.push(`Diagnostic note: ${result.reason}`);
    }
  }

  return lines.join("\n");
}
