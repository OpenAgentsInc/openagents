import { createHash } from "node:crypto";

import { checkpointPathAdmitted, type Sha256Digest } from "./cloud-computer-checkpoint.js";

export const CLOUD_COMPUTER_WORKSPACE_RESTORE_RECEIPT_SCHEMA =
  "openagents.cloud_computer.workspace_restore_receipt.v1" as const;
export const CLOUD_COMPUTER_WORKSPACE_CHECKPOINT_RECEIPT_SCHEMA =
  "openagents.cloud_computer.workspace_checkpoint_receipt.v1" as const;

declare const contentRefBrand: unique symbol;
export type CloudComputerWorkspaceContentRef = string & {
  readonly [contentRefBrand]: "CloudComputerWorkspaceContentRef";
};

/** Converts a durable logical ref to an opaque handle. Storage URLs and paths are forbidden. */
export const cloudComputerWorkspaceContentRef = (
  value: string,
): CloudComputerWorkspaceContentRef => {
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,255}$/u.test(value)) {
    throw new CloudComputerWorkspaceError("invalid_input", "content ref must be opaque");
  }
  return value as CloudComputerWorkspaceContentRef;
};

export type CloudComputerWorkspaceEntry = Readonly<{
  path: string;
  kind: "directory" | "file" | "symlink" | "socket" | "fifo" | "device";
  classification: "workspace" | "git_metadata" | "secret" | "runtime_metadata";
  admitted: boolean;
  byteCount: number;
  digest: Sha256Digest | null;
  linkTarget?: string | undefined;
}>;

export type CloudComputerWorkspaceCheckpoint = Readonly<{
  checkpointRef: string;
  workspaceRef: string;
  ownerRef: string;
  tenantRef: string;
  generation: number;
  kind: "full" | "delta";
  baseImageDigest: Sha256Digest;
  parentCheckpointRef: string | null;
  contentRef: CloudComputerWorkspaceContentRef;
  contentDigest: Sha256Digest;
  byteCount: number;
  entries: ReadonlyArray<CloudComputerWorkspaceEntry>;
  deletedPaths: ReadonlyArray<string>;
}>;

export type CloudComputerSignedBaseImage = Readonly<{
  imageRef: string;
  digest: Sha256Digest;
  signatureRef: string;
  signerIdentity: string;
}>;

export type CloudComputerWorkspaceCacheRef = Readonly<{
  cacheRef: string;
  kind: "base_image" | "toolchain" | "dependency";
  digest: Sha256Digest;
}>;

export type CloudComputerWorkspaceAllocation = Readonly<{
  allocationRef: string;
  rootPath: string;
  lowerPath: string;
  upperPath: string;
  workPath: string;
  mergedPath: string;
}>;

export type CloudComputerWorkspaceBenchmark = Readonly<{
  dependencyInstallDurationMs: number;
  largeFileWriteDurationMs: number;
  largeFileBytes: number;
}>;

export interface CloudComputerWorkspaceRunner {
  readonly allocate: (input: {
    readonly workspaceRef: string;
    readonly generation: number;
  }) => Promise<CloudComputerWorkspaceAllocation>;
  readonly verifySignedBaseImage: (input: CloudComputerSignedBaseImage) => Promise<{
    readonly verifiedDigest: Sha256Digest;
    readonly signerIdentity: string;
  }>;
  readonly mountBaseImage: (input: {
    readonly allocation: CloudComputerWorkspaceAllocation;
    readonly image: CloudComputerSignedBaseImage;
    readonly readOnly: true;
  }) => Promise<void>;
  readonly mountOverlay: (input: {
    readonly allocation: CloudComputerWorkspaceAllocation;
    readonly lowerReadOnly: true;
  }) => Promise<void>;
  readonly restoreEntries: (input: {
    readonly allocation: CloudComputerWorkspaceAllocation;
    readonly contentRef: CloudComputerWorkspaceContentRef;
    readonly expectedContentDigest: Sha256Digest;
    readonly kind: "full" | "delta";
    readonly entries: ReadonlyArray<CloudComputerWorkspaceEntry>;
    /** Paths removed by this layer. The content adapter must materialize overlay whiteouts. */
    readonly deletedPaths: ReadonlyArray<string>;
  }) => Promise<{
    readonly observedContentDigest: Sha256Digest;
    readonly restoredPaths: ReadonlyArray<string>;
    readonly bytesRead: number;
  }>;
  readonly warmCaches: (input: {
    readonly allocation: CloudComputerWorkspaceAllocation;
    readonly cacheRefs: ReadonlyArray<CloudComputerWorkspaceCacheRef>;
    readonly readOnly: true;
  }) => Promise<{ readonly hitRefs: ReadonlyArray<string>; readonly bytesRead: number }>;
  readonly benchmarkLocalStorage: (input: {
    readonly allocation: CloudComputerWorkspaceAllocation;
  }) => Promise<CloudComputerWorkspaceBenchmark>;
  readonly captureOverlay: (input: {
    readonly allocation: CloudComputerWorkspaceAllocation;
    readonly boundary: "explicit" | "bounded_interval" | "stop" | "host_replacement";
    readonly include: (entry: CloudComputerWorkspaceEntry) => boolean;
    readonly preserveWhiteouts: true;
  }) => Promise<{
    readonly contentRef: CloudComputerWorkspaceContentRef;
    readonly contentDigest: Sha256Digest;
    readonly entries: ReadonlyArray<CloudComputerWorkspaceEntry>;
    readonly deletedPaths: ReadonlyArray<string>;
    readonly byteCount: number;
    readonly durationMs: number;
  }>;
  readonly release: (input: {
    readonly allocation: CloudComputerWorkspaceAllocation;
    readonly reason: "restore_failed" | "checkpointed" | "destroyed";
  }) => Promise<void>;
}

export type LocalCowWorkspacePorts = Readonly<{
  filesystem: Readonly<{
    mkdir: (
      path: string,
      options: { readonly recursive: boolean; readonly mode: number },
    ) => Promise<void>;
    removeTree: (path: string) => Promise<void>;
  }>;
  commands: Readonly<{
    run: (input: {
      readonly executable: "/bin/mount" | "/bin/umount" | "/usr/bin/git";
      readonly args: ReadonlyArray<string>;
    }) => Promise<{ readonly exitCode: number }>;
  }>;
  images: Readonly<{
    verify: (image: CloudComputerSignedBaseImage) => Promise<{
      readonly verifiedDigest: Sha256Digest;
      readonly signerIdentity: string;
      readonly localImagePath: string;
    }>;
  }>;
  contents: Readonly<{
    restoreAtomically: (input: {
      readonly contentRef: CloudComputerWorkspaceContentRef;
      readonly expectedContentDigest: Sha256Digest;
      readonly destinationPath: string;
      /** Full replaces prior state; delta merges entries and applies whiteouts. */
      readonly kind: "full" | "delta";
      readonly entries: ReadonlyArray<CloudComputerWorkspaceEntry>;
      readonly deletedPaths: ReadonlyArray<string>;
    }) => Promise<{
      readonly observedContentDigest: Sha256Digest;
      readonly restoredPaths: ReadonlyArray<string>;
      readonly bytesRead: number;
    }>;
    captureAtomically: (input: {
      readonly sourcePath: string;
      readonly boundary: "explicit" | "bounded_interval" | "stop" | "host_replacement";
      readonly include: (entry: CloudComputerWorkspaceEntry) => boolean;
      readonly preserveWhiteouts: true;
    }) => Promise<{
      readonly contentRef: CloudComputerWorkspaceContentRef;
      readonly contentDigest: Sha256Digest;
      readonly entries: ReadonlyArray<CloudComputerWorkspaceEntry>;
      readonly deletedPaths: ReadonlyArray<string>;
      readonly byteCount: number;
      readonly durationMs: number;
    }>;
  }>;
  caches: Readonly<{
    warmReadOnly: (input: {
      readonly workspacePath: string;
      readonly refs: ReadonlyArray<CloudComputerWorkspaceCacheRef>;
    }) => Promise<{ readonly hitRefs: ReadonlyArray<string>; readonly bytesRead: number }>;
  }>;
  benchmark: (workspacePath: string) => Promise<CloudComputerWorkspaceBenchmark>;
}>;

const assertCommand = (
  result: { readonly exitCode: number },
  operation: string,
  allowed: ReadonlyArray<number> = [0],
): void => {
  if (!allowed.includes(result.exitCode)) {
    throw new CloudComputerWorkspaceError(
      "invalid_input",
      `${operation} exited ${result.exitCode}`,
    );
  }
};

/** Linux overlayfs adapter. All privileged calls use fixed executables and argv, never a shell. */
export const createLocalCowWorkspaceRunner = (input: {
  readonly rootPath: string;
  readonly ports: LocalCowWorkspacePorts;
}): CloudComputerWorkspaceRunner => {
  if (
    !input.rootPath.startsWith("/") ||
    input.rootPath === "/" ||
    input.rootPath.includes("\0") ||
    input.rootPath.split("/").includes("..")
  ) {
    throw new CloudComputerWorkspaceError("invalid_input", "invalid local workspace root");
  }
  const verifiedImages = new Map<Sha256Digest, string>();
  const allocations = new Map<string, CloudComputerWorkspaceAllocation>();
  const assertOwnedAllocation = (allocation: CloudComputerWorkspaceAllocation): void => {
    const owned = allocations.get(allocation.allocationRef);
    if (
      owned === undefined ||
      owned.rootPath !== allocation.rootPath ||
      owned.lowerPath !== allocation.lowerPath ||
      owned.upperPath !== allocation.upperPath ||
      owned.workPath !== allocation.workPath ||
      owned.mergedPath !== allocation.mergedPath
    ) {
      throw new CloudComputerWorkspaceError(
        "invalid_input",
        "workspace allocation is not owned by this runner",
      );
    }
  };
  const command = async (
    executable: "/bin/mount" | "/bin/umount" | "/usr/bin/git",
    args: ReadonlyArray<string>,
    operation: string,
    allowed?: ReadonlyArray<number>,
  ): Promise<void> =>
    assertCommand(await input.ports.commands.run({ executable, args }), operation, allowed);

  return {
    allocate: async ({ workspaceRef, generation }) => {
      assertPublicRef(workspaceRef, "workspace ref");
      if (!Number.isSafeInteger(generation) || generation < 1) {
        throw new CloudComputerWorkspaceError("invalid_input", "invalid workspace generation");
      }
      const key = createHash("sha256").update(workspaceRef).digest("hex").slice(0, 32);
      const rootPath = `${input.rootPath}/${key}-${generation}`;
      const allocation: CloudComputerWorkspaceAllocation = {
        allocationRef: `allocation.${key}.${generation}`,
        rootPath,
        lowerPath: `${rootPath}/lower`,
        upperPath: `${rootPath}/upper`,
        workPath: `${rootPath}/work`,
        mergedPath: `${rootPath}/merged`,
      };
      await input.ports.filesystem.mkdir(input.rootPath, { recursive: true, mode: 0o700 });
      await input.ports.filesystem.mkdir(rootPath, { recursive: false, mode: 0o700 });
      try {
        await Promise.all(
          [
            allocation.lowerPath,
            allocation.upperPath,
            allocation.workPath,
            allocation.mergedPath,
          ].map((path) => input.ports.filesystem.mkdir(path, { recursive: false, mode: 0o700 })),
        );
        allocations.set(allocation.allocationRef, allocation);
        return allocation;
      } catch (error) {
        await input.ports.filesystem.removeTree(rootPath);
        throw error;
      }
    },
    verifySignedBaseImage: async (image) => {
      const verified = await input.ports.images.verify(image);
      if (
        !verified.localImagePath.startsWith("/") ||
        verified.localImagePath.includes("\0") ||
        verified.localImagePath.split("/").includes("..")
      ) {
        throw new CloudComputerWorkspaceError(
          "base_image_verification_failed",
          "verified image path is not absolute",
        );
      }
      verifiedImages.set(verified.verifiedDigest, verified.localImagePath);
      return verified;
    },
    mountBaseImage: async ({ allocation, image }) => {
      assertOwnedAllocation(allocation);
      const imagePath = verifiedImages.get(image.digest);
      if (imagePath === undefined) {
        throw new CloudComputerWorkspaceError(
          "base_image_verification_failed",
          "base image was not verified",
        );
      }
      await command(
        "/bin/mount",
        ["-o", "loop,ro,nodev,nosuid", "--", imagePath, allocation.lowerPath],
        "mount base image",
      );
    },
    mountOverlay: async ({ allocation }) => {
      assertOwnedAllocation(allocation);
      await command(
        "/bin/mount",
        [
          "-t",
          "overlay",
          "overlay",
          "-o",
          `lowerdir=${allocation.lowerPath},upperdir=${allocation.upperPath},workdir=${allocation.workPath}`,
          "--",
          allocation.mergedPath,
        ],
        "mount workspace overlay",
      );
    },
    restoreEntries: async ({
      allocation,
      contentRef,
      expectedContentDigest,
      kind,
      entries,
      deletedPaths,
    }) => {
      assertOwnedAllocation(allocation);
      const restored = await input.ports.contents.restoreAtomically({
        contentRef,
        expectedContentDigest,
        destinationPath: allocation.upperPath,
        kind,
        entries,
        deletedPaths,
      });
      if (entries.some((entry) => entry.path === ".git/HEAD")) {
        await command(
          "/usr/bin/git",
          ["-C", allocation.upperPath, "config", "--local", "--unset-all", "credential.helper"],
          "clear Git credential helper",
          [0, 5],
        );
        await command(
          "/usr/bin/git",
          ["-C", allocation.upperPath, "config", "--local", "--unset-all", "http.extraheader"],
          "clear Git authorization header",
          [0, 5],
        );
        await command(
          "/usr/bin/git",
          ["-C", allocation.upperPath, "config", "--local", "core.hooksPath", "/dev/null"],
          "disable Git hooks",
        );
      }
      return restored;
    },
    warmCaches: ({ allocation, cacheRefs }) => {
      assertOwnedAllocation(allocation);
      return input.ports.caches.warmReadOnly({
        workspacePath: allocation.mergedPath,
        refs: cacheRefs,
      });
    },
    benchmarkLocalStorage: ({ allocation }) => {
      assertOwnedAllocation(allocation);
      return input.ports.benchmark(allocation.mergedPath);
    },
    captureOverlay: ({ allocation, boundary, include, preserveWhiteouts }) => {
      assertOwnedAllocation(allocation);
      return input.ports.contents.captureAtomically({
        sourcePath: allocation.upperPath,
        boundary,
        include,
        preserveWhiteouts,
      });
    },
    release: async ({ allocation }) => {
      assertOwnedAllocation(allocation);
      await command(
        "/bin/umount",
        ["--", allocation.mergedPath],
        "unmount workspace overlay",
        [0, 32],
      );
      await command("/bin/umount", ["--", allocation.lowerPath], "unmount base image", [0, 32]);
      await input.ports.filesystem.removeTree(allocation.rootPath);
      allocations.delete(allocation.allocationRef);
    },
  };
};

export type CloudComputerWorkspaceRestoreReceipt = Readonly<{
  schema: typeof CLOUD_COMPUTER_WORKSPACE_RESTORE_RECEIPT_SCHEMA;
  checkpointRef: string;
  workspaceRef: string;
  generation: number;
  baseImageDigest: Sha256Digest;
  contentDigest: Sha256Digest;
  checkpointRefs: ReadonlyArray<string>;
  restoredPaths: ReadonlyArray<string>;
  excludedPaths: ReadonlyArray<string>;
  restoredByteCount: number;
  checkpointBytesRead: number;
  cacheRefs: ReadonlyArray<string>;
  cacheHitRefs: ReadonlyArray<string>;
  cacheBytesRead: number;
  cacheWarmDurationMs: number;
  workspaceReadyDurationMs: number;
  restoreDurationMs: number;
  benchmark: CloudComputerWorkspaceBenchmark;
  checkpointOverheadMs: number;
}>;

export type CloudComputerWorkspaceCheckpointReceipt = Readonly<{
  schema: typeof CLOUD_COMPUTER_WORKSPACE_CHECKPOINT_RECEIPT_SCHEMA;
  workspaceRef: string;
  generation: number;
  parentCheckpointRef: string | null;
  kind: "full" | "delta";
  contentRef: CloudComputerWorkspaceContentRef;
  contentDigest: Sha256Digest;
  includedPaths: ReadonlyArray<string>;
  deletedPaths: ReadonlyArray<string>;
  byteCount: number;
  durationMs: number;
  boundary: "explicit" | "bounded_interval" | "stop" | "host_replacement";
}>;

export class CloudComputerWorkspaceError extends Error {
  constructor(
    readonly code:
      | "invalid_input"
      | "unauthorized_scope"
      | "unpinned_base_image"
      | "base_image_verification_failed"
      | "checkpoint_integrity_failed"
      | "unsafe_checkpoint_entry",
    message: string,
  ) {
    super(message);
    this.name = "CloudComputerWorkspaceError";
  }
}

const digestPattern = /^sha256:[a-f0-9]{64}$/u;
const publicRefPattern = /^[a-zA-Z0-9][a-zA-Z0-9._:/@+-]{0,255}$/u;

const assertPublicRef = (value: string, field: string): void => {
  if (!publicRefPattern.test(value)) {
    throw new CloudComputerWorkspaceError("invalid_input", `invalid ${field}`);
  }
};

const assertDigest: (value: string, field: string) => asserts value is Sha256Digest = (
  value,
  field,
) => {
  if (!digestPattern.test(value)) {
    throw new CloudComputerWorkspaceError("invalid_input", `invalid ${field}`);
  }
};

const sorted = (values: Iterable<string>): Array<string> => Array.from(values).sort();

const safeRelativePath = (path: string): boolean => {
  if (path.length === 0 || path.length > 4_096 || path.startsWith("/") || path.includes("\\")) {
    return false;
  }
  if (path.includes("\0")) return false;
  return path
    .split("/")
    .every((segment) => segment.length > 0 && segment !== "." && segment !== "..");
};

const structurallyExcluded = (path: string): boolean => {
  const segments = path.split("/");
  const first = segments[0] ?? "";
  const second = segments[1] ?? "";
  if (
    segments.some(
      (segment) =>
        segment === ".env" || (segment.startsWith(".env.") && segment !== ".env.example"),
    )
  )
    return true;
  if ([".aws", ".gnupg", ".ssh"].includes(first)) return true;
  if ([".netrc", ".npmrc", ".pypirc"].includes(first)) return true;
  if (first === ".codex" && second === "auth.json") return true;
  if (first === ".docker" && second === "config.json") return true;
  if (first === ".config" && second === "gcloud") return true;
  if (
    first === ".git" &&
    [
      "config",
      "config.worktree",
      "credential",
      "credentials",
      "credential-cache",
      "hooks",
    ].includes(second)
  )
    return true;
  if (
    first === ".openagents" &&
    ["credentials", "provider", "runtime", "secrets", "sockets"].includes(second)
  ) {
    return true;
  }
  return false;
};

export const cloudComputerWorkspaceEntryAdmitted = (
  entry: CloudComputerWorkspaceEntry,
): boolean => {
  if (!safeRelativePath(entry.path) || !entry.admitted) return false;
  if (entry.classification !== "workspace" && entry.classification !== "git_metadata") return false;
  if (structurallyExcluded(entry.path) || !checkpointPathAdmitted(entry.path)) return false;
  return entry.kind === "directory" || entry.kind === "file" || entry.kind === "symlink";
};

const validateEntry = (entry: CloudComputerWorkspaceEntry): void => {
  if (
    !safeRelativePath(entry.path) ||
    !Number.isSafeInteger(entry.byteCount) ||
    entry.byteCount < 0
  ) {
    throw new CloudComputerWorkspaceError("unsafe_checkpoint_entry", `unsafe entry ${entry.path}`);
  }
  if (entry.kind === "file") {
    if (entry.digest === null)
      throw new CloudComputerWorkspaceError(
        "unsafe_checkpoint_entry",
        `missing digest for ${entry.path}`,
      );
    assertDigest(entry.digest, `entry digest for ${entry.path}`);
  }
  if (entry.kind === "symlink") {
    if (entry.linkTarget === undefined || !safeRelativePath(entry.linkTarget)) {
      throw new CloudComputerWorkspaceError(
        "unsafe_checkpoint_entry",
        `unsafe symlink ${entry.path}`,
      );
    }
  }
};

const validateBaseImage = (image: CloudComputerSignedBaseImage): void => {
  assertDigest(image.digest, "base image digest");
  assertPublicRef(image.signatureRef, "base image signature ref");
  assertPublicRef(image.signerIdentity, "base image signer identity");
  if (!image.imageRef.endsWith(`@${image.digest}`)) {
    throw new CloudComputerWorkspaceError(
      "unpinned_base_image",
      "base image ref must pin its digest",
    );
  }
};

const validateAllocation = (allocation: CloudComputerWorkspaceAllocation): void => {
  const paths = [
    allocation.rootPath,
    allocation.lowerPath,
    allocation.upperPath,
    allocation.workPath,
    allocation.mergedPath,
  ];
  if (
    paths.some(
      (path) => !path.startsWith("/") || path.includes("\0") || path.split("/").includes(".."),
    )
  ) {
    throw new CloudComputerWorkspaceError("invalid_input", "runner returned an unsafe allocation");
  }
  if (
    new Set(paths).size !== paths.length ||
    paths.some(
      (path) => path !== allocation.rootPath && !path.startsWith(`${allocation.rootPath}/`),
    )
  ) {
    throw new CloudComputerWorkspaceError(
      "invalid_input",
      "runner returned an invalid allocation layout",
    );
  }
};

export const restoreCloudComputerWorkspace = async (input: {
  /** Ordered ancestor chain. The first checkpoint is full and the last is the restore target. */
  readonly checkpoints: readonly [
    CloudComputerWorkspaceCheckpoint,
    ...ReadonlyArray<CloudComputerWorkspaceCheckpoint>,
  ];
  readonly baseImage: CloudComputerSignedBaseImage;
  readonly cacheRefs: ReadonlyArray<CloudComputerWorkspaceCacheRef>;
  readonly requestedOwnerRef: string;
  readonly requestedTenantRef: string;
  readonly runner: CloudComputerWorkspaceRunner;
  readonly now?: () => number;
}): Promise<{
  readonly allocation: CloudComputerWorkspaceAllocation;
  readonly receipt: CloudComputerWorkspaceRestoreReceipt;
}> => {
  const { checkpoints, baseImage, runner } = input;
  const checkpoint = checkpoints.at(-1)!;
  const checkpointRefs = new Set<string>();
  checkpoints.forEach((layer, index) => {
    assertPublicRef(layer.checkpointRef, "checkpoint ref");
    assertPublicRef(layer.workspaceRef, "workspace ref");
    assertDigest(layer.contentDigest, "checkpoint content digest");
    assertDigest(layer.baseImageDigest, "checkpoint base image digest");
    if (!Number.isSafeInteger(layer.generation) || layer.generation < 1) {
      throw new CloudComputerWorkspaceError("invalid_input", "invalid workspace generation");
    }
    if (checkpointRefs.has(layer.checkpointRef)) {
      throw new CloudComputerWorkspaceError("checkpoint_integrity_failed", "duplicate checkpoint");
    }
    checkpointRefs.add(layer.checkpointRef);
    if (
      layer.ownerRef !== input.requestedOwnerRef ||
      layer.tenantRef !== input.requestedTenantRef ||
      layer.workspaceRef !== checkpoint.workspaceRef
    ) {
      throw new CloudComputerWorkspaceError(
        "unauthorized_scope",
        "checkpoint chain scope does not match the restore authority",
      );
    }
    if (layer.baseImageDigest !== baseImage.digest) {
      throw new CloudComputerWorkspaceError(
        "base_image_verification_failed",
        "signed base image differs from the checkpoint chain binding",
      );
    }
    if (index === 0) {
      if (layer.kind !== "full" || layer.parentCheckpointRef !== null) {
        throw new CloudComputerWorkspaceError(
          "checkpoint_integrity_failed",
          "checkpoint chain must start with a root full checkpoint",
        );
      }
    } else {
      const parent = checkpoints[index - 1]!;
      if (layer.kind !== "delta" || layer.parentCheckpointRef !== parent.checkpointRef) {
        throw new CloudComputerWorkspaceError(
          "checkpoint_integrity_failed",
          "checkpoint chain is not an ordered full and delta ancestry",
        );
      }
    }
    layer.entries.forEach(validateEntry);
    const entryPaths = new Set(layer.entries.map((entry) => entry.path));
    if (
      new Set(layer.deletedPaths).size !== layer.deletedPaths.length ||
      layer.deletedPaths.some((path) => !safeRelativePath(path) || entryPaths.has(path))
    ) {
      throw new CloudComputerWorkspaceError(
        "unsafe_checkpoint_entry",
        "checkpoint contains an unsafe or conflicting deletion tombstone",
      );
    }
    const excluded = layer.entries.filter((entry) => !cloudComputerWorkspaceEntryAdmitted(entry));
    if (excluded.length > 0) {
      throw new CloudComputerWorkspaceError(
        "unsafe_checkpoint_entry",
        "committed checkpoint contains an excluded entry",
      );
    }
    if (layer.byteCount !== layer.entries.reduce((total, entry) => total + entry.byteCount, 0)) {
      throw new CloudComputerWorkspaceError(
        "checkpoint_integrity_failed",
        "checkpoint byte count does not match its entries",
      );
    }
  });
  validateBaseImage(baseImage);
  for (const cacheRef of input.cacheRefs) {
    assertPublicRef(cacheRef.cacheRef, "cache ref");
    assertDigest(cacheRef.digest, "cache digest");
  }
  const finalEntries = new Map<string, CloudComputerWorkspaceEntry>();
  for (const layer of checkpoints) {
    for (const deletedPath of layer.deletedPaths) {
      for (const path of finalEntries.keys()) {
        if (path === deletedPath || path.startsWith(`${deletedPath}/`)) finalEntries.delete(path);
      }
    }
    for (const entry of layer.entries) finalEntries.set(entry.path, entry);
  }
  const admittedByteCount = Array.from(finalEntries.values()).reduce(
    (total, entry) => total + entry.byteCount,
    0,
  );

  const now = input.now ?? Date.now;
  const startedAt = now();
  const allocation = await runner.allocate({
    workspaceRef: checkpoint.workspaceRef,
    generation: checkpoint.generation,
  });
  try {
    validateAllocation(allocation);
    const verified = await runner.verifySignedBaseImage(baseImage);
    if (
      verified.verifiedDigest !== baseImage.digest ||
      verified.signerIdentity !== baseImage.signerIdentity
    ) {
      throw new CloudComputerWorkspaceError(
        "base_image_verification_failed",
        "base image signature did not match the pinned image",
      );
    }
    await runner.mountBaseImage({ allocation, image: baseImage, readOnly: true });
    const restoreStartedAt = now();
    let checkpointBytesRead = 0;
    for (const layer of checkpoints) {
      // Layers mutate one overlay and must apply in ancestor order.
      // eslint-disable-next-line no-await-in-loop
      const restored = await runner.restoreEntries({
        allocation,
        contentRef: layer.contentRef,
        expectedContentDigest: layer.contentDigest,
        kind: layer.kind,
        entries: layer.entries,
        deletedPaths: layer.deletedPaths,
      });
      if (restored.observedContentDigest !== layer.contentDigest) {
        throw new CloudComputerWorkspaceError(
          "checkpoint_integrity_failed",
          "restored checkpoint digest did not match",
        );
      }
      const expectedLayerPaths = sorted(layer.entries.map((entry) => entry.path));
      const restoredLayerPaths = sorted(restored.restoredPaths);
      if (
        new Set(restoredLayerPaths).size !== restoredLayerPaths.length ||
        JSON.stringify(restoredLayerPaths) !== JSON.stringify(expectedLayerPaths)
      ) {
        throw new CloudComputerWorkspaceError(
          "checkpoint_integrity_failed",
          "restored paths did not match the checkpoint layer",
        );
      }
      checkpointBytesRead += restored.bytesRead;
    }
    await runner.mountOverlay({ allocation, lowerReadOnly: true });
    const restoredAt = now();
    const cacheStartedAt = restoredAt;
    const caches = await runner.warmCaches({
      allocation,
      cacheRefs: input.cacheRefs,
      readOnly: true,
    });
    const cacheFinishedAt = now();
    const restoredPaths = sorted(finalEntries.keys());
    const benchmark = await runner.benchmarkLocalStorage({ allocation });
    return {
      allocation,
      receipt: {
        schema: CLOUD_COMPUTER_WORKSPACE_RESTORE_RECEIPT_SCHEMA,
        checkpointRef: checkpoint.checkpointRef,
        workspaceRef: checkpoint.workspaceRef,
        generation: checkpoint.generation,
        baseImageDigest: baseImage.digest,
        contentDigest: checkpoint.contentDigest,
        checkpointRefs: checkpoints.map((layer) => layer.checkpointRef),
        restoredPaths,
        excludedPaths: [],
        restoredByteCount: admittedByteCount,
        checkpointBytesRead,
        cacheRefs: input.cacheRefs.map((ref) => ref.cacheRef),
        cacheHitRefs: sorted(caches.hitRefs),
        cacheBytesRead: caches.bytesRead,
        cacheWarmDurationMs: cacheFinishedAt - cacheStartedAt,
        workspaceReadyDurationMs: cacheFinishedAt - startedAt,
        restoreDurationMs: restoredAt - restoreStartedAt,
        benchmark,
        checkpointOverheadMs: restoredAt - restoreStartedAt,
      },
    };
  } catch (error) {
    await runner.release({ allocation, reason: "restore_failed" });
    throw error;
  }
};

export const checkpointCloudComputerWorkspace = async (input: {
  readonly allocation: CloudComputerWorkspaceAllocation;
  readonly workspaceRef: string;
  readonly generation: number;
  readonly baseImageDigest: Sha256Digest;
  readonly parentCheckpointRef: string | null;
  readonly boundary: "explicit" | "bounded_interval" | "stop" | "host_replacement";
  readonly runner: CloudComputerWorkspaceRunner;
}): Promise<{
  readonly artifact: Omit<
    CloudComputerWorkspaceCheckpoint,
    "checkpointRef" | "ownerRef" | "tenantRef"
  >;
  readonly receipt: CloudComputerWorkspaceCheckpointReceipt;
}> => {
  validateAllocation(input.allocation);
  assertDigest(input.baseImageDigest, "base image digest");
  const captured = await input.runner.captureOverlay({
    allocation: input.allocation,
    boundary: input.boundary,
    include: cloudComputerWorkspaceEntryAdmitted,
    preserveWhiteouts: true,
  });
  assertDigest(captured.contentDigest, "captured content digest");
  captured.entries.forEach(validateEntry);
  if (captured.entries.some((entry) => !cloudComputerWorkspaceEntryAdmitted(entry))) {
    throw new CloudComputerWorkspaceError(
      "unsafe_checkpoint_entry",
      "capture returned an excluded workspace entry",
    );
  }
  const capturedPaths = new Set(captured.entries.map((entry) => entry.path));
  if (
    new Set(captured.deletedPaths).size !== captured.deletedPaths.length ||
    captured.deletedPaths.some((path) => !safeRelativePath(path) || capturedPaths.has(path))
  ) {
    throw new CloudComputerWorkspaceError(
      "unsafe_checkpoint_entry",
      "capture returned an unsafe or conflicting deletion tombstone",
    );
  }
  const byteCount = captured.entries.reduce((total, entry) => total + entry.byteCount, 0);
  if (byteCount !== captured.byteCount) {
    throw new CloudComputerWorkspaceError(
      "checkpoint_integrity_failed",
      "captured byte count did not match entries",
    );
  }
  const includedPaths = sorted(captured.entries.map((entry) => entry.path));
  return {
    artifact: {
      workspaceRef: input.workspaceRef,
      generation: input.generation,
      kind: input.parentCheckpointRef === null ? "full" : "delta",
      baseImageDigest: input.baseImageDigest,
      parentCheckpointRef: input.parentCheckpointRef,
      contentRef: captured.contentRef,
      contentDigest: captured.contentDigest,
      byteCount,
      entries: captured.entries,
      deletedPaths: captured.deletedPaths,
    },
    receipt: {
      schema: CLOUD_COMPUTER_WORKSPACE_CHECKPOINT_RECEIPT_SCHEMA,
      workspaceRef: input.workspaceRef,
      generation: input.generation,
      parentCheckpointRef: input.parentCheckpointRef,
      kind: input.parentCheckpointRef === null ? "full" : "delta",
      contentRef: captured.contentRef,
      contentDigest: captured.contentDigest,
      includedPaths,
      deletedPaths: sorted(captured.deletedPaths),
      byteCount,
      durationMs: captured.durationMs,
      boundary: input.boundary,
    },
  };
};
