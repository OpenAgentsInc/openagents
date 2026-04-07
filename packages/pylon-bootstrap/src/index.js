import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import readline from "node:readline/promises";

export const DEFAULT_RELEASE_REPO = "OpenAgentsInc/openagents";
export const DEFAULT_RELEASE_API_BASE = "https://api.github.com";
export const DEFAULT_RELEASE_GIT_BASE = "https://github.com";
export const DEFAULT_RUSTUP_INIT_URL = "https://sh.rustup.rs";
export const DEFAULT_MODEL_ID = "gemma-4-e4b";
export const DEFAULT_DIAGNOSTIC_REPEATS = 3;
export const DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS = 96;
export const DEFAULT_FETCH_TIMEOUT_MS = 15_000;
const PYLON_RELEASE_TAG_PREFIX = "pylon-v";
const RELEASE_ASSET_INSTALL_METHOD = "release_asset";
const SOURCE_BUILD_INSTALL_METHOD = "source_build";

function emitStatus(onStatus, message, detail = null) {
  if (typeof onStatus === "function") {
    onStatus({ message, detail });
  }
}

function emitVerboseStatus(onStatus, verbose, message, detail = null) {
  if (verbose) {
    emitStatus(onStatus, message, detail);
  }
}

function normalizeVersion(value) {
  return value.replace(/^pylon-v/, "").replace(/^v/, "");
}

function createBootstrapError(message, context = {}) {
  const error = new Error(message);
  Object.assign(error, context);
  return error;
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

class MissingReleaseAssetsError extends Error {
  constructor({
    tagName,
    version,
    target,
    archiveBasename,
    archiveName,
    checksumName,
    targetCommitish,
  }) {
    super(`Release ${tagName} is missing ${archiveName} or ${checksumName}.`);
    this.name = "MissingReleaseAssetsError";
    this.tagName = tagName;
    this.version = version;
    this.target = target;
    this.archiveBasename = archiveBasename;
    this.archiveName = archiveName;
    this.checksumName = checksumName;
    this.targetCommitish = targetCommitish ?? null;
  }
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

function formatRequestHeaders(headers = requestHeaders()) {
  return Object.entries(headers).flatMap(([key, value]) => [
    "--header",
    `${key}: ${value}`,
  ]);
}

function timedSignal(timeoutMs = DEFAULT_FETCH_TIMEOUT_MS) {
  if (typeof AbortSignal?.timeout === "function") {
    return {
      signal: AbortSignal.timeout(timeoutMs),
      dispose() {},
    };
  }

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  return {
    signal: controller.signal,
    dispose() {
      clearTimeout(timer);
    },
  };
}

function classifyNetworkError(error) {
  const cause = error?.cause ?? null;
  const code = cause?.code ?? cause?.errno ?? null;
  const detail = [error?.message, cause?.message].filter(Boolean).join(" :: ");
  const lowered = detail.toLowerCase();
  let classification = "network";

  if (
    error?.name === "AbortError" ||
    code === "ETIMEDOUT" ||
    lowered.includes("timeout")
  ) {
    classification = "timeout";
  } else if (
    code === "ENOTFOUND" ||
    code === "EAI_AGAIN" ||
    lowered.includes("getaddrinfo")
  ) {
    classification = "dns";
  } else if (
    code === "CERT_HAS_EXPIRED" ||
    code === "DEPTH_ZERO_SELF_SIGNED_CERT" ||
    lowered.includes("certificate") ||
    lowered.includes("tls") ||
    lowered.includes("ssl")
  ) {
    classification = "tls";
  } else if (
    code === "ECONNREFUSED" ||
    code === "ECONNRESET" ||
    code === "EHOSTUNREACH" ||
    code === "ENETUNREACH" ||
    code === "EADDRNOTAVAIL" ||
    lowered.includes("connect")
  ) {
    classification = "connect";
  }

  const endpoint =
    cause?.address || cause?.hostname || cause?.port
      ? [
          cause?.hostname ?? cause?.address ?? null,
          cause?.port != null ? String(cause.port) : null,
        ]
          .filter(Boolean)
          .join(":")
      : null;

  return {
    classification,
    code,
    endpoint,
    detail: detail || "network request failed",
  };
}

function isRetryableFetchError(error) {
  if (!(error instanceof Error)) {
    return false;
  }
  return classifyNetworkError(error).classification !== "tls";
}

function renderNetworkFailure({ stage, url, error, curlError = null }) {
  const details = classifyNetworkError(error);
  const parts = [
    `${stage} failed for ${url}.`,
    `classification=${details.classification}`,
  ];
  if (details.code) {
    parts.push(`code=${details.code}`);
  }
  if (details.endpoint) {
    parts.push(`endpoint=${details.endpoint}`);
  }
  parts.push(`detail=${details.detail}`);
  if (curlError) {
    parts.push(
      `curl_fallback=${curlError instanceof Error ? curlError.message : String(curlError)}`,
    );
  }
  return parts.join(" ");
}

async function fetchResponse(fetchImpl, url, headers = requestHeaders()) {
  const timeout = timedSignal();
  try {
    return await fetchImpl(url, {
      headers,
      signal: timeout.signal,
    });
  } finally {
    timeout.dispose();
  }
}

async function runCurlText(runProcessImpl, url, headers = requestHeaders()) {
  const { stdout } = await runProcessImpl("curl", [
    "--fail",
    "--silent",
    "--show-error",
    "--location",
    "--connect-timeout",
    "15",
    "--max-time",
    "60",
    ...formatRequestHeaders(headers),
    url,
  ]);
  return stdout;
}

async function runCurlDownload(
  runProcessImpl,
  url,
  destination,
  headers = requestHeaders(),
) {
  await fs.mkdir(path.dirname(destination), { recursive: true });
  await runProcessImpl("curl", [
    "--fail",
    "--silent",
    "--show-error",
    "--location",
    "--connect-timeout",
    "15",
    "--max-time",
    "300",
    "--output",
    destination,
    ...formatRequestHeaders(headers),
    url,
  ]);
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

async function fetchJson(
  fetchImpl,
  url,
  {
    headers = requestHeaders(),
    runProcessImpl = runProcess,
    onStatus = null,
    stage = "GitHub release lookup",
    verbose = false,
  } = {},
) {
  try {
    emitVerboseStatus(onStatus, verbose, stage, url);
    const response = await fetchResponse(fetchImpl, url, headers);
    if (!response.ok) {
      throw createBootstrapError(
        `GitHub release lookup failed for ${url} (${response.status} ${response.statusText}).`,
        {
          stage,
          url,
          httpStatus: response.status,
        },
      );
    }
    return response.json();
  } catch (error) {
    if (
      !error?.httpStatus &&
      isRetryableFetchError(error) &&
      typeof runProcessImpl === "function"
    ) {
      emitStatus(onStatus, "Retrying with curl transport", `${stage} ${url}`);
      try {
        return JSON.parse(await runCurlText(runProcessImpl, url, headers));
      } catch (curlError) {
        throw createBootstrapError(
          renderNetworkFailure({ stage, url, error, curlError }),
          { stage, url, cause: error },
        );
      }
    }
    if (error?.httpStatus) {
      throw error;
    }
    throw createBootstrapError(renderNetworkFailure({ stage, url, error }), {
      stage,
      url,
      cause: error,
    });
  }
}

async function fetchText(
  fetchImpl,
  url,
  {
    headers = requestHeaders(),
    runProcessImpl = runProcess,
    onStatus = null,
    stage = "download text",
    verbose = false,
  } = {},
) {
  try {
    emitVerboseStatus(onStatus, verbose, stage, url);
    const response = await fetchResponse(fetchImpl, url, headers);
    if (!response.ok) {
      throw createBootstrapError(
        `Download failed for ${url} (${response.status} ${response.statusText}).`,
        {
          stage,
          url,
          httpStatus: response.status,
        },
      );
    }
    return response.text();
  } catch (error) {
    if (
      !error?.httpStatus &&
      isRetryableFetchError(error) &&
      typeof runProcessImpl === "function"
    ) {
      emitStatus(onStatus, "Retrying with curl transport", `${stage} ${url}`);
      try {
        return await runCurlText(runProcessImpl, url, headers);
      } catch (curlError) {
        throw createBootstrapError(
          renderNetworkFailure({ stage, url, error, curlError }),
          { stage, url, cause: error },
        );
      }
    }
    if (error?.httpStatus) {
      throw error;
    }
    throw createBootstrapError(renderNetworkFailure({ stage, url, error }), {
      stage,
      url,
      cause: error,
    });
  }
}

async function downloadFile(
  fetchImpl,
  url,
  destination,
  {
    headers = requestHeaders(),
    runProcessImpl = runProcess,
    onStatus = null,
    stage = "download file",
    verbose = false,
  } = {},
) {
  try {
    emitVerboseStatus(onStatus, verbose, stage, url);
    const response = await fetchResponse(fetchImpl, url, headers);
    if (!response.ok) {
      throw createBootstrapError(
        `Download failed for ${url} (${response.status} ${response.statusText}).`,
        {
          stage,
          url,
          httpStatus: response.status,
        },
      );
    }
    const payload = Buffer.from(await response.arrayBuffer());
    await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.writeFile(destination, payload);
  } catch (error) {
    if (
      !error?.httpStatus &&
      isRetryableFetchError(error) &&
      typeof runProcessImpl === "function"
    ) {
      emitStatus(onStatus, "Retrying with curl transport", `${stage} ${url}`);
      try {
        await runCurlDownload(runProcessImpl, url, destination, headers);
        return;
      } catch (curlError) {
        throw createBootstrapError(
          renderNetworkFailure({ stage, url, error, curlError }),
          { stage, url, cause: error },
        );
      }
    }
    if (error?.httpStatus) {
      throw error;
    }
    throw createBootstrapError(renderNetworkFailure({ stage, url, error }), {
      stage,
      url,
      cause: error,
    });
  }
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
  runProcessImpl = runProcess,
  onStatus = null,
  verbose = false,
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
  const payload = await fetchJson(fetchImpl, url, {
    runProcessImpl,
    onStatus,
    verbose,
    stage: normalizedVersion
      ? "GitHub tagged release lookup"
      : "GitHub release list lookup",
  });
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
    throw new MissingReleaseAssetsError({
      tagName,
      version,
      target,
      archiveBasename,
      archiveName,
      checksumName,
      targetCommitish: release?.target_commitish ?? null,
    });
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

async function readInstallManifest(manifestPath) {
  try {
    const payload = await fs.readFile(manifestPath, "utf8");
    return JSON.parse(payload);
  } catch {
    return null;
  }
}

async function writeInstallManifest(manifestPath, payload) {
  await fs.mkdir(path.dirname(manifestPath), { recursive: true });
  await fs.writeFile(manifestPath, `${JSON.stringify(payload, null, 2)}\n`);
}

export function deriveReleaseGitBase(apiBase = DEFAULT_RELEASE_API_BASE) {
  const normalized = (apiBase ?? DEFAULT_RELEASE_API_BASE).replace(/\/$/, "");
  if (normalized === DEFAULT_RELEASE_API_BASE) {
    return DEFAULT_RELEASE_GIT_BASE;
  }
  return normalized.replace(/\/api(?:\/v3)?$/i, "");
}

export function buildReleaseCloneUrl(
  repo,
  {
    apiBase = DEFAULT_RELEASE_API_BASE,
    gitBase = null,
    cloneUrl = null,
  } = {},
) {
  if (cloneUrl) {
    return cloneUrl;
  }
  return `${(gitBase ?? deriveReleaseGitBase(apiBase)).replace(/\/$/, "")}/${repo}.git`;
}

function withPrependedPath(env, entry) {
  const normalizedEntry = path.resolve(entry);
  const parts = (env.PATH ?? process.env.PATH ?? "")
    .split(path.delimiter)
    .filter(Boolean);
  if (!parts.includes(normalizedEntry)) {
    parts.unshift(normalizedEntry);
  }
  return {
    ...env,
    PATH: parts.join(path.delimiter),
  };
}

async function commandExists(command, env = process.env) {
  const pathValue = env.PATH ?? process.env.PATH ?? "";
  const directories = pathValue.split(path.delimiter).filter(Boolean);
  const suffixes =
    process.platform === "win32"
      ? (env.PATHEXT ?? process.env.PATHEXT ?? ".EXE;.CMD;.BAT;.COM")
          .split(";")
          .filter(Boolean)
      : [""];

  for (const directory of directories) {
    for (const suffix of suffixes) {
      const candidate = path.join(directory, `${command}${suffix}`);
      if (await pathExists(candidate)) {
        return true;
      }
    }
  }

  return false;
}

async function promptForApproval(message) {
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new Error(
      `${message}\nInteractive approval is required, but this terminal is not interactive.`,
    );
  }
  const terminal = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });
  try {
    const answer = await terminal.question(`${message} [y/N] `);
    return /^y(?:es)?$/i.test(answer.trim());
  } finally {
    terminal.close();
  }
}

function manualSourceBuildCommands(tagName, cloneUrl) {
  return [
    `git clone --depth 1 --branch ${tagName} ${cloneUrl}`,
    "cd openagents",
    "cargo build --release -p pylon -p pylon-tui",
  ].join("\n");
}

function rustInstallCommand() {
  return "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y";
}

async function ensureRustToolchain({
  target,
  fetchImpl,
  runProcessImpl,
  onStatus,
  promptImpl = promptForApproval,
  commandExistsImpl = commandExists,
  env = process.env,
  rustupInitUrl = DEFAULT_RUSTUP_INIT_URL,
}) {
  let toolchainEnv = withPrependedPath(env, path.join(os.homedir(), ".cargo", "bin"));
  const hasCargo = await commandExistsImpl("cargo", toolchainEnv);
  const hasRustc = await commandExistsImpl("rustc", toolchainEnv);
  if (hasCargo && hasRustc) {
    return toolchainEnv;
  }

  emitStatus(
    onStatus,
    "Rust toolchain required for source build",
    `${target.os}-${target.arch}`,
  );

  const approved = await promptImpl(
    `Rust is required to build Pylon from source for ${target.os}-${target.arch}. Install the official Rust toolchain now via rustup?`,
  );
  if (!approved) {
    throw new Error(
      `Rust is required to build Pylon from source.\nInstall it manually and rerun:\n${rustInstallCommand()}`,
    );
  }

  emitStatus(onStatus, "Installing Rust toolchain", "official rustup installer");
  const scriptPayload = await fetchText(fetchImpl, rustupInitUrl, {
    headers: {
      accept: "text/plain",
      "user-agent": "@openagentsinc/pylon bootstrap",
    },
    runProcessImpl,
    onStatus,
    stage: "Rust toolchain installer download",
  });
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "pylon-rustup-"));
  const scriptPath = path.join(tempDir, "rustup-init.sh");

  try {
    await fs.writeFile(scriptPath, scriptPayload);
    await fs.chmod(scriptPath, 0o755);
    await runProcessImpl("sh", [scriptPath, "-y"], {
      cwd: tempDir,
      env: toolchainEnv,
      stdio: "inherit",
    });
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }

  toolchainEnv = withPrependedPath(env, path.join(os.homedir(), ".cargo", "bin"));
  const cargoInstalled = await commandExistsImpl("cargo", toolchainEnv);
  const rustcInstalled = await commandExistsImpl("rustc", toolchainEnv);
  if (!cargoInstalled || !rustcInstalled) {
    throw new Error(
      `Rust install completed, but \`cargo\` and \`rustc\` were not found on PATH.\nInstall them manually and rerun:\n${rustInstallCommand()}`,
    );
  }

  emitStatus(
    onStatus,
    "Rust toolchain installed",
    path.join(os.homedir(), ".cargo", "bin"),
  );
  return toolchainEnv;
}

async function installSourceBuild(
  {
    selected,
    options,
    paths,
    target,
  },
  {
    fetchImpl,
    runProcessImpl,
    onStatus,
    promptImpl = promptForApproval,
    commandExistsImpl = commandExists,
  },
) {
  const cloneUrl = buildReleaseCloneUrl(options.repo ?? DEFAULT_RELEASE_REPO, {
    apiBase: options.apiBase ?? DEFAULT_RELEASE_API_BASE,
    cloneUrl: options.sourceRepoUrl ?? null,
    gitBase: options.gitBase ?? null,
  });
  const manualBuildInstructions = manualSourceBuildCommands(
    selected.tagName,
    cloneUrl,
  );

  emitStatus(
    onStatus,
    "Prebuilt asset missing; falling back to source build",
    `${selected.tagName} for ${target.os}-${target.arch}`,
  );

  if (!(await commandExistsImpl("git", process.env))) {
    throw new Error(
      `Source build fallback requires \`git\`.\nInstall it and rerun \`npx @openagentsinc/pylon\`, or build manually:\n${manualBuildInstructions}`,
    );
  }

  const buildEnv = await ensureRustToolchain({
    target,
    fetchImpl,
    runProcessImpl,
    onStatus,
    promptImpl,
    commandExistsImpl,
  });
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "pylon-source-build-"));
  const repoDir = path.join(tempDir, "openagents");
  const buildCommand = [
    "cargo",
    "build",
    "--release",
    "-p",
    "pylon",
    "-p",
    "pylon-tui",
  ];

  try {
    await fs.mkdir(repoDir, { recursive: true });
    emitStatus(onStatus, "Fetching source checkout", selected.tagName);
    await runProcessImpl("git", ["init"], {
      cwd: repoDir,
      env: buildEnv,
    });
    await runProcessImpl("git", ["remote", "add", "origin", cloneUrl], {
      cwd: repoDir,
      env: buildEnv,
    });
    await runProcessImpl(
      "git",
      [
        "fetch",
        "--depth",
        "1",
        "origin",
        `refs/tags/${selected.tagName}:refs/tags/${selected.tagName}`,
      ],
      {
        cwd: repoDir,
        env: buildEnv,
      },
    );
    await runProcessImpl(
      "git",
      ["checkout", "--detach", `refs/tags/${selected.tagName}`],
      {
        cwd: repoDir,
        env: buildEnv,
      },
    );

    const { stdout: commitStdout } = await runProcessImpl(
      "git",
      ["rev-parse", "HEAD"],
      {
        cwd: repoDir,
        env: buildEnv,
      },
    );
    const sourceCommit = commitStdout.trim();
    if (
      selected.targetCommitish &&
      /^[a-f0-9]{40}$/i.test(selected.targetCommitish) &&
      sourceCommit !== selected.targetCommitish
    ) {
      throw new Error(
        `Resolved release tag ${selected.tagName} checked out ${sourceCommit}, expected ${selected.targetCommitish}.`,
      );
    }

    emitStatus(
      onStatus,
      "Building Pylon from source",
      `${selected.tagName} (${sourceCommit.slice(0, 12)})`,
    );
    await runProcessImpl(buildCommand[0], buildCommand.slice(1), {
      cwd: repoDir,
      env: buildEnv,
      stdio: "inherit",
    });

    const builtPylonPath = path.join(repoDir, "target", "release", "pylon");
    const builtPylonTuiPath = path.join(repoDir, "target", "release", "pylon-tui");
    if (!(await pathExists(builtPylonPath)) || !(await pathExists(builtPylonTuiPath))) {
      throw new Error(
        `Source build completed without the expected binaries at ${path.join(repoDir, "target", "release")}.`,
      );
    }

    await fs.rm(paths.installDir, { recursive: true, force: true });
    await fs.mkdir(paths.installDir, { recursive: true });
    await Promise.all([
      fs.copyFile(builtPylonPath, paths.pylonPath),
      fs.copyFile(builtPylonTuiPath, paths.pylonTuiPath),
    ]);
    await Promise.allSettled([
      fs.chmod(paths.pylonPath, 0o755),
      fs.chmod(paths.pylonTuiPath, 0o755),
    ]);

    await writeInstallManifest(paths.manifestPath, {
      version: selected.version,
      tagName: selected.tagName,
      target,
      installMethod: SOURCE_BUILD_INSTALL_METHOD,
      sourceCloneUrl: cloneUrl,
      sourceCommit,
      sourceTargetCommitish: selected.targetCommitish ?? null,
      buildCommand: buildCommand.join(" "),
    });

    emitStatus(
      onStatus,
      "Installed source-built binaries",
      `${selected.tagName} for ${target.os}-${target.arch}`,
    );

    return {
      ...selected,
      ...paths,
      target,
      cached: false,
      expectedSha256: null,
      installMethod: SOURCE_BUILD_INSTALL_METHOD,
      sourceCloneUrl: cloneUrl,
      sourceCommit,
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      `${message}\nManual source-build fallback:\n${manualBuildInstructions}`,
    );
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
}

async function findLatestCachedInstall(installRoot, target) {
  const normalizedRoot = path.resolve(installRoot ?? defaultInstallRoot());
  const versionsDir = path.join(normalizedRoot, "versions");
  let entries;
  try {
    entries = await fs.readdir(versionsDir, { withFileTypes: true });
  } catch {
    return null;
  }

  const candidates = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    if (!entry.name.endsWith(`-${target.os}-${target.arch}`)) {
      continue;
    }

    const installDir = path.join(versionsDir, entry.name);
    const manifestPath = path.join(installDir, "install.json");
    const pylonPath = path.join(installDir, "pylon");
    const pylonTuiPath = path.join(installDir, "pylon-tui");
    if (
      !(await pathExists(manifestPath)) ||
      !(await pathExists(pylonPath)) ||
      !(await pathExists(pylonTuiPath))
    ) {
      continue;
    }

    try {
      const manifest = JSON.parse(await fs.readFile(manifestPath, "utf8"));
      const manifestStat = await fs.stat(manifestPath);
      candidates.push({
        version: manifest.version,
        tagName: manifest.tagName,
        target,
        installRoot: normalizedRoot,
        versionsDir,
        downloadsDir: path.join(
          normalizedRoot,
          "downloads",
          `pylon-v${normalizeVersion(manifest.version)}`,
        ),
        installDir,
        archiveBasename: entry.name,
        archivePath: null,
        checksumPath: null,
        manifestPath,
        pylonPath,
        pylonTuiPath,
        expectedSha256: manifest.sha256 ?? null,
        cached: true,
        mtimeMs: manifestStat.mtimeMs,
      });
    } catch {
      // Ignore malformed cache entries and keep scanning.
    }
  }

  candidates.sort((left, right) => right.mtimeMs - left.mtimeMs);
  return candidates[0] ?? null;
}

export async function runProcess(
  command,
  args,
  { cwd, env, stdio = ["ignore", "pipe", "pipe"] } = {},
) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env,
      stdio,
    });
    let stdout = "";
    let stderr = "";
    if (child.stdout) {
      child.stdout.on("data", (chunk) => {
        stdout += chunk.toString();
      });
    }
    if (child.stderr) {
      child.stderr.on("data", (chunk) => {
        stderr += chunk.toString();
      });
    }
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

function isUnsupportedGemmaDiagnoseError(error) {
  const message = error instanceof Error ? error.message : String(error);
  return (
    message.includes("unknown gemma command: diagnose") ||
    message.includes("unknown command: diagnose")
  );
}

export async function ensureReleaseInstall(
  options = {},
  {
    fetchImpl = globalThis.fetch,
    runProcessImpl = runProcess,
    onStatus = null,
    promptImpl = promptForApproval,
    commandExistsImpl = commandExists,
  } = {},
) {
  if (typeof fetchImpl !== "function") {
    throw new Error("A global fetch implementation is required to bootstrap Pylon.");
  }

  emitStatus(
    onStatus,
    "Resolving latest tagged Pylon release",
    options.version ? `requested ${options.version}` : "default release track",
  );
  const target = resolvePlatformTarget(options.platform, options.arch);
  const installRoot = options.installRoot ?? defaultInstallRoot();
  if (options.version) {
    const requestedPaths = buildInstallPaths(installRoot, options.version, target);
    const requestedCached =
      (await pathExists(requestedPaths.pylonPath)) &&
      (await pathExists(requestedPaths.pylonTuiPath));
    if (requestedCached) {
      emitStatus(
        onStatus,
        "Using cached standalone binaries",
        `pylon-v${normalizeVersion(options.version)} for ${target.os}-${target.arch}`,
      );
      return {
        version: normalizeVersion(options.version),
        tagName: `pylon-v${normalizeVersion(options.version)}`,
        target,
        ...requestedPaths,
        expectedSha256: await fs
          .readFile(requestedPaths.manifestPath, "utf8")
          .then((payload) => JSON.parse(payload).sha256)
          .catch(() => null),
        cached: true,
      };
    }
  }

  let release;
  try {
    release = await fetchReleaseMetadata({
      fetchImpl,
      runProcessImpl,
      onStatus,
      verbose: Boolean(options.verbose),
      apiBase: options.apiBase ?? DEFAULT_RELEASE_API_BASE,
      repo: options.repo ?? DEFAULT_RELEASE_REPO,
      version: options.version ?? null,
    });
  } catch (error) {
    const cached = !options.version
      ? await findLatestCachedInstall(installRoot, target)
      : null;
    if (cached) {
      emitStatus(
        onStatus,
        "Using cached standalone binaries",
        `release lookup failed; falling back to ${cached.tagName}`,
      );
      return cached;
    }

    const requestedVersion = normalizeRequestedVersion(options.version);
    const recovery = [
      error instanceof Error ? error.message : String(error),
      `Retry with verbose diagnostics: npx @openagentsinc/pylon --verbose${requestedVersion ? ` --version ${requestedVersion}` : ""}`,
      "Tagged Pylon releases: https://github.com/OpenAgentsInc/openagents/releases?q=pylon-v",
    ];
    if (requestedVersion) {
      recovery.push(
        `Expected asset: ${buildAssetNames(requestedVersion, target).archiveName}`,
      );
    }
    throw createBootstrapError(recovery.join("\n"), { cause: error });
  }
  let selected;
  let missingAssetsError = null;
  try {
    selected = selectReleaseAssets(release, target);
  } catch (error) {
    if (!(error instanceof MissingReleaseAssetsError)) {
      throw error;
    }
    missingAssetsError = error;
    selected = {
      tagName: error.tagName,
      version: error.version,
      archiveBasename: error.archiveBasename,
      targetCommitish: error.targetCommitish,
    };
  }
  const paths = buildInstallPaths(installRoot, selected.version, target);
  const manifest = await readInstallManifest(paths.manifestPath);

  const binariesPresent =
    (await pathExists(paths.pylonPath)) && (await pathExists(paths.pylonTuiPath));
  if (binariesPresent) {
    const installMethod =
      manifest?.installMethod ??
      (missingAssetsError
        ? SOURCE_BUILD_INSTALL_METHOD
        : RELEASE_ASSET_INSTALL_METHOD);
    emitStatus(
      onStatus,
      installMethod === SOURCE_BUILD_INSTALL_METHOD
        ? "Using cached source-built binaries"
        : "Using cached standalone binaries",
      `${selected.tagName} for ${target.os}-${target.arch}`,
    );
    return {
      ...selected,
      ...paths,
      target,
      expectedSha256: manifest?.sha256 ?? null,
      cached: true,
      installMethod,
      sourceCloneUrl: manifest?.sourceCloneUrl ?? null,
      sourceCommit: manifest?.sourceCommit ?? null,
    };
  }

  if (missingAssetsError) {
    return installSourceBuild(
      {
        selected,
        options,
        paths,
        target,
      },
      {
        fetchImpl,
        runProcessImpl,
        onStatus,
        promptImpl,
        commandExistsImpl,
      },
    );
  }

  emitStatus(
    onStatus,
    "Fetching release checksum",
    selected.checksumAsset.name,
  );
  const checksumPayload = await fetchText(fetchImpl, selected.checksumAsset.url, {
    runProcessImpl,
    onStatus,
    verbose: Boolean(options.verbose),
    stage: "Release checksum download",
  });
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
    emitStatus(
      onStatus,
      "Downloading standalone binaries",
      selected.archiveAsset.name,
    );
    await downloadFile(fetchImpl, selected.archiveAsset.url, paths.archivePath, {
      runProcessImpl,
      onStatus,
      verbose: Boolean(options.verbose),
      stage: "Release archive download",
    });
  }

  const actualSha256 = await sha256File(paths.archivePath);
  if (actualSha256 !== expectedSha256) {
    throw new Error(
      `SHA-256 verification failed for ${selected.archiveAsset.name}: expected ${expectedSha256}, got ${actualSha256}.`,
    );
  }

  emitStatus(
    onStatus,
    "Extracting standalone binaries",
    paths.installDir,
  );
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

  await writeInstallManifest(paths.manifestPath, {
    version: selected.version,
    tagName: selected.tagName,
    target,
    archive: selected.archiveAsset.name,
    sha256: expectedSha256,
    installMethod: RELEASE_ASSET_INSTALL_METHOD,
  });

  emitStatus(
    onStatus,
    "Installed standalone binaries",
    `${selected.tagName} for ${target.os}-${target.arch}`,
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
    onStatus = null,
  } = {},
) {
  const pylonPath = path.resolve(options.pylonPath);
  const pylonTuiPath = path.resolve(options.pylonTuiPath);
  const model = options.model ?? DEFAULT_MODEL_ID;
  const diagnosticRepeats =
    options.diagnosticRepeats ?? DEFAULT_DIAGNOSTIC_REPEATS;
  const diagnosticMaxOutputTokens =
    options.diagnosticMaxOutputTokens ?? DEFAULT_DIAGNOSTIC_MAX_OUTPUT_TOKENS;

  emitStatus(onStatus, "Verifying Pylon binary", path.basename(pylonPath));
  await runPylonCommand(pylonPath, ["--help"], options, runProcessImpl);
  emitStatus(onStatus, "Bootstrapping local Pylon identity");
  const init = await runPylonJson(pylonPath, ["init"], options, runProcessImpl);
  emitStatus(onStatus, "Checking runtime health");
  const status = await runPylonJson(
    pylonPath,
    ["status", "--json"],
    options,
    runProcessImpl,
  );
  emitStatus(onStatus, "Scanning for local models");
  const inventory = await runPylonJson(
    pylonPath,
    ["inventory", "--json"],
    options,
    runProcessImpl,
  );

  let download = null;
  if (!options.skipModelDownload) {
    emitStatus(onStatus, "Downloading curated model bundle", model);
    download = await runPylonJson(
      pylonPath,
      ["gemma", "download", model, "--json"],
      options,
      runProcessImpl,
    );
  } else {
    emitStatus(
      onStatus,
      "Skipping optional curated GGUF cache",
      "use --download-curated-cache to prefetch Hugging Face weights",
    );
  }

  let diagnostic = null;
  if (!options.skipDiagnostics) {
    emitStatus(onStatus, "Running first-run diagnostic", model);
    try {
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
    } catch (error) {
      if (!isUnsupportedGemmaDiagnoseError(error)) {
        throw error;
      }
      emitStatus(
        onStatus,
        "Skipping first-run diagnostic",
        "installed Pylon release does not expose gemma diagnose",
      );
    }
  } else {
    emitStatus(onStatus, "Skipping first-run diagnostic", model);
  }

  const diagnosticResult =
    diagnostic?.results?.find((result) => result.model_id === model) ??
    diagnostic?.results?.[0] ??
    null;

  emitStatus(
    onStatus,
    "Bootstrap complete",
    diagnosticResult?.status
      ? `diagnostic ${diagnosticResult.status}`
      : "smoke path complete",
  );

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

export async function launchInstalledPylonTui(
  options,
  {
    runProcessImpl = runProcess,
    onStatus = null,
  } = {},
) {
  const pylonTuiPath = path.resolve(options.pylonTuiPath);
  emitStatus(onStatus, "Opening Pylon terminal UI", path.basename(pylonTuiPath));
  return runProcessImpl(pylonTuiPath, [], {
    env: buildPylonEnv(options),
    stdio: "inherit",
  });
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
  } else {
    lines.push(
      "Curated GGUF cache: skipped by default (pass --download-curated-cache to prefetch optional Hugging Face weights)",
    );
  }

  const localGemmaError =
    summary.status?.snapshot?.availability?.local_gemma?.last_error ?? null;
  if (localGemmaError) {
    lines.push(`Local runtime note: ${localGemmaError}`);
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
