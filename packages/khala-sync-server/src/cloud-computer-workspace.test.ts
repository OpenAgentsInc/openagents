import { describe, expect, test, vi } from "vite-plus/test";

import {
  checkpointCloudComputerWorkspace,
  CloudComputerWorkspaceError,
  cloudComputerWorkspaceContentRef,
  createLocalCowWorkspaceRunner,
  restoreCloudComputerWorkspace,
  type CloudComputerWorkspaceAllocation,
  type CloudComputerWorkspaceCheckpoint,
  type CloudComputerWorkspaceEntry,
  type CloudComputerWorkspaceRunner,
  type LocalCowWorkspacePorts,
} from "./cloud-computer-workspace.js";

const sha = (character: string): `sha256:${string}` => `sha256:${character.repeat(64)}`;
const allocation: CloudComputerWorkspaceAllocation = {
  allocationRef: "allocation.workspace.1",
  rootPath: "/var/lib/openagents/workspaces/one",
  lowerPath: "/var/lib/openagents/workspaces/one/lower",
  upperPath: "/var/lib/openagents/workspaces/one/upper",
  workPath: "/var/lib/openagents/workspaces/one/work",
  mergedPath: "/var/lib/openagents/workspaces/one/merged",
};
const entries: ReadonlyArray<CloudComputerWorkspaceEntry> = [
  {
    path: "src/index.ts",
    kind: "file",
    classification: "workspace",
    admitted: true,
    byteCount: 12,
    digest: sha("1"),
  },
  {
    path: ".git/HEAD",
    kind: "file",
    classification: "git_metadata",
    admitted: true,
    byteCount: 20,
    digest: sha("2"),
  },
  {
    path: ".git/refs/heads/main",
    kind: "file",
    classification: "git_metadata",
    admitted: true,
    byteCount: 40,
    digest: sha("3"),
  },
  {
    path: ".env",
    kind: "file",
    classification: "secret",
    admitted: false,
    byteCount: 8,
    digest: sha("4"),
  },
  {
    path: ".openagents/runtime/provider.json",
    kind: "file",
    classification: "runtime_metadata",
    admitted: false,
    byteCount: 9,
    digest: sha("5"),
  },
];
const checkpoint: CloudComputerWorkspaceCheckpoint = {
  checkpointRef: "checkpoint.workspace.7",
  workspaceRef: "workspace.one",
  ownerRef: "owner.one",
  tenantRef: "tenant.one",
  generation: 7,
  kind: "full",
  baseImageDigest: sha("c"),
  parentCheckpointRef: null,
  contentRef: cloudComputerWorkspaceContentRef("content.workspace.one"),
  contentDigest: sha("a"),
  byteCount: entries.slice(0, 3).reduce((total, entry) => total + entry.byteCount, 0),
  entries: entries.slice(0, 3),
  deletedPaths: [],
};

const makeRunner = () =>
  ({
    allocate: vi.fn(async () => allocation),
    verifySignedBaseImage: vi.fn(async (image) => ({
      verifiedDigest: image.digest,
      signerIdentity: image.signerIdentity,
    })),
    mountBaseImage: vi.fn(async () => undefined),
    mountOverlay: vi.fn(async () => undefined),
    restoreEntries: vi.fn(
      async (input: Parameters<CloudComputerWorkspaceRunner["restoreEntries"]>[0]) => ({
        observedContentDigest: input.expectedContentDigest,
        restoredPaths: input.entries.map((entry) => entry.path),
        bytesRead: 72,
      }),
    ),
    warmCaches: vi.fn(async (input: Parameters<CloudComputerWorkspaceRunner["warmCaches"]>[0]) => ({
      hitRefs: input.cacheRefs.slice(0, 1).map((ref) => ref.cacheRef),
      bytesRead: 1_024,
    })),
    benchmarkLocalStorage: vi.fn(async () => ({
      dependencyInstallDurationMs: 300,
      largeFileWriteDurationMs: 20,
      largeFileBytes: 1_048_576,
    })),
    captureOverlay: vi.fn(async () => ({
      contentRef: cloudComputerWorkspaceContentRef("content.workspace.two"),
      contentDigest: sha("b"),
      entries: entries.slice(0, 3),
      deletedPaths: [],
      byteCount: 72,
      durationMs: 18,
    })),
    release: vi.fn(async () => undefined),
  }) satisfies CloudComputerWorkspaceRunner;

const baseImage = {
  imageRef: `us-docker.pkg.dev/openagents/runtime/agent@${sha("c")}`,
  digest: sha("c"),
  signatureRef: "signature.base-image.1",
  signerIdentity: "openagents-release",
} as const;

describe("cloud computer local workspace", () => {
  test("restores admitted files and Git metadata into a read-only-base overlay", async () => {
    const runner = makeRunner();
    const ticks = [100, 110, 120, 145];
    const result = await restoreCloudComputerWorkspace({
      checkpoints: [checkpoint],
      baseImage,
      cacheRefs: [{ cacheRef: "cache.toolchain.node", kind: "toolchain", digest: sha("d") }],
      requestedOwnerRef: "owner.one",
      requestedTenantRef: "tenant.one",
      runner,
      now: () => ticks.shift() ?? 145,
    });

    expect(runner.mountBaseImage).toHaveBeenCalledWith({
      allocation,
      image: baseImage,
      readOnly: true,
    });
    expect(runner.mountOverlay).toHaveBeenCalledWith({ allocation, lowerReadOnly: true });
    expect(runner.restoreEntries).toHaveBeenCalledWith(
      expect.objectContaining({
        entries: entries.slice(0, 3),
      }),
    );
    expect(result.receipt.restoredPaths).toEqual([
      ".git/HEAD",
      ".git/refs/heads/main",
      "src/index.ts",
    ]);
    expect(result.receipt.excludedPaths).toEqual([]);
    expect(result.receipt.cacheHitRefs).toEqual(["cache.toolchain.node"]);
    expect(result.receipt.cacheWarmDurationMs).toBe(25);
    expect(result.receipt.workspaceReadyDurationMs).toBe(45);
    expect(result.receipt.restoreDurationMs).toBe(10);
    expect(result.receipt.checkpointOverheadMs).toBe(10);
    expect(result.receipt.benchmark.largeFileBytes).toBe(1_048_576);
  });

  test("restores an ordered full and delta chain and applies deletion tombstones", async () => {
    const runner = makeRunner();
    const deltaEntry = {
      ...entries[0]!,
      path: "src/new.ts",
      digest: sha("8"),
    } as const;
    const delta: CloudComputerWorkspaceCheckpoint = {
      ...checkpoint,
      checkpointRef: "checkpoint.workspace.8",
      generation: 7,
      kind: "delta",
      parentCheckpointRef: checkpoint.checkpointRef,
      contentRef: cloudComputerWorkspaceContentRef("content.workspace.delta"),
      contentDigest: sha("8"),
      byteCount: deltaEntry.byteCount,
      entries: [deltaEntry],
      deletedPaths: [".git/HEAD", "src/index.ts"],
    };

    const result = await restoreCloudComputerWorkspace({
      checkpoints: [checkpoint, delta],
      baseImage,
      cacheRefs: [],
      requestedOwnerRef: "owner.one",
      requestedTenantRef: "tenant.one",
      runner,
    });

    expect(runner.restoreEntries).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        contentRef: checkpoint.contentRef,
        kind: "full",
        entries: checkpoint.entries,
        deletedPaths: [],
      }),
    );
    expect(runner.restoreEntries).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        contentRef: delta.contentRef,
        kind: "delta",
        entries: [deltaEntry],
        deletedPaths: [".git/HEAD", "src/index.ts"],
      }),
    );
    expect(result.receipt.checkpointRefs).toEqual([
      "checkpoint.workspace.7",
      "checkpoint.workspace.8",
    ]);
    expect(result.receipt.restoredPaths).toEqual([".git/refs/heads/main", "src/new.ts"]);
    expect(result.receipt.checkpointBytesRead).toBe(144);
    expect(runner.restoreEntries.mock.invocationCallOrder[1]).toBeLessThan(
      runner.mountOverlay.mock.invocationCallOrder[0]!,
    );
  });

  test("rejects a delta with a missing or reordered ancestor", async () => {
    const runner = makeRunner();
    const broken = {
      ...checkpoint,
      checkpointRef: "checkpoint.workspace.8",
      generation: 8,
      kind: "delta",
      parentCheckpointRef: "checkpoint.workspace.missing",
    } as const;
    await expect(
      restoreCloudComputerWorkspace({
        checkpoints: [checkpoint, broken],
        baseImage,
        cacheRefs: [],
        requestedOwnerRef: "owner.one",
        requestedTenantRef: "tenant.one",
        runner,
      }),
    ).rejects.toMatchObject({ code: "checkpoint_integrity_failed" });
    expect(runner.allocate).not.toHaveBeenCalled();
  });

  test("refuses cross-owner and cross-tenant restores before allocating local storage", async () => {
    const runner = makeRunner();
    await expect(
      restoreCloudComputerWorkspace({
        checkpoints: [checkpoint],
        baseImage,
        cacheRefs: [],
        requestedOwnerRef: "owner.two",
        requestedTenantRef: "tenant.one",
        runner,
      }),
    ).rejects.toMatchObject({ code: "unauthorized_scope" });
    expect(runner.allocate).not.toHaveBeenCalled();
  });

  test("requires a signed image ref pinned to the verified digest", async () => {
    const runner = makeRunner();
    await expect(
      restoreCloudComputerWorkspace({
        checkpoints: [checkpoint],
        baseImage: { ...baseImage, imageRef: "us-docker.pkg.dev/openagents/runtime/agent:latest" },
        cacheRefs: [],
        requestedOwnerRef: "owner.one",
        requestedTenantRef: "tenant.one",
        runner,
      }),
    ).rejects.toMatchObject({ code: "unpinned_base_image" });

    const mismatchedRunner = makeRunner();
    mismatchedRunner.verifySignedBaseImage = vi.fn(async () => ({
      verifiedDigest: sha("e"),
      signerIdentity: "openagents-release",
    }));
    await expect(
      restoreCloudComputerWorkspace({
        checkpoints: [checkpoint],
        baseImage,
        cacheRefs: [],
        requestedOwnerRef: "owner.one",
        requestedTenantRef: "tenant.one",
        runner: mismatchedRunner,
      }),
    ).rejects.toMatchObject({ code: "base_image_verification_failed" });
    expect(mismatchedRunner.release).toHaveBeenCalledWith({ allocation, reason: "restore_failed" });
  });

  test("rejects a signed image that differs from a committed checkpoint layer", async () => {
    const runner = makeRunner();
    const otherImage = {
      ...baseImage,
      imageRef: `us-docker.pkg.dev/openagents/runtime/agent@${sha("e")}`,
      digest: sha("e"),
    } as const;
    await expect(
      restoreCloudComputerWorkspace({
        checkpoints: [checkpoint],
        baseImage: otherImage,
        cacheRefs: [],
        requestedOwnerRef: "owner.one",
        requestedTenantRef: "tenant.one",
        runner,
      }),
    ).rejects.toMatchObject({ code: "base_image_verification_failed" });
    expect(runner.allocate).not.toHaveBeenCalled();
  });

  test("rejects traversal and escaping symlinks without invoking the runner", async () => {
    const attempts = [
      { ...entries[0]!, path: "../secrets" },
      {
        path: "link",
        kind: "symlink",
        classification: "workspace",
        admitted: true,
        byteCount: 0,
        digest: null,
        linkTarget: "../../etc/passwd",
      } as const,
    ].map(async (unsafe) => {
      const runner = makeRunner();
      const unsafeCheckpoint = { ...checkpoint, entries: [unsafe], byteCount: unsafe.byteCount };
      await expect(
        restoreCloudComputerWorkspace({
          checkpoints: [unsafeCheckpoint],
          baseImage,
          cacheRefs: [],
          requestedOwnerRef: "owner.one",
          requestedTenantRef: "tenant.one",
          runner,
        }),
      ).rejects.toBeInstanceOf(CloudComputerWorkspaceError);
      expect(runner.allocate).not.toHaveBeenCalled();
    });
    await Promise.all(attempts);
  });

  test("rejects a committed manifest containing credential-bearing Git metadata", async () => {
    const secret = {
      ...entries[0]!,
      path: ".git/credentials",
      classification: "git_metadata",
      admitted: true,
    } as const;
    const runner = makeRunner();
    await expect(
      restoreCloudComputerWorkspace({
        checkpoints: [{ ...checkpoint, entries: [secret], byteCount: secret.byteCount }],
        baseImage,
        cacheRefs: [],
        requestedOwnerRef: "owner.one",
        requestedTenantRef: "tenant.one",
        runner,
      }),
    ).rejects.toMatchObject({ code: "unsafe_checkpoint_entry" });
    expect(runner.allocate).not.toHaveBeenCalled();
  });

  test("never restores sockets or nested environment files marked admitted", async () => {
    const unsafeEntries: ReadonlyArray<CloudComputerWorkspaceEntry> = [
      { ...entries[0]!, path: "packages/app/.env.production" },
      {
        path: "agent.sock",
        kind: "socket",
        classification: "workspace",
        admitted: true,
        byteCount: 0,
        digest: null,
      },
    ];
    const runner = makeRunner();
    await expect(
      restoreCloudComputerWorkspace({
        checkpoints: [
          {
            ...checkpoint,
            entries: unsafeEntries,
            byteCount: unsafeEntries[0]!.byteCount,
          },
        ],
        baseImage,
        cacheRefs: [],
        requestedOwnerRef: "owner.one",
        requestedTenantRef: "tenant.one",
        runner,
      }),
    ).rejects.toMatchObject({ code: "unsafe_checkpoint_entry" });
    expect(runner.allocate).not.toHaveBeenCalled();
  });

  test("fails closed and releases the allocation on corrupt restored content", async () => {
    const runner = makeRunner();
    runner.restoreEntries = vi.fn(async () => ({
      observedContentDigest: sha("f"),
      restoredPaths: [],
      bytesRead: 2,
    }));
    await expect(
      restoreCloudComputerWorkspace({
        checkpoints: [checkpoint],
        baseImage,
        cacheRefs: [],
        requestedOwnerRef: "owner.one",
        requestedTenantRef: "tenant.one",
        runner,
      }),
    ).rejects.toMatchObject({ code: "checkpoint_integrity_failed" });
    expect(runner.release).toHaveBeenCalledWith({ allocation, reason: "restore_failed" });
  });

  test("captures only admitted overlay entries at explicit boundaries", async () => {
    const runner = makeRunner();
    const result = await checkpointCloudComputerWorkspace({
      allocation,
      workspaceRef: "workspace.one",
      generation: 7,
      baseImageDigest: baseImage.digest,
      parentCheckpointRef: "checkpoint.workspace.6",
      boundary: "bounded_interval",
      runner,
    });
    expect(runner.captureOverlay).toHaveBeenCalledWith(
      expect.objectContaining({
        allocation,
        boundary: "bounded_interval",
        include: expect.any(Function),
        preserveWhiteouts: true,
      }),
    );
    expect(result.receipt.includedPaths).toEqual([
      ".git/HEAD",
      ".git/refs/heads/main",
      "src/index.ts",
    ]);
    expect(result.receipt.byteCount).toBe(72);
    expect(result.receipt.durationMs).toBe(18);
    expect(result.artifact.kind).toBe("delta");
  });

  test("rejects a privileged capture runner that returns excluded state", async () => {
    const runner = makeRunner();
    runner.captureOverlay = vi.fn(async () => ({
      contentRef: cloudComputerWorkspaceContentRef("content.workspace.unsafe"),
      contentDigest: sha("9"),
      entries: [entries[3]!],
      deletedPaths: [],
      byteCount: entries[3]!.byteCount,
      durationMs: 1,
    }));
    await expect(
      checkpointCloudComputerWorkspace({
        allocation,
        workspaceRef: "workspace.one",
        generation: 7,
        baseImageDigest: baseImage.digest,
        parentCheckpointRef: null,
        boundary: "explicit",
        runner,
      }),
    ).rejects.toMatchObject({ code: "unsafe_checkpoint_entry" });
  });

  test("rejects storage URLs and local paths as public checkpoint content refs", () => {
    expect(() => cloudComputerWorkspaceContentRef("gs://bucket/private-object")).toThrow(
      CloudComputerWorkspaceError,
    );
    expect(() => cloudComputerWorkspaceContentRef("/var/lib/private-checkpoint")).toThrow(
      CloudComputerWorkspaceError,
    );
  });

  test("local COW adapter mounts fixed argv and sanitizes restored Git metadata", async () => {
    const commands: Array<{ executable: string; args: ReadonlyArray<string> }> = [];
    const madeDirectories: Array<{ path: string; recursive: boolean }> = [];
    const captureAtomically = vi.fn(async () => ({
      contentRef: cloudComputerWorkspaceContentRef("content.workspace.captured"),
      contentDigest: sha("7"),
      entries: entries.slice(0, 3),
      deletedPaths: ["src/deleted.ts"],
      byteCount: 72,
      durationMs: 2,
    }));
    const restoreAtomically = vi.fn(
      async (input: Parameters<LocalCowWorkspacePorts["contents"]["restoreAtomically"]>[0]) => ({
        observedContentDigest: input.expectedContentDigest,
        restoredPaths: input.entries.map((entry) => entry.path),
        bytesRead: 72,
      }),
    );
    const runner = createLocalCowWorkspaceRunner({
      rootPath: "/var/lib/openagents/workspaces",
      ports: {
        filesystem: {
          mkdir: vi.fn(async (path, options) => {
            madeDirectories.push({ path, recursive: options.recursive });
          }),
          removeTree: vi.fn(async () => undefined),
        },
        commands: {
          run: vi.fn(async (input) => {
            commands.push(input);
            return { exitCode: 0 };
          }),
        },
        images: {
          verify: vi.fn(async (image) => ({
            verifiedDigest: image.digest,
            signerIdentity: image.signerIdentity,
            localImagePath: "/var/lib/openagents/images/base.squashfs",
          })),
        },
        contents: {
          restoreAtomically,
          captureAtomically,
        },
        caches: {
          warmReadOnly: vi.fn(async () => ({ hitRefs: [], bytesRead: 0 })),
        },
        benchmark: vi.fn(async () => ({
          dependencyInstallDurationMs: 1,
          largeFileWriteDurationMs: 1,
          largeFileBytes: 1,
        })),
      },
    });
    const localAllocation = await runner.allocate({ workspaceRef: "workspace.one", generation: 7 });
    await runner.verifySignedBaseImage(baseImage);
    await runner.mountBaseImage({ allocation: localAllocation, image: baseImage, readOnly: true });
    await runner.mountOverlay({ allocation: localAllocation, lowerReadOnly: true });
    await runner.restoreEntries({
      allocation: localAllocation,
      contentRef: checkpoint.contentRef,
      expectedContentDigest: checkpoint.contentDigest,
      kind: checkpoint.kind,
      entries: checkpoint.entries,
      deletedPaths: checkpoint.deletedPaths,
    });
    await runner.captureOverlay({
      allocation: localAllocation,
      boundary: "explicit",
      include: () => true,
      preserveWhiteouts: true,
    });

    expect(madeDirectories[1]).toEqual(expect.objectContaining({ recursive: false }));
    expect(commands[0]).toEqual(
      expect.objectContaining({
        executable: "/bin/mount",
        args: expect.arrayContaining(["loop,ro,nodev,nosuid"]),
      }),
    );
    expect(commands).toContainEqual({
      executable: "/usr/bin/git",
      args: ["-C", localAllocation.upperPath, "config", "--local", "core.hooksPath", "/dev/null"],
    });
    expect(commands.flatMap((command) => command.args)).not.toContain(".git/config");
    expect(restoreAtomically).toHaveBeenCalledWith(
      expect.objectContaining({
        destinationPath: localAllocation.upperPath,
        kind: "full",
        deletedPaths: [],
      }),
    );
    expect(captureAtomically).toHaveBeenCalledWith(
      expect.objectContaining({
        sourcePath: localAllocation.upperPath,
        preserveWhiteouts: true,
      }),
    );
  });
});
