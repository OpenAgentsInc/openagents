import { lstat, readdir, readFile } from "node:fs/promises";
import { relative, resolve, sep } from "node:path";
import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  OPENAGENTS_STUDYBENCH_EVIDENCE_SPAN_SCHEMA_REF,
  OpenAgentsStudybenchEvidenceSpan,
  OpenAgentsStudybenchVisibility,
} from "./studybench";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

export const OPENAGENTS_REPO_CORPUS_ENTRY_SCHEMA_REF = "openagents.repo_corpus_entry.v0" as const;
export const OPENAGENTS_REPO_CORPUS_MANIFEST_SCHEMA_REF = "openagents.repo_corpus_manifest.v0" as const;
export const OPENAGENTS_REPO_CORPUS_EVIDENCE_SPAN_SCHEMA_REF =
  "openagents.repo_corpus_evidence_span.v0" as const;

export const OpenAgentsRepoCorpusEntry = S.Struct({
  byteSize: S.Number,
  kind: S.String,
  path: S.String,
  schemaRef: S.Literal(OPENAGENTS_REPO_CORPUS_ENTRY_SCHEMA_REF),
  sha256: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  visibility: OpenAgentsStudybenchVisibility,
});
export type OpenAgentsRepoCorpusEntry = typeof OpenAgentsRepoCorpusEntry.Type;

export const OpenAgentsRepoCorpusManifest = S.Struct({
  commit: S.String,
  defaultVisibility: OpenAgentsStudybenchVisibility,
  entries: S.Array(OpenAgentsRepoCorpusEntry),
  excludedPathPatterns: S.Array(S.String),
  generatedAt: S.String,
  manifestHash: S.String,
  manifestRef: S.String,
  repo: S.String,
  schemaRef: S.Literal(OPENAGENTS_REPO_CORPUS_MANIFEST_SCHEMA_REF),
  sourceBoundary: S.Literal("public_refs_only"),
});
export type OpenAgentsRepoCorpusManifest = typeof OpenAgentsRepoCorpusManifest.Type;

export const OpenAgentsRepoCorpusEvidenceSpan = S.Struct({
  corpusRef: S.String,
  evidence: OpenAgentsStudybenchEvidenceSpan,
  schemaRef: S.Literal(OPENAGENTS_REPO_CORPUS_EVIDENCE_SPAN_SCHEMA_REF),
  spanHash: S.String,
});
export type OpenAgentsRepoCorpusEvidenceSpan = typeof OpenAgentsRepoCorpusEvidenceSpan.Type;

export interface BuildOpenAgentsRepoCorpusManifestInput {
  readonly commit: string;
  readonly defaultSourceAuthorityRefs?: ReadonlyArray<string>;
  readonly defaultVisibility?: typeof OpenAgentsStudybenchVisibility.Type;
  readonly generatedAt?: string;
  readonly manifestRef?: string;
  readonly repo: string;
  readonly rootDir: string;
}

export interface ExtractOpenAgentsRepoCorpusEvidenceSpanInput {
  readonly endLine: number;
  readonly manifest: OpenAgentsRepoCorpusManifest;
  readonly path: string;
  readonly rootDir: string;
  readonly spanId?: string;
  readonly startLine: number;
}

const DEFAULT_EXCLUDED_PATH_PATTERNS = [
  ".git/",
  ".claude/",
  ".git-worktrees/",
  ".pylon-local/",
  ".secrets/",
  ".cache/",
  "node_modules/",
  "dist/",
  "target/",
  "coverage/",
  "tmp/",
  "*.dmg",
  "*.log",
  "*.tar",
  "*.tgz",
  "*.tmp",
  "*.zip",
  "public-unsafe-name/",
  "*.study-artifact-index.json",
] as const;

const DEFAULT_SOURCE_AUTHORITY_REFS = ["authority.openagents.repo_corpus.public_source"];

export function buildOpenAgentsRepoCorpusManifest(
  input: BuildOpenAgentsRepoCorpusManifestInput,
): Effect.Effect<OpenAgentsRepoCorpusManifest, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const rootDir = resolve(input.rootDir);
    const defaultVisibility = input.defaultVisibility ?? "openagents_public_retained";
    const defaultSourceAuthorityRefs = input.defaultSourceAuthorityRefs ?? DEFAULT_SOURCE_AUTHORITY_REFS;
    const entries = yield* collectRepoCorpusEntries(rootDir, rootDir, defaultVisibility, defaultSourceAuthorityRefs);
    const baseManifest: OpenAgentsRepoCorpusManifest = {
      commit: input.commit,
      defaultVisibility,
      entries: entries.sort((left, right) => left.path.localeCompare(right.path)),
      excludedPathPatterns: [...DEFAULT_EXCLUDED_PATH_PATTERNS],
      generatedAt: input.generatedAt ?? "generated_at.withheld_for_stable_manifest_hash",
      manifestHash: "sha256:pending",
      manifestRef: "openagents_repo_corpus_manifest.pending",
      repo: input.repo,
      schemaRef: OPENAGENTS_REPO_CORPUS_MANIFEST_SCHEMA_REF,
      sourceBoundary: "public_refs_only",
    };
    const manifestHash = openAgentsRepoCorpusManifestHash(baseManifest);
    const manifest: OpenAgentsRepoCorpusManifest = {
      ...baseManifest,
      manifestHash,
      manifestRef: input.manifestRef ?? `openagents_repo_corpus_manifest.${shortHash(manifestHash)}`,
    };

    return yield* decodeOpenAgentsRepoCorpusManifest(manifest);
  });
}

export function extractOpenAgentsRepoCorpusEvidenceSpan(
  input: ExtractOpenAgentsRepoCorpusEvidenceSpanInput,
): Effect.Effect<OpenAgentsRepoCorpusEvidenceSpan, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const manifestEntry = input.manifest.entries.find((entry) => entry.path === normalizeRepoPath(input.path));

    if (manifestEntry === undefined) {
      return yield* repoCorpusError("repoCorpusEvidenceSpan.path", "path is not admitted by the corpus manifest");
    }

    if (!Number.isInteger(input.startLine) || !Number.isInteger(input.endLine) || input.startLine < 1) {
      return yield* repoCorpusError("repoCorpusEvidenceSpan.startLine", "line range must use positive integer lines");
    }

    if (input.endLine < input.startLine) {
      return yield* repoCorpusError("repoCorpusEvidenceSpan.endLine", "must be greater than or equal to startLine");
    }

    const absolutePath = resolve(input.rootDir, manifestEntry.path);
    const bytes = yield* readFileBytes(absolutePath, "repoCorpusEvidenceSpan.path");
    const text = new TextDecoder().decode(bytes).replaceAll("\r\n", "\n");
    const lines = text.split("\n");

    if (input.endLine > lines.length) {
      return yield* repoCorpusError("repoCorpusEvidenceSpan.endLine", "line range exceeds file length");
    }

    const excerpt = lines
      .slice(input.startLine - 1, input.endLine)
      .map((line, index) => `${String(input.startLine + index).padStart(4, "0")}: ${line}`)
      .join("\n");
    const evidence: typeof OpenAgentsStudybenchEvidenceSpan.Type = {
      end_line: input.endLine,
      excerpt,
      path: manifestEntry.path,
      schemaRef: OPENAGENTS_STUDYBENCH_EVIDENCE_SPAN_SCHEMA_REF,
      span_id: input.spanId ?? `span.${shortHash(`${input.manifest.manifestHash}:${manifestEntry.path}:${input.startLine}:${input.endLine}`)}`,
      start_line: input.startLine,
    };
    const spanHash = openAgentsRepoCorpusEvidenceSpanHash(input.manifest.manifestRef, evidence);

    return yield* decodeOpenAgentsRepoCorpusEvidenceSpan({
      corpusRef: input.manifest.manifestRef,
      evidence,
      schemaRef: OPENAGENTS_REPO_CORPUS_EVIDENCE_SPAN_SCHEMA_REF,
      spanHash,
    });
  });
}

export function decodeOpenAgentsRepoCorpusManifest(
  value: unknown,
): Effect.Effect<OpenAgentsRepoCorpusManifest, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "repoCorpusManifest");
    const manifest = yield* decodeRepoCorpusSchema(OpenAgentsRepoCorpusManifest, value, "repoCorpusManifest");
    yield* validateOpenAgentsRepoCorpusManifest(manifest);
    return manifest;
  });
}

export function decodeOpenAgentsRepoCorpusEvidenceSpan(
  value: unknown,
): Effect.Effect<OpenAgentsRepoCorpusEvidenceSpan, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "repoCorpusEvidenceSpan");
    const span = yield* decodeRepoCorpusSchema(OpenAgentsRepoCorpusEvidenceSpan, value, "repoCorpusEvidenceSpan");
    yield* validateOpenAgentsRepoCorpusEvidenceSpan(span);
    return span;
  });
}

export function openAgentsRepoCorpusManifestHash(manifest: OpenAgentsRepoCorpusManifest): string {
  const { generatedAt: _generatedAt, manifestHash: _manifestHash, manifestRef: _manifestRef, ...stable } = manifest;
  return sha256Ref(stableJson(stable));
}

// A commit-INDEPENDENT digest over the admitted file content (entries + exclusion
// rules + visibility/repo), excluding `commit` and the derived/timestamp fields.
// `manifestHash` embeds the HEAD `commit`, so it changes on every commit even when
// no admitted file changed. This content hash is stable across pure commit drift
// and changes only when an admitted file's content actually changes — the basis
// for the SA-4 standing-freshness "content drift" signal.
export function openAgentsRepoCorpusContentHash(manifest: OpenAgentsRepoCorpusManifest): string {
  const {
    commit: _commit,
    generatedAt: _generatedAt,
    manifestHash: _manifestHash,
    manifestRef: _manifestRef,
    ...stable
  } = manifest;
  return sha256Ref(stableJson(stable));
}

export function openAgentsRepoCorpusEvidenceSpanHash(
  corpusRef: string,
  evidence: typeof OpenAgentsStudybenchEvidenceSpan.Type,
): string {
  return sha256Ref(stableJson({ corpusRef, evidence }));
}

function validateOpenAgentsRepoCorpusManifest(
  manifest: OpenAgentsRepoCorpusManifest,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(manifest.repo, "repoCorpusManifest.repo");
    yield* requireNonEmpty(manifest.commit, "repoCorpusManifest.commit");
    yield* requireNonEmpty(manifest.manifestRef, "repoCorpusManifest.manifestRef");

    if (manifest.entries.length === 0) {
      return yield* repoCorpusError("repoCorpusManifest.entries", "must include at least one admitted file");
    }

    if (manifest.manifestHash !== openAgentsRepoCorpusManifestHash(manifest)) {
      return yield* repoCorpusError("repoCorpusManifest.manifestHash", "must match deterministic manifest content hash");
    }

    const paths = new Set<string>();

    for (const [index, entry] of manifest.entries.entries()) {
      const path = `repoCorpusManifest.entries[${index}]`;
      yield* requireNonEmpty(entry.path, `${path}.path`);
      yield* requireNonEmpty(entry.kind, `${path}.kind`);
      yield* requireNonEmptyRefs(entry.sourceAuthorityRefs, `${path}.sourceAuthorityRefs`);

      if (paths.has(entry.path)) {
        return yield* repoCorpusError(`${path}.path`, "must be unique in the manifest");
      }

      paths.add(entry.path);

      if (pathIsExcluded(entry.path)) {
        return yield* repoCorpusError(`${path}.path`, "must not match an excluded corpus path");
      }

      if (!Number.isInteger(entry.byteSize) || entry.byteSize < 0) {
        return yield* repoCorpusError(`${path}.byteSize`, "must be a non-negative integer byte size");
      }

      if (!entry.sha256.startsWith("sha256:")) {
        return yield* repoCorpusError(`${path}.sha256`, "must be a sha256 content hash ref");
      }
    }
  });
}

function validateOpenAgentsRepoCorpusEvidenceSpan(
  span: OpenAgentsRepoCorpusEvidenceSpan,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(span.corpusRef, "repoCorpusEvidenceSpan.corpusRef");
    yield* requireNonEmpty(span.evidence.span_id, "repoCorpusEvidenceSpan.evidence.span_id");
    yield* requireNonEmpty(span.evidence.path, "repoCorpusEvidenceSpan.evidence.path");
    yield* requireNonEmpty(span.evidence.excerpt, "repoCorpusEvidenceSpan.evidence.excerpt");

    if (span.spanHash !== openAgentsRepoCorpusEvidenceSpanHash(span.corpusRef, span.evidence)) {
      return yield* repoCorpusError("repoCorpusEvidenceSpan.spanHash", "must match deterministic evidence span hash");
    }
  });
}

function collectRepoCorpusEntries(
  rootDir: string,
  currentDir: string,
  visibility: typeof OpenAgentsStudybenchVisibility.Type,
  sourceAuthorityRefs: ReadonlyArray<string>,
): Effect.Effect<ReadonlyArray<OpenAgentsRepoCorpusEntry>, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    const dirents = yield* readDirectory(currentDir);
    const entries: OpenAgentsRepoCorpusEntry[] = [];

    for (const dirent of dirents) {
      const absolutePath = resolve(currentDir, dirent);
      const relativePath = normalizeRepoPath(relative(rootDir, absolutePath));

      if (pathIsExcluded(relativePath)) {
        continue;
      }

      const stats = yield* statPath(absolutePath, relativePath);

      if (stats.isSymbolicLink()) {
        continue;
      }

      if (stats.isDirectory()) {
        const childEntries = yield* collectRepoCorpusEntries(rootDir, absolutePath, visibility, sourceAuthorityRefs);
        entries.push(...childEntries);
        continue;
      }

      if (!stats.isFile()) {
        continue;
      }

      const bytes = yield* readFileBytes(absolutePath, relativePath);
      entries.push({
        byteSize: bytes.byteLength,
        kind: repoCorpusKindForPath(relativePath),
        path: relativePath,
        schemaRef: OPENAGENTS_REPO_CORPUS_ENTRY_SCHEMA_REF,
        sha256: sha256Ref(bytes),
        sourceAuthorityRefs: [...sourceAuthorityRefs],
        visibility,
      });
    }

    return entries;
  });
}

function readDirectory(path: string): Effect.Effect<ReadonlyArray<string>, ProbeBenchmarkContractError> {
  return Effect.tryPromise({
    try: () => readdir(path),
    catch: (error) =>
      new ProbeBenchmarkContractError({
        path,
        reason: error instanceof Error ? error.message : String(error),
      }),
  });
}

function statPath(
  absolutePath: string,
  relativePath: string,
): Effect.Effect<Awaited<ReturnType<typeof lstat>>, ProbeBenchmarkContractError> {
  return Effect.tryPromise({
    try: () => lstat(absolutePath),
    catch: (error) =>
      new ProbeBenchmarkContractError({
        path: relativePath,
        reason: error instanceof Error ? error.message : String(error),
      }),
  });
}

function readFileBytes(path: string, errorPath: string): Effect.Effect<Uint8Array, ProbeBenchmarkContractError> {
  return Effect.tryPromise({
    try: () => readFile(path),
    catch: (error) =>
      new ProbeBenchmarkContractError({
        path: errorPath,
        reason: error instanceof Error ? error.message : String(error),
      }),
  });
}

function normalizeRepoPath(path: string): string {
  return path.split(sep).join("/").replace(/^\/+/, "");
}

function pathIsExcluded(path: string): boolean {
  const normalized = normalizeRepoPath(path);
  const pathWithSlashes = `${normalized}/`;
  const lower = normalized.toLowerCase();

  if (normalized === "") {
    return false;
  }

  if (
    pathWithSlashes.startsWith(".git/") ||
    pathWithSlashes.startsWith(".claude/") ||
    pathWithSlashes.startsWith(".git-worktrees/") ||
    pathWithSlashes.startsWith(".pylon-local/") ||
    pathWithSlashes.startsWith(".secrets/") ||
    pathWithSlashes.startsWith(".cache/") ||
    pathWithSlashes.startsWith("node_modules/") ||
    pathWithSlashes.startsWith("dist/") ||
    pathWithSlashes.startsWith("target/") ||
    pathWithSlashes.startsWith("coverage/") ||
    pathWithSlashes.startsWith("tmp/")
  ) {
    return true;
  }

  if (
    pathWithSlashes.includes("/node_modules/") ||
    pathWithSlashes.includes("/dist/") ||
    pathWithSlashes.includes("/target/") ||
    pathWithSlashes.includes("/coverage/") ||
    pathWithSlashes.includes("/tmp/") ||
    pathWithSlashes.includes("/.cache/")
  ) {
    return true;
  }

  return (
    publicUnsafePathName(lower) ||
    lower.endsWith(".dmg") ||
    lower.endsWith(".log") ||
    lower.endsWith(".tar") ||
    lower.endsWith(".tgz") ||
    lower.endsWith(".tmp") ||
    lower.endsWith(".zip") ||
    lower.endsWith(".study-artifact-index.json")
  );
}

function publicUnsafePathName(lower: string): boolean {
  return /(^|[/_.-])(raw[-_]?access[-_]?token|access[-_]?token|provider[-_]?secret|bearer|sk-[a-z0-9]|wallet[-_]?mnemonic|payment[-_]?preimage)([/_.-]|$)/i.test(
    lower,
  );
}

function repoCorpusKindForPath(path: string): string {
  const lower = path.toLowerCase();

  if (lower.endsWith(".md")) {
    return "markdown";
  }

  if (lower.endsWith(".ts") || lower.endsWith(".tsx")) {
    return "typescript";
  }

  if (lower.endsWith(".js") || lower.endsWith(".jsx") || lower.endsWith(".mjs") || lower.endsWith(".cjs")) {
    return "javascript";
  }

  if (lower.endsWith(".json") || lower.endsWith(".jsonl")) {
    return "json";
  }

  if (lower.endsWith(".toml") || lower.endsWith(".yaml") || lower.endsWith(".yml")) {
    return "config";
  }

  return "text";
}

function decodeRepoCorpusSchema<A, I>(
  schema: S.Schema<A, I>,
  value: unknown,
  path: string,
): Effect.Effect<A, ProbeBenchmarkContractError> {
  return S.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError(
      (error) =>
        new ProbeBenchmarkContractError({
          path,
          reason: String(error),
        }),
    ),
  );
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? repoCorpusError(path, "must be a non-empty string")
    : Effect.void;
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (refs.length === 0) {
    return repoCorpusError(path, "must include at least one ref");
  }

  const blankIndex = refs.findIndex((ref) => ref.trim().length === 0);
  return blankIndex === -1
    ? Effect.void
    : repoCorpusError(`${path}[${blankIndex}]`, "must be a non-empty ref");
}

function repoCorpusError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
