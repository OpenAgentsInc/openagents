import {
  FORGE_READ_SCHEMA,
  ForgeRepositoryProjection,
  type ForgeRepositoryProjection as ForgeRepositoryProjectionType,
} from "@/features/forge/repository-read";

export const forgeProjection = (
  overrides: Partial<ForgeRepositoryProjectionType> = {},
): ForgeRepositoryProjectionType =>
  ForgeRepositoryProjection.make({
    schema: FORGE_READ_SCHEMA,
    servedAt: "2026-07-25T22:00:00.000Z",
    repository: {
      repositoryRef: "repo.openagents.omega",
      owner: "OpenAgentsInc",
      name: "omega",
      description: "The OpenAgents coding agent.",
      nip34Coordinate:
        "30617:0326d8f9eb5abea63d9613ac90451dfce62ca2e9855144b5a71d8e8569932974:omega",
      authorityMode: "openagents_git_authoritative",
      defaultBranch: "main",
      canonicalCloneUrl: "https://openagents.com/git/OpenAgentsInc/omega.git",
      publicWebRead: true,
      projectionFreshness: "Updated 18 seconds ago",
      maintainers: [
        {
          displayName: "Private member",
          nostrPubkey: "0326d8f9eb5abea63d9613ac90451dfce62ca2e9855144b5a71d8e8569932974",
        },
      ],
    },
    access: { mode: "member", canWrite: true },
    selectedRef: "refs/heads/main",
    selectedPath: "src/index.ts",
    refs: [
      {
        name: "refs/heads/main",
        objectId: "a".repeat(40),
        kind: "branch",
        isDefault: true,
      },
      {
        name: "refs/tags/v0.1.0",
        objectId: "b".repeat(40),
        kind: "tag",
        isDefault: false,
      },
    ],
    commits: [
      {
        objectId: "a".repeat(40),
        shortId: "aaaaaaaa",
        subject: "Add the repository viewer",
        authorName: "Forge service",
        authoredAt: "2026-07-25T21:30:00.000Z",
        parentIds: ["b".repeat(40)],
        additions: 21,
        deletions: 3,
        changedFiles: 2,
      },
    ],
    tree: [
      {
        name: "components",
        path: "src/components",
        kind: "directory",
        size: 0,
        objectId: "c".repeat(40),
      },
      {
        name: "index.ts",
        path: "src/index.ts",
        kind: "file",
        size: 38,
        objectId: "d".repeat(40),
      },
    ],
    readme: {
      _tag: "markdown",
      path: "README.md",
      objectId: "e".repeat(40),
      byteSize: 24,
      content: "# Omega\n\nOwned Git.",
    },
    file: {
      _tag: "text",
      path: "src/index.ts",
      objectId: "d".repeat(40),
      byteSize: 38,
      language: "typescript",
      content: "export const forge = true\n",
      highlightedLines: [
        [
          { content: "export", color: "#A0A0A0" },
          { content: " const forge = true", color: "#FFF" },
        ],
        [{ content: "" }],
      ],
    },
    commit: {
      objectId: "a".repeat(40),
      shortId: "aaaaaaaa",
      subject: "Add the repository viewer",
      body: "Use the owned Forge read service.",
      authorName: "Forge service",
      authoredAt: "2026-07-25T21:30:00.000Z",
      parentIds: ["b".repeat(40)],
      additions: 21,
      deletions: 3,
      changedFiles: 2,
    },
    diff: {
      baseObjectId: "b".repeat(40),
      headObjectId: "a".repeat(40),
      unified:
        "diff --git a/src/index.ts b/src/index.ts\n@@ -1,1 +1,2 @@\n export const old = true\n+export const forge = true",
      truncated: false,
    },
    ...overrides,
  });
