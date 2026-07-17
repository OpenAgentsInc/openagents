import { createHash } from "node:crypto";
import { lstatSync, readFileSync, realpathSync } from "node:fs";
import { relative, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { Effect } from "effect";
import type {
  ArtifactSourceInventory,
  AuthorityEntry,
  ContentEntry,
  EvidenceConfidence,
  GitSourceInventory,
  LicenseStatus,
  Sha256Digest,
  SourceProvenance,
  TargetInventory,
  Visibility,
} from "./manifest.ts";

export class FastFollowInventoryError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "FastFollowInventoryError";
  }
}

const digest = (bytes: Uint8Array): Sha256Digest =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const git = (root: string, ...arguments_: string[]): string => {
  const result = spawnSync("git", ["-c", "core.hooksPath=/dev/null", ...arguments_], {
    cwd: root,
    encoding: "utf8",
    stdio: "pipe",
  });
  if (result.status !== 0)
    throw new FastFollowInventoryError(
      "git_inventory_failed",
      result.stderr.trim() || "Git inventory failed",
    );
  return result.stdout.trim();
};
const gitBytes = (root: string, ...arguments_: string[]): Uint8Array => {
  const result = spawnSync("git", ["-c", "core.hooksPath=/dev/null", ...arguments_], {
    cwd: root,
    encoding: "buffer",
    stdio: "pipe",
  });
  if (result.status !== 0)
    throw new FastFollowInventoryError(
      "git_inventory_failed",
      result.stderr.toString("utf8").trim() || "Git inventory failed",
    );
  return result.stdout;
};
const exactGitIdentity = (root: string): { commit: string; tree: string } => ({
  commit: git(root, "rev-parse", "--verify", "HEAD^{commit}"),
  tree: git(root, "rev-parse", "--verify", "HEAD^{tree}"),
});
const containedFile = (root: string, path: string): { absolute: string; relative: string } => {
  const canonicalRoot = realpathSync(root);
  const absolute = realpathSync(resolve(canonicalRoot, path));
  if (absolute !== canonicalRoot && !absolute.startsWith(canonicalRoot + sep))
    throw new FastFollowInventoryError(
      "inventory_path_escape",
      `path escapes inventory root: ${path}`,
    );
  if (!lstatSync(absolute).isFile())
    throw new FastFollowInventoryError(
      "inventory_not_file",
      `inventory path is not a file: ${path}`,
    );
  return { absolute, relative: relative(canonicalRoot, absolute).replaceAll("\\", "/") };
};
const contentEntry = (root: string, path: string): ContentEntry => {
  const file = containedFile(root, path);
  const bytes = readFileSync(file.absolute);
  return { path: file.relative, byte_size: bytes.byteLength, content_digest: digest(bytes) };
};
const contentEntryAtHead = (root: string, path: string): ContentEntry => {
  const canonicalRoot = realpathSync(root);
  const requested = relative(canonicalRoot, resolve(canonicalRoot, path)).replaceAll("\\", "/");
  if (!requested || requested.startsWith("../") || requested === "..")
    throw new FastFollowInventoryError(
      "inventory_path_escape",
      `path escapes inventory root: ${path}`,
    );
  const row = git(canonicalRoot, "ls-tree", "HEAD", "--", requested).split(/\r?\n/)[0];
  if (!row)
    throw new FastFollowInventoryError(
      "inventory_not_in_snapshot",
      `path is absent from HEAD: ${path}`,
    );
  const mode = row.split(/\s+/, 1)[0];
  if (mode === "120000")
    throw new FastFollowInventoryError(
      "inventory_symlink_refused",
      `snapshot path is a symlink: ${path}`,
    );
  if (mode !== "040000") {
    const bytes = gitBytes(canonicalRoot, "show", `HEAD:${requested}`);
    return { path: requested, byte_size: bytes.byteLength, content_digest: digest(bytes) };
  }
  const files = git(canonicalRoot, "ls-tree", "-r", "--name-only", "HEAD", "--", requested)
    .split(/\r?\n/)
    .filter(Boolean)
    .toSorted();
  const entries = files.map((file) => {
    const bytes = gitBytes(canonicalRoot, "show", `HEAD:${file}`);
    return { path: file, byte_size: bytes.byteLength, content_digest: digest(bytes) };
  });
  return {
    path: requested,
    byte_size: entries.reduce((total, entry) => total + entry.byte_size, 0),
    content_digest: digest(Buffer.from(JSON.stringify(entries))),
  };
};
const attempt = <A>(operation: () => A): Effect.Effect<A, FastFollowInventoryError> =>
  Effect.try({
    try: operation,
    catch: (error) =>
      error instanceof FastFollowInventoryError
        ? error
        : new FastFollowInventoryError(
            "inventory_io_failed",
            error instanceof Error ? error.message : String(error),
          ),
  });

export interface InventoryTargetInput {
  readonly root: string;
  readonly repository: string;
  readonly authorities: ReadonlyArray<{
    readonly path: string;
    readonly authority_kinds: AuthorityEntry["authority_kinds"];
  }>;
}

export const inventoryTarget = (
  input: InventoryTargetInput,
): Effect.Effect<TargetInventory, FastFollowInventoryError> =>
  attempt(() => {
    const identity = exactGitIdentity(input.root);
    return {
      repository: input.repository,
      ...identity,
      authorities: input.authorities.map((authority) => ({
        ...contentEntryAtHead(input.root, authority.path),
        authority_kinds: authority.authority_kinds,
      })),
    };
  });

export interface InventoryGitSourceInput {
  readonly root: string;
  readonly source_id: string;
  readonly canonical_ref: string;
  readonly selected_paths: ReadonlyArray<string>;
  readonly visibility: Visibility;
  readonly provenance: SourceProvenance;
}

export const inventoryPublicGitSource = (
  input: InventoryGitSourceInput,
): Effect.Effect<GitSourceInventory, FastFollowInventoryError> =>
  attempt(() => ({
    source_kind: "public_git",
    source_id: input.source_id,
    canonical_ref: input.canonical_ref,
    ...exactGitIdentity(input.root),
    visibility: input.visibility,
    evidence_confidence: "verified_bytes",
    provenance: input.provenance,
    selected_corpus: input.selected_paths.map((path) => contentEntryAtHead(input.root, path)),
  }));

export interface InventoryArtifactSourceInput {
  readonly artifact_path: string;
  readonly source_id: string;
  readonly canonical_ref: string;
  readonly release_identity: string;
  readonly selected_paths?: ReadonlyArray<{ readonly root: string; readonly path: string }>;
  readonly visibility: Visibility;
  readonly evidence_confidence: Exclude<EvidenceConfidence, "verified_bytes"> | "verified_bytes";
  readonly provenance: {
    readonly origin: string;
    readonly license: LicenseStatus;
    readonly license_ref?: string;
  };
}

export const inventoryArtifactSource = (
  input: InventoryArtifactSourceInput,
): Effect.Effect<ArtifactSourceInventory, FastFollowInventoryError> =>
  attempt(() => {
    if (input.release_identity.trim().length === 0)
      throw new FastFollowInventoryError(
        "artifact_release_identity_missing",
        "artifact label alone is not an exact release identity",
      );
    const artifactBytes = readFileSync(realpathSync(input.artifact_path));
    return {
      source_kind: "artifact",
      source_id: input.source_id,
      canonical_ref: input.canonical_ref,
      artifact_digest: digest(artifactBytes),
      release_identity: input.release_identity,
      visibility: input.visibility,
      evidence_confidence: input.evidence_confidence,
      provenance: input.provenance,
      selected_corpus: (input.selected_paths ?? []).map((item) =>
        contentEntry(item.root, item.path),
      ),
    };
  });
