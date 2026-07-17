import { createHash } from "node:crypto";
import type { FastFollowDocument } from "./index.ts";
import { computeDocumentDigest, computeIntentDigest, stableJson } from "./index.ts";

export const FAST_FOLLOW_MANIFEST_FORMAT_VERSION = "0.1" as const;
export const FAST_FOLLOW_COMPILER_VERSION = "0.1.0" as const;

export type Sha256Digest = `sha256:${string}`;
export type EvidenceConfidence = "verified_bytes" | "observed_artifact" | "inferred_bundle";
export type Visibility = "public" | "target_private";
export type LicenseStatus = "known_permissive" | "known_restricted" | "unknown";

export interface ContentEntry {
  readonly path: string;
  readonly byte_size: number;
  readonly content_digest: Sha256Digest;
}

export interface AuthorityEntry extends ContentEntry {
  readonly authority_kinds: ReadonlyArray<
    "agent_instructions" | "invariants" | "product_spec" | "assurance_spec" | "roadmap"
  >;
}

export interface TargetInventory {
  readonly repository: string;
  readonly commit: string;
  readonly tree: string;
  readonly authorities: ReadonlyArray<AuthorityEntry>;
}

export interface SourceProvenance {
  readonly origin: string;
  readonly license: LicenseStatus;
  readonly license_ref?: string;
}

interface SourceInventoryBase {
  readonly source_id: string;
  readonly canonical_ref: string;
  readonly visibility: Visibility;
  readonly evidence_confidence: EvidenceConfidence;
  readonly provenance: SourceProvenance;
  readonly selected_corpus: ReadonlyArray<ContentEntry>;
}

export interface GitSourceInventory extends SourceInventoryBase {
  readonly source_kind: "public_git";
  readonly commit: string;
  readonly tree: string;
}

export interface ArtifactSourceInventory extends SourceInventoryBase {
  readonly source_kind: "artifact";
  readonly artifact_digest: Sha256Digest;
  readonly release_identity: string;
}

export type SourceInventory = GitSourceInventory | ArtifactSourceInventory;

export interface FastFollowWorkUnit {
  readonly unit_ref: string;
  readonly directive_ref: string;
  readonly stage: "research" | "gap_analysis" | "candidate_proposal";
  readonly source_refs: ReadonlyArray<string>;
  readonly target_scopes: ReadonlyArray<string>;
  readonly work_products: ReadonlyArray<string>;
  readonly source_code_copying:
    | "allowed_by_declared_provenance"
    | "denied_license_unknown_or_restricted";
  readonly authority: "evidence_only";
  readonly dedupe_identity: Sha256Digest;
}

export interface FastFollowManifestPayload {
  readonly fast_follow_manifest_format_version: typeof FAST_FOLLOW_MANIFEST_FORMAT_VERSION;
  readonly do_not_edit: true;
  readonly compiler: {
    readonly version: typeof FAST_FOLLOW_COMPILER_VERSION;
    readonly content_digest: Sha256Digest;
  };
  readonly spec: {
    readonly path: string;
    readonly id: string;
    readonly revision: number;
    readonly document_digest: Sha256Digest;
    readonly intent_digest: Sha256Digest;
  };
  readonly target: TargetInventory;
  readonly sources: ReadonlyArray<SourceInventory & { readonly copying_allowed: boolean }>;
  readonly directive_graph: ReadonlyArray<{
    readonly directive_ref: string;
    readonly priority: number;
    readonly source_refs: ReadonlyArray<string>;
    readonly target_scopes: ReadonlyArray<string>;
  }>;
  readonly work_units: ReadonlyArray<FastFollowWorkUnit>;
  readonly classification: {
    readonly manifest_visibility: Visibility;
    readonly external_instructions: "untrusted_study_data";
    readonly grants_runtime_authority: false;
  };
}

export interface FastFollowManifest extends FastFollowManifestPayload {
  readonly manifest_content_digest: Sha256Digest;
}

export interface CompileFastFollowManifestInput {
  readonly document: FastFollowDocument;
  readonly spec_path: string;
  readonly compiler_content_digest: Sha256Digest;
  readonly target: TargetInventory;
  readonly sources: ReadonlyArray<SourceInventory>;
}

export class FastFollowCompileError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "FastFollowCompileError";
  }
}

const sha256 = (value: string): Sha256Digest =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;
const fail = (code: string, message: string): never => {
  throw new FastFollowCompileError(code, message);
};
const isExactGitObject = (value: string): boolean => /^[a-f0-9]{40,64}$/.test(value);
const isDigest = (value: string): value is Sha256Digest => /^sha256:[a-f0-9]{64}$/.test(value);
const isContainedPath = (value: string): boolean =>
  value.length > 0 &&
  !value.startsWith("/") &&
  !/^[A-Za-z]:[\\/]/.test(value) &&
  !value.replaceAll("\\", "/").split("/").includes("..");

const requiredAuthorities = (
  document: FastFollowDocument,
): Map<string, Array<AuthorityEntry["authority_kinds"][number]>> => {
  const target = document.blocks.target as Record<string, unknown>;
  const result = new Map<string, Array<AuthorityEntry["authority_kinds"][number]>>();
  const add = (field: string, kind: AuthorityEntry["authority_kinds"][number]) => {
    for (const path of target[field] as string[])
      result.set(path, [...(result.get(path) ?? []), kind]);
  };
  add("agent_instructions", "agent_instructions");
  add("invariants", "invariants");
  add("product_specs", "product_spec");
  add("assurance_specs", "assurance_spec");
  add("roadmap_authorities", "roadmap");
  return result;
};

const validateContent = (entries: ReadonlyArray<ContentEntry>, code: string): void => {
  const paths = new Set<string>();
  for (const entry of entries) {
    if (!isContainedPath(entry.path))
      fail("manifest_path_escape", `uncontained inventory path: ${entry.path}`);
    if (
      !isDigest(entry.content_digest) ||
      !Number.isInteger(entry.byte_size) ||
      entry.byte_size < 0
    )
      fail(code, `invalid content identity for ${entry.path}`);
    if (paths.has(entry.path)) fail(code, `duplicate content path: ${entry.path}`);
    paths.add(entry.path);
  }
};

const sourceExactIdentity = (source: SourceInventory): string =>
  source.source_kind === "public_git"
    ? `${source.commit}:${source.tree}`
    : `${source.release_identity}:${source.artifact_digest}`;

export const compileFastFollowManifest = (
  input: CompileFastFollowManifestInput,
): {
  readonly manifest: FastFollowManifest;
  readonly bytes: string;
  readonly digest: Sha256Digest;
} => {
  if (!isDigest(input.compiler_content_digest))
    fail("compiler_identity_invalid", "compiler content digest must be an exact SHA-256 identity");
  if (!isContainedPath(input.spec_path))
    fail("manifest_path_escape", "spec path must be repository-relative");
  if (!isExactGitObject(input.target.commit) || !isExactGitObject(input.target.tree))
    fail(
      "target_identity_not_exact",
      "target inventory requires exact commit and tree object identities",
    );

  const targetBlock = input.document.blocks.target as Record<string, unknown>;
  if (input.target.repository !== targetBlock.repository)
    fail("target_repository_mismatch", "target inventory does not match the authored repository");
  validateContent(input.target.authorities, "target_authority_invalid");
  const authorityRequirements = requiredAuthorities(input.document);
  const inventoryAuthorities = new Map(
    input.target.authorities.map((entry) => [entry.path, entry]),
  );
  for (const [path, kinds] of authorityRequirements) {
    const entry = inventoryAuthorities.get(path);
    if (!entry) fail("target_authority_missing", `target authority is not inventoried: ${path}`);
    if (entry && kinds.some((kind) => !entry.authority_kinds.includes(kind)))
      fail("target_authority_kind_mismatch", `target authority kind does not match: ${path}`);
  }

  const authoredSources = input.document.blocks.sources as Array<Record<string, unknown>>;
  const inventories = new Map<string, SourceInventory>();
  for (const source of input.sources) {
    if (inventories.has(source.source_id))
      fail("source_inventory_duplicate", `duplicate source inventory: ${source.source_id}`);
    inventories.set(source.source_id, source);
  }
  const orderedSources = authoredSources.map((authored) => {
    const id = String(authored.id);
    const source = inventories.get(id);
    if (!source) return fail("source_inventory_missing", `source inventory is missing: ${id}`);
    if (source.canonical_ref !== authored.canonical_ref)
      fail("source_canonical_ref_mismatch", `source canonical ref does not match: ${id}`);
    validateContent(source.selected_corpus, "source_corpus_invalid");
    if (source.source_kind === "public_git") {
      if (!isExactGitObject(source.commit) || !isExactGitObject(source.tree))
        fail("source_identity_not_exact", `Git source requires exact commit and tree: ${id}`);
      if (authored.access !== "public_source")
        fail("source_kind_mismatch", `only public_source may use a public Git inventory: ${id}`);
    } else {
      if (!isDigest(source.artifact_digest) || source.release_identity.trim().length === 0)
        fail(
          "source_identity_not_exact",
          `artifact source requires digest and release identity: ${id}`,
        );
      if (source.evidence_confidence === "verified_bytes" && source.selected_corpus.length === 0)
        fail(
          "source_confidence_invalid",
          `verified artifact confidence requires selected bytes: ${id}`,
        );
    }
    const copyingAllowed = source.provenance.license === "known_permissive";
    return {
      ...source,
      selected_corpus: [...source.selected_corpus].toSorted((a, b) => a.path.localeCompare(b.path)),
      copying_allowed: copyingAllowed,
    };
  });
  if (inventories.size !== authoredSources.length)
    fail(
      "source_inventory_unknown",
      "source inventories contain an ID absent from the authored spec",
    );

  const directives = input.document.blocks.directives as Array<Record<string, unknown>>;
  const workGeneration = input.document.blocks.work_generation as Record<string, unknown>;
  const initial = workGeneration.initial_program as Record<string, unknown> | undefined;
  const defaultStage = String(initial?.default_stage ?? "research");
  const stage = (["research", "gap_analysis", "candidate_proposal"] as const).includes(
    defaultStage as never,
  )
    ? (defaultStage as FastFollowWorkUnit["stage"])
    : "research";
  const targetIdentity = `${input.target.commit}:${input.target.tree}`;
  const sourceIdentities = Object.fromEntries(
    orderedSources.map((source) => [source.source_id, sourceExactIdentity(source)]),
  );
  const workUnits = directives.map((directive) => {
    const sourceIds = (directive.source_refs as string[]).map((ref) => ref.split("#", 1)[0]!);
    const sourceCodeCopying = sourceIds.every(
      (id) => orderedSources.find((source) => source.source_id === id)?.copying_allowed,
    )
      ? ("allowed_by_declared_provenance" as const)
      : ("denied_license_unknown_or_restricted" as const);
    const dedupeIdentity = sha256(
      stableJson({
        intent_digest: computeIntentDigest(input.document),
        target: targetIdentity,
        sources: sourceIds.map((id) => [id, sourceIdentities[id]]),
        directive: directive.id,
        stage,
      }),
    );
    return {
      unit_ref: `fast_follow.${String(directive.id)}.${dedupeIdentity.slice(7, 23)}`,
      directive_ref: String(directive.id),
      stage,
      source_refs: [...(directive.source_refs as string[])],
      target_scopes: [...(directive.target_scopes as string[])],
      work_products: [...(directive.work_products as string[])],
      source_code_copying: sourceCodeCopying,
      authority: "evidence_only" as const,
      dedupe_identity: dedupeIdentity,
    };
  });
  const manifestVisibility: Visibility = orderedSources.some(
    (source) => source.visibility === "target_private",
  )
    ? "target_private"
    : "public";
  const payload: FastFollowManifestPayload = {
    fast_follow_manifest_format_version: FAST_FOLLOW_MANIFEST_FORMAT_VERSION,
    do_not_edit: true,
    compiler: {
      version: FAST_FOLLOW_COMPILER_VERSION,
      content_digest: input.compiler_content_digest,
    },
    spec: {
      path: input.spec_path,
      id: String(input.document.frontmatter.fast_follow_spec_id),
      revision: Number(input.document.frontmatter.fast_follow_revision),
      document_digest: `sha256:${computeDocumentDigest(input.document.source)}`,
      intent_digest: `sha256:${computeIntentDigest(input.document)}`,
    },
    target: {
      ...input.target,
      authorities: [...input.target.authorities].toSorted((a, b) => a.path.localeCompare(b.path)),
    },
    sources: orderedSources,
    directive_graph: directives.map((directive) => ({
      directive_ref: String(directive.id),
      priority: Number(directive.priority),
      source_refs: [...(directive.source_refs as string[])],
      target_scopes: [...(directive.target_scopes as string[])],
    })),
    work_units: workUnits,
    classification: {
      manifest_visibility: manifestVisibility,
      external_instructions: "untrusted_study_data",
      grants_runtime_authority: false,
    },
  };
  const digest = sha256(stableJson(payload));
  const manifest: FastFollowManifest = { ...payload, manifest_content_digest: digest };
  return { manifest, bytes: `${stableJson(manifest)}\n`, digest };
};

export interface StaleReason {
  readonly code:
    | "target_commit_changed"
    | "target_tree_changed"
    | "target_authority_changed"
    | "source_identity_changed"
    | "source_corpus_changed";
  readonly ref: string;
  readonly expected: string;
  readonly observed: string;
}

export type ManifestFreshness =
  | { readonly state: "fresh"; readonly manifest_digest: Sha256Digest }
  | {
      readonly state: "stale";
      readonly manifest_digest: Sha256Digest;
      readonly reasons: ReadonlyArray<StaleReason>;
    };

export const checkManifestFreshness = (
  manifest: FastFollowManifest,
  current: { readonly target: TargetInventory; readonly sources: ReadonlyArray<SourceInventory> },
): ManifestFreshness => {
  const reasons: StaleReason[] = [];
  const add = (code: StaleReason["code"], ref: string, expected: string, observed: string) => {
    if (expected !== observed) reasons.push({ code, ref, expected, observed });
  };
  add(
    "target_commit_changed",
    manifest.target.repository,
    manifest.target.commit,
    current.target.commit,
  );
  add("target_tree_changed", manifest.target.repository, manifest.target.tree, current.target.tree);
  const currentAuthorities = new Map(
    current.target.authorities.map((entry) => [entry.path, entry.content_digest]),
  );
  for (const authority of manifest.target.authorities)
    add(
      "target_authority_changed",
      authority.path,
      authority.content_digest,
      currentAuthorities.get(authority.path) ?? "missing",
    );
  const currentSources = new Map(current.sources.map((source) => [source.source_id, source]));
  for (const source of manifest.sources) {
    const observed = currentSources.get(source.source_id);
    if (!observed) {
      reasons.push({
        code: "source_identity_changed",
        ref: source.source_id,
        expected: sourceExactIdentity(source),
        observed: "missing",
      });
      continue;
    }
    add(
      "source_identity_changed",
      source.source_id,
      sourceExactIdentity(source),
      sourceExactIdentity(observed),
    );
    add(
      "source_corpus_changed",
      source.source_id,
      stableJson([...source.selected_corpus].toSorted((a, b) => a.path.localeCompare(b.path))),
      stableJson([...observed.selected_corpus].toSorted((a, b) => a.path.localeCompare(b.path))),
    );
  }
  return reasons.length === 0
    ? { state: "fresh", manifest_digest: manifest.manifest_content_digest }
    : { state: "stale", manifest_digest: manifest.manifest_content_digest, reasons };
};
