import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

import artifactsManifestJson from "../../lnd/lnd-artifacts.json";

export type LndBinaryTarget =
  | "darwin-amd64"
  | "darwin-arm64"
  | "linux-amd64"
  | "linux-arm64"
  | "windows-amd64";

export type LndBinarySource = "bundled" | "dev_override";

type LndArtifactTargetEntry = Readonly<{
  readonly archiveFileName: string;
  readonly archiveSha256: string;
  readonly binaryRelativePath: string;
  readonly binaryFileName: string;
  readonly binarySha256: string;
}>;

type LndArtifactsManifest = Readonly<{
  readonly version: string;
  readonly releaseBaseUrl: string;
  readonly targets: Readonly<Record<LndBinaryTarget, LndArtifactTargetEntry>>;
}>;

type LndRuntimeManifestEntry = Readonly<{
  readonly binaryFileName: string;
  readonly sha256: string;
  readonly source: "release" | "local_dev";
}>;

type LndRuntimeManifest = Readonly<{
  readonly version: string;
  readonly generatedAt: string;
  readonly targets: Readonly<Record<LndBinaryTarget, LndRuntimeManifestEntry>>;
}>;

export type LndBinaryResolution = Readonly<{
  readonly target: LndBinaryTarget;
  readonly binaryPath: string;
  readonly binaryFileName: string;
  readonly resourceRoot: string;
  readonly source: LndBinarySource;
}>;

export type LndResolvedBinary = LndBinaryResolution &
  Readonly<{
    readonly sha256: string;
  }>;

export type LndBinaryResolverErrorCode =
  | "unsupported_platform"
  | "invalid_target"
  | "binary_not_found"
  | "runtime_manifest_missing"
  | "runtime_manifest_invalid"
  | "checksum_mismatch";

export class LndBinaryResolverError extends Error {
  readonly code: LndBinaryResolverErrorCode;

  constructor(code: LndBinaryResolverErrorCode, message: string) {
    super(message);
    this.name = "LndBinaryResolverError";
    this.code = code;
  }
}

type ResolveLndBinaryPathOptions = Readonly<{
  readonly appPath: string;
  readonly resourcesPath: string;
  readonly isPackaged: boolean;
  readonly platform?: NodeJS.Platform;
  readonly arch?: string;
  readonly env?: NodeJS.ProcessEnv;
}>;

const artifactsManifest = artifactsManifestJson as LndArtifactsManifest;

const knownTargets: ReadonlySet<LndBinaryTarget> = new Set<LndBinaryTarget>([
  "darwin-amd64",
  "darwin-arm64",
  "linux-amd64",
  "linux-arm64",
  "windows-amd64",
]);

const toHashHex = (buffer: Buffer): string =>
  crypto.createHash("sha256").update(buffer).digest("hex");

const normalizeEnvValue = (value: string | undefined): string | undefined => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
};

const parseTarget = (candidate: string | undefined): LndBinaryTarget | undefined => {
  const normalized = normalizeEnvValue(candidate);
  if (!normalized) return undefined;
  if (!knownTargets.has(normalized as LndBinaryTarget)) {
    throw new LndBinaryResolverError(
      "invalid_target",
      `Unsupported OA_DESKTOP_LND_TARGET value: ${normalized}`,
    );
  }
  return normalized as LndBinaryTarget;
};

export const resolveLndBinaryTarget = (platform: NodeJS.Platform, arch: string): LndBinaryTarget => {
  switch (`${platform}:${arch}`) {
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
      throw new LndBinaryResolverError(
        "unsupported_platform",
        `No LND artifact target configured for platform=${platform} arch=${arch}`,
      );
  }
};

const resolveResourceRoot = (options: ResolveLndBinaryPathOptions): string =>
  options.isPackaged
    ? path.join(options.resourcesPath, "lnd")
    : path.join(options.appPath, "build-resources", "lnd");

const loadRuntimeManifest = (resourceRoot: string): LndRuntimeManifest => {
  const runtimeManifestPath = path.join(resourceRoot, "runtime-manifest.json");
  if (!fs.existsSync(runtimeManifestPath)) {
    throw new LndBinaryResolverError(
      "runtime_manifest_missing",
      `Missing runtime manifest: ${runtimeManifestPath}`,
    );
  }

  try {
    const parsed = JSON.parse(fs.readFileSync(runtimeManifestPath, "utf8")) as Partial<LndRuntimeManifest>;
    if (!parsed || typeof parsed !== "object" || typeof parsed.version !== "string") {
      throw new Error("invalid root shape");
    }
    if (!parsed.targets || typeof parsed.targets !== "object") {
      throw new Error("missing targets map");
    }
    return parsed as LndRuntimeManifest;
  } catch (error) {
    if (error instanceof LndBinaryResolverError) throw error;
    throw new LndBinaryResolverError(
      "runtime_manifest_invalid",
      `Invalid runtime manifest at ${runtimeManifestPath}: ${String(error)}`,
    );
  }
};

export const resolveLndBinaryPath = (options: ResolveLndBinaryPathOptions): LndBinaryResolution => {
  const platform = options.platform ?? process.platform;
  const arch = options.arch ?? process.arch;
  const env = options.env ?? process.env;

  const devOverridePath = normalizeEnvValue(env.OA_DESKTOP_LND_DEV_BINARY_PATH);
  if (!options.isPackaged && devOverridePath) {
    if (!fs.existsSync(devOverridePath)) {
      throw new LndBinaryResolverError(
        "binary_not_found",
        `OA_DESKTOP_LND_DEV_BINARY_PATH does not exist: ${devOverridePath}`,
      );
    }

    const target = parseTarget(env.OA_DESKTOP_LND_TARGET) ?? resolveLndBinaryTarget(platform, arch);
    return {
      target,
      binaryPath: path.resolve(devOverridePath),
      binaryFileName: path.basename(devOverridePath),
      resourceRoot: path.dirname(path.resolve(devOverridePath)),
      source: "dev_override",
    };
  }

  const target = parseTarget(env.OA_DESKTOP_LND_TARGET) ?? resolveLndBinaryTarget(platform, arch);
  const targetEntry = artifactsManifest.targets[target];
  if (!targetEntry) {
    throw new LndBinaryResolverError(
      "invalid_target",
      `LND artifacts manifest does not include target ${target}`,
    );
  }

  const resourceRoot = resolveResourceRoot(options);
  const binaryPath = path.join(resourceRoot, target, targetEntry.binaryFileName);

  if (!fs.existsSync(binaryPath)) {
    throw new LndBinaryResolverError("binary_not_found", `Missing LND binary at ${binaryPath}`);
  }

  return {
    target,
    binaryPath,
    binaryFileName: targetEntry.binaryFileName,
    resourceRoot,
    source: "bundled",
  };
};

export const verifyLndBinaryIntegrity = (
  binaryPath: string,
  expectedSha256: string,
): Readonly<{
  readonly sha256: string;
  readonly valid: boolean;
}> => {
  const buffer = fs.readFileSync(binaryPath);
  const sha256 = toHashHex(buffer);
  const valid = sha256.toLowerCase() === expectedSha256.toLowerCase();
  return { sha256, valid };
};

export const resolveAndVerifyLndBinary = (options: ResolveLndBinaryPathOptions): LndResolvedBinary => {
  const resolution = resolveLndBinaryPath(options);

  if (resolution.source === "dev_override") {
    const expectedSha256 = normalizeEnvValue((options.env ?? process.env).OA_DESKTOP_LND_DEV_BINARY_SHA256);
    if (expectedSha256) {
      const checked = verifyLndBinaryIntegrity(resolution.binaryPath, expectedSha256);
      if (!checked.valid) {
        throw new LndBinaryResolverError(
          "checksum_mismatch",
          `LND dev override checksum mismatch for ${resolution.binaryPath}; expected=${expectedSha256} actual=${checked.sha256}`,
        );
      }
      return { ...resolution, sha256: checked.sha256 };
    }

    const sha256 = toHashHex(fs.readFileSync(resolution.binaryPath));
    return { ...resolution, sha256 };
  }

  const runtimeManifest = loadRuntimeManifest(resolution.resourceRoot);
  const runtimeTarget = runtimeManifest.targets[resolution.target];
  if (!runtimeTarget || typeof runtimeTarget.sha256 !== "string") {
    throw new LndBinaryResolverError(
      "runtime_manifest_invalid",
      `Runtime manifest missing checksum for target ${resolution.target}`,
    );
  }

  const checked = verifyLndBinaryIntegrity(resolution.binaryPath, runtimeTarget.sha256);
  if (!checked.valid) {
    throw new LndBinaryResolverError(
      "checksum_mismatch",
      `LND binary checksum mismatch for ${resolution.binaryPath}; expected=${runtimeTarget.sha256} actual=${checked.sha256}`,
    );
  }

  return { ...resolution, sha256: checked.sha256 };
};
