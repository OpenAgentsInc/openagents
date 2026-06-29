import { execFile as execFileCallback } from "node:child_process";
import { lstat, readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { promisify } from "node:util";
import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { containsSecretMaterial, type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  buildOpenAgentsRepoCorpusManifest,
  extractOpenAgentsRepoCorpusEvidenceSpan,
  OpenAgentsRepoCorpusEvidenceSpan,
  OpenAgentsRepoCorpusManifest,
  openAgentsRepoCorpusEvidenceSpanHash,
  openAgentsRepoCorpusManifestHash,
} from "./repo-corpus-manifest";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

const execFile = promisify(execFileCallback);

export const OPENAGENTS_REPO_STUDY_PACKET_SCHEMA_REF = "openagents.repo_study_packet.v0" as const;

export const OPENAGENTS_REPO_STUDY_PACKET_SECTION_KINDS = [
  "source_map",
  "invariant_map",
  "typed_ref_glossary",
  "trap_catalog",
  "test_command_catalog",
  "edit_playbook",
  "retained_failure_fixture",
] as const;

export const OpenAgentsRepoStudyPacketSectionKind = S.Literals([
  ...OPENAGENTS_REPO_STUDY_PACKET_SECTION_KINDS,
]);
export type OpenAgentsRepoStudyPacketSectionKind = typeof OpenAgentsRepoStudyPacketSectionKind.Type;

export const OpenAgentsRepoStudyPacketRationaleKind = S.Literals([
  "openagents_repo",
  "commit_history",
  "backroom_archive",
  "tassadar_audit",
  "machine_studying_roadmap",
]);
export type OpenAgentsRepoStudyPacketRationaleKind = typeof OpenAgentsRepoStudyPacketRationaleKind.Type;

export const OpenAgentsRepoStudyPacketExternalAvailability = S.Literals([
  "available",
  "unavailable_external_workspace",
]);
export type OpenAgentsRepoStudyPacketExternalAvailability =
  typeof OpenAgentsRepoStudyPacketExternalAvailability.Type;

export const OpenAgentsRepoStudyPacketCommit = S.Struct({
  commit: S.String,
  committedAt: S.String,
  subjectDigest: S.String,
  subjectPreview: S.String,
});
export type OpenAgentsRepoStudyPacketCommit = typeof OpenAgentsRepoStudyPacketCommit.Type;

export const OpenAgentsRepoStudyPacketSection = S.Struct({
  corpusEntryPaths: S.Array(S.String),
  description: S.String,
  kind: OpenAgentsRepoStudyPacketSectionKind,
  ref: S.String,
  sourceAuthorityRefs: S.Array(S.String),
});
export type OpenAgentsRepoStudyPacketSection = typeof OpenAgentsRepoStudyPacketSection.Type;

export const OpenAgentsRepoStudyPacketRationaleSource = S.Struct({
  availability: OpenAgentsRepoStudyPacketExternalAvailability,
  byteSize: S.optional(S.Number),
  commit: S.optional(S.String),
  kind: OpenAgentsRepoStudyPacketRationaleKind,
  path: S.optional(S.String),
  ref: S.String,
  repo: S.String,
  sourceHash: S.optional(S.String),
});
export type OpenAgentsRepoStudyPacketRationaleSource = typeof OpenAgentsRepoStudyPacketRationaleSource.Type;

export const OpenAgentsRepoStudyPacket = S.Struct({
  commit: S.String,
  commitHistory: S.Array(OpenAgentsRepoStudyPacketCommit),
  corpusManifestHash: S.String,
  corpusManifestRef: S.String,
  evidenceSpans: S.Array(OpenAgentsRepoCorpusEvidenceSpan),
  generatedAt: S.String,
  packetHash: S.String,
  packetRef: S.String,
  rationaleSources: S.Array(OpenAgentsRepoStudyPacketRationaleSource),
  repo: S.String,
  schemaRef: S.Literal(OPENAGENTS_REPO_STUDY_PACKET_SCHEMA_REF),
  sections: S.Array(OpenAgentsRepoStudyPacketSection),
  sourceBoundary: S.Literal("public_refs_only"),
});
export type OpenAgentsRepoStudyPacket = typeof OpenAgentsRepoStudyPacket.Type;

export interface BuildOpenAgentsRepoStudyPacketInput {
  readonly backroomRepo?: string;
  readonly backroomRootDir?: string;
  readonly commit: string;
  readonly commitHistory?: ReadonlyArray<OpenAgentsRepoStudyPacketCommit>;
  readonly evidenceSpanPaths?: ReadonlyArray<string>;
  readonly generatedAt?: string;
  readonly manifest?: OpenAgentsRepoCorpusManifest;
  readonly packetRef?: string;
  readonly rationaleSources?: ReadonlyArray<OpenAgentsRepoStudyPacketRationaleSource>;
  readonly repo: string;
  readonly rootDir: string;
  readonly sections?: ReadonlyArray<OpenAgentsRepoStudyPacketSection>;
}

const DEFAULT_EVIDENCE_SPAN_PATHS = [
  "AGENTS.md",
  "INVARIANTS.md",
  "docs/promises/README.md",
  "docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md",
  "docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md",
  "packages/probe/packages/runtime/src/benchmark/openagents-study-graph.ts",
  "packages/probe/packages/runtime/src/benchmark/openagents-study-packet.ts",
  "packages/probe/packages/runtime/src/benchmark/openagents-study-verification.ts",
  "packages/probe/packages/runtime/src/benchmark/openagents-autopilot-coder-studied-context.ts",
  "packages/probe/packages/runtime/src/benchmark/openagents-studybench-eval-harness.ts",
  "packages/probe/packages/runtime/src/benchmark/repo-corpus-manifest.ts",
  "packages/probe/packages/runtime/src/benchmark/studybench.ts",
] as const;

const COMMIT_SUBJECT_UNSAFE_PATTERNS = [
  /\b(raw[_ -]?access[_ -]?token|access[_ -]?token|provider[_ -]?secret|bearer|sk-[a-z0-9])\b/i,
  /\b(hidden[_ -]?verifier|benchmark[_ -]?secret|wallet[_ -]?mnemonic|payment[_ -]?preimage)\b/i,
] as const;

export function buildOpenAgentsRepoStudyPacket(
  input: BuildOpenAgentsRepoStudyPacketInput,
): Effect.Effect<OpenAgentsRepoStudyPacket, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const rootDir = resolve(input.rootDir);
    const manifest =
      input.manifest ??
      (yield* buildOpenAgentsRepoCorpusManifest({
        commit: input.commit,
        generatedAt: input.generatedAt,
        repo: input.repo,
        rootDir,
      }));
    const commitHistory =
      input.commitHistory === undefined
        ? yield* readOpenAgentsRepoCommitHistory(rootDir)
        : [...input.commitHistory];
    const evidenceSpans = yield* buildStudyPacketEvidenceSpans({
      manifest,
      paths: input.evidenceSpanPaths ?? DEFAULT_EVIDENCE_SPAN_PATHS,
      rootDir,
    });
    const rationaleSources =
      input.rationaleSources === undefined
        ? yield* buildStudyPacketRationaleSources({
            backroomRepo: input.backroomRepo ?? "OpenAgentsInc/backroom",
            backroomRootDir: input.backroomRootDir ?? resolve(rootDir, "..", "backroom"),
            commit: input.commit,
            manifest,
            repo: input.repo,
          })
        : [...input.rationaleSources];
    const sections = input.sections === undefined ? buildStudyPacketSections(manifest) : [...input.sections];
    const basePacket: OpenAgentsRepoStudyPacket = {
      commit: input.commit,
      commitHistory,
      corpusManifestHash: manifest.manifestHash,
      corpusManifestRef: manifest.manifestRef,
      evidenceSpans,
      generatedAt: input.generatedAt ?? "generated_at.withheld_for_stable_packet_hash",
      packetHash: "sha256:pending",
      packetRef: "openagents_repo_study_packet.pending",
      rationaleSources,
      repo: input.repo,
      schemaRef: OPENAGENTS_REPO_STUDY_PACKET_SCHEMA_REF,
      sections,
      sourceBoundary: "public_refs_only",
    };
    const packetHash = openAgentsRepoStudyPacketHash(basePacket);
    const packet: OpenAgentsRepoStudyPacket = {
      ...basePacket,
      packetHash,
      packetRef: input.packetRef ?? `openagents_repo_study_packet.${shortHash(packetHash)}`,
    };

    return yield* decodeOpenAgentsRepoStudyPacket(packet);
  });
}

export function decodeOpenAgentsRepoStudyPacket(
  value: unknown,
): Effect.Effect<OpenAgentsRepoStudyPacket, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "repoStudyPacket");
    const packet = yield* decodeStudyPacketSchema(OpenAgentsRepoStudyPacket, value, "repoStudyPacket");
    yield* validateOpenAgentsRepoStudyPacket(packet);
    return packet;
  });
}

export function openAgentsRepoStudyPacketHash(packet: OpenAgentsRepoStudyPacket): string {
  const {
    generatedAt: _generatedAt,
    packetHash: _packetHash,
    packetRef: _packetRef,
    ...stable
  } = packet;
  return sha256Ref(stableJson(stable));
}

export function readOpenAgentsRepoCommitHistory(
  rootDir: string,
): Effect.Effect<ReadonlyArray<OpenAgentsRepoStudyPacketCommit>, ProbeBenchmarkContractError> {
  return Effect.tryPromise({
    try: async () => {
      const { stdout } = await execFile(
        "git",
        ["-C", rootDir, "log", "--format=%H%x1f%ct%x1f%s%x1e"],
        { maxBuffer: 64 * 1024 * 1024 },
      );
      return parseGitLog(stdout);
    },
    catch: (error) =>
      new ProbeBenchmarkContractError({
        path: "repoStudyPacket.commitHistory",
        reason: error instanceof Error ? error.message : String(error),
      }),
  });
}

function buildStudyPacketEvidenceSpans(input: {
  readonly manifest: OpenAgentsRepoCorpusManifest;
  readonly paths: ReadonlyArray<string>;
  readonly rootDir: string;
}): Effect.Effect<ReadonlyArray<OpenAgentsRepoCorpusEvidenceSpan>, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const spans: OpenAgentsRepoCorpusEvidenceSpan[] = [];

    for (const path of input.paths) {
      if (!manifestHasPath(input.manifest, path)) {
        continue;
      }

      const endLine = yield* safeEvidenceEndLine(input.rootDir, path);
      const span = yield* extractOpenAgentsRepoCorpusEvidenceSpan({
        endLine,
        manifest: input.manifest,
        path,
        rootDir: input.rootDir,
        spanId: `repo_study_packet.${slugPath(path)}.l1_l${endLine}`,
        startLine: 1,
      });
      spans.push(span);
    }

    if (spans.length === 0) {
      return yield* studyPacketError("repoStudyPacket.evidenceSpans", "must include at least one evidence span");
    }

    return spans;
  });
}

function buildStudyPacketRationaleSources(input: {
  readonly backroomRepo: string;
  readonly backroomRootDir: string;
  readonly commit: string;
  readonly manifest: OpenAgentsRepoCorpusManifest;
  readonly repo: string;
}): Effect.Effect<ReadonlyArray<OpenAgentsRepoStudyPacketRationaleSource>, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    const backroom = yield* readBackroomRationaleSource(input.backroomRootDir, input.backroomRepo);
    return [
      {
        availability: "available",
        commit: input.commit,
        kind: "openagents_repo",
        ref: `rationale_source.openagents.repo.${shortHash(input.manifest.manifestHash)}`,
        repo: input.repo,
        sourceHash: input.manifest.manifestHash,
      },
      {
        availability: "available",
        commit: input.commit,
        kind: "commit_history",
        ref: `rationale_source.openagents.commit_history.${shortHash(input.commit)}`,
        repo: input.repo,
      },
      {
        availability: "available",
        commit: input.commit,
        kind: "tassadar_audit",
        path: "docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md",
        ref: "rationale_source.openagents.tassadar_audit.20260618",
        repo: input.repo,
      },
      {
        availability: "available",
        commit: input.commit,
        kind: "machine_studying_roadmap",
        path: "docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md",
        ref: "rationale_source.openagents.machine_studying_roadmap.20260617",
        repo: input.repo,
      },
      backroom,
    ];
  });
}

function buildStudyPacketSections(manifest: OpenAgentsRepoCorpusManifest): ReadonlyArray<OpenAgentsRepoStudyPacketSection> {
  return [
    {
      corpusEntryPaths: selectManifestPaths(manifest, [
        "AGENTS.md",
        "INVARIANTS.md",
        "apps/openagents.com/AGENTS.md",
        "apps/openagents.com/INVARIANTS.md",
        "docs/tassadar/",
        "docs/research/machine-studying/",
        "packages/probe/",
        "packages/tassadar-executor/",
      ]),
      description: "Directory and authority source map for the openagents repo study packet.",
      kind: "source_map",
      ref: "repo_study_section.openagents.source_map.v1",
      sourceAuthorityRefs: ["authority.openagents.repo_study.source_map"],
    },
    {
      corpusEntryPaths: selectManifestPaths(manifest, [
        "INVARIANTS.md",
        "apps/openagents.com/INVARIANTS.md",
        "docs/promises/",
        "docs/tassadar/",
      ]),
      description: "Invariant map covering routing, claim discipline, proof replay, and projection boundaries.",
      kind: "invariant_map",
      ref: "repo_study_section.openagents.invariant_map.v1",
      sourceAuthorityRefs: ["authority.openagents.invariants"],
    },
    {
      corpusEntryPaths: selectManifestPaths(manifest, [
        "docs/promises/README.md",
        "docs/tassadar/README.md",
        "packages/probe/packages/runtime/src/benchmark/openagents-autopilot-coder-studied-context.ts",
        "packages/probe/packages/runtime/src/benchmark/openagents-study-graph.ts",
        "packages/probe/packages/runtime/src/benchmark/openagents-study-packet.ts",
        "packages/probe/packages/runtime/src/benchmark/openagents-study-verification.ts",
        "packages/probe/packages/runtime/src/benchmark/openagents-studybench-eval-harness.ts",
        "packages/probe/packages/runtime/src/benchmark/studybench.ts",
        "packages/probe/packages/runtime/src/benchmark/repo-corpus-manifest.ts",
      ]),
      description: "Typed refs for promises, runs, packets, receipts, evidence spans, and benchmark artifacts.",
      kind: "typed_ref_glossary",
      ref: "repo_study_section.openagents.typed_ref_glossary.v1",
      sourceAuthorityRefs: ["authority.openagents.typed_refs"],
    },
    {
      corpusEntryPaths: selectManifestPaths(manifest, [
        "AGENTS.md",
        "INVARIANTS.md",
        "docs/research/machine-studying/openagents-studybench/private-boundary.md",
        "docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md",
      ]),
      description: "Trap catalog for deprecated routing, overclaims, unsafe projection, and exactness confusion.",
      kind: "trap_catalog",
      ref: "repo_study_section.openagents.trap_catalog.v1",
      sourceAuthorityRefs: ["authority.openagents.repo_study.traps"],
    },
    {
      corpusEntryPaths: selectManifestPaths(manifest, [
        "packages/probe/packages/runtime/package.json",
        "apps/openagents.com/package.json",
        "docs/autopilot-coder/2026-06-13-afk-autonomous-loop.md",
      ]),
      description: "Focused test command catalog for Probe, OpenAgents.com, Tassadar, and deployment gates.",
      kind: "test_command_catalog",
      ref: "repo_study_section.openagents.test_command_catalog.v1",
      sourceAuthorityRefs: ["authority.openagents.repo_study.tests"],
    },
    {
      corpusEntryPaths: selectManifestPaths(manifest, [
        "docs/research/machine-studying/openagents-studybench/study-packets/openagents-launch-study-packet-v0.md",
        "docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md",
      ]),
      description: "Edit playbooks for study packet, public projection, Probe benchmark, and launch-copy edits.",
      kind: "edit_playbook",
      ref: "repo_study_section.openagents.edit_playbook.v1",
      sourceAuthorityRefs: ["authority.openagents.repo_study.playbooks"],
    },
    {
      corpusEntryPaths: selectManifestPaths(manifest, [
        "docs/research/machine-studying/openagents-studybench/public-retained/openagents-launch-v0.jsonl",
        "packages/probe/docs/benchmarks/2026-06-17-openagents-studybench-mvp-14-comparison.json",
      ]),
      description: "Public retained fixture refs for launch-study failures and score comparisons.",
      kind: "retained_failure_fixture",
      ref: "repo_study_section.openagents.retained_failure_fixture.v1",
      sourceAuthorityRefs: ["authority.openagents.repo_study.retained_failures"],
    },
  ].filter((section) => section.corpusEntryPaths.length > 0);
}

function readBackroomRationaleSource(
  rootDir: string,
  repo: string,
): Effect.Effect<OpenAgentsRepoStudyPacketRationaleSource, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    const readmePath = resolve(rootDir, "README.md");
    const maybeStats = yield* statPath(readmePath, "repoStudyPacket.backroom.path").pipe(
      Effect.catch(() => Effect.succeed(null)),
    );

    if (maybeStats === null || !maybeStats.isFile()) {
      return {
        availability: "unavailable_external_workspace",
        kind: "backroom_archive",
        path: "README.md",
        ref: "rationale_source.backroom.archive.unavailable",
        repo,
      };
    }

    const bytes = yield* readFileBytes(readmePath, "repoStudyPacket.backroom.README.md");
    const commit = yield* readGitHead(rootDir).pipe(Effect.catch(() => Effect.succeed(undefined)));
    const sourceHash = sha256Ref(bytes);
    return {
      availability: "available",
      byteSize: bytes.byteLength,
      commit,
      kind: "backroom_archive",
      path: "README.md",
      ref: `rationale_source.backroom.archive.${shortHash(sourceHash)}`,
      repo,
      sourceHash,
    };
  });
}

function readGitHead(rootDir: string): Effect.Effect<string, ProbeBenchmarkContractError> {
  return Effect.tryPromise({
    try: async () => {
      const { stdout } = await execFile("git", ["-C", rootDir, "rev-parse", "HEAD"]);
      return stdout.trim();
    },
    catch: (error) =>
      new ProbeBenchmarkContractError({
        path: "repoStudyPacket.gitHead",
        reason: error instanceof Error ? error.message : String(error),
      }),
  });
}

function validateOpenAgentsRepoStudyPacket(
  packet: OpenAgentsRepoStudyPacket,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(packet.repo, "repoStudyPacket.repo");
    yield* requireNonEmpty(packet.commit, "repoStudyPacket.commit");
    yield* requireNonEmpty(packet.packetRef, "repoStudyPacket.packetRef");
    yield* requireNonEmpty(packet.corpusManifestRef, "repoStudyPacket.corpusManifestRef");

    if (packet.packetHash !== openAgentsRepoStudyPacketHash(packet)) {
      return yield* studyPacketError("repoStudyPacket.packetHash", "must match deterministic packet content hash");
    }

    if (!packet.corpusManifestHash.startsWith("sha256:")) {
      return yield* studyPacketError("repoStudyPacket.corpusManifestHash", "must be a sha256 ref");
    }

    if (packet.commitHistory.length === 0) {
      return yield* studyPacketError("repoStudyPacket.commitHistory", "must include commit history entries");
    }

    if (packet.evidenceSpans.length === 0) {
      return yield* studyPacketError("repoStudyPacket.evidenceSpans", "must include evidence spans");
    }

    if (packet.sections.length === 0) {
      return yield* studyPacketError("repoStudyPacket.sections", "must include study packet sections");
    }

    const sectionKinds = new Set(packet.sections.map((section) => section.kind));
    for (const requiredKind of OPENAGENTS_REPO_STUDY_PACKET_SECTION_KINDS) {
      if (!sectionKinds.has(requiredKind)) {
        return yield* studyPacketError("repoStudyPacket.sections", `missing ${requiredKind} section`);
      }
    }

    const rationaleKinds = new Set(packet.rationaleSources.map((source) => source.kind));
    for (const requiredKind of ["openagents_repo", "commit_history", "backroom_archive"] as const) {
      if (!rationaleKinds.has(requiredKind)) {
        return yield* studyPacketError("repoStudyPacket.rationaleSources", `missing ${requiredKind} rationale source`);
      }
    }

    for (const [index, commit] of packet.commitHistory.entries()) {
      const path = `repoStudyPacket.commitHistory[${index}]`;
      yield* requireNonEmpty(commit.commit, `${path}.commit`);
      yield* requireNonEmpty(commit.committedAt, `${path}.committedAt`);
      yield* requireSha256(commit.subjectDigest, `${path}.subjectDigest`);
      yield* requireNonEmpty(commit.subjectPreview, `${path}.subjectPreview`);
    }

    for (const [index, span] of packet.evidenceSpans.entries()) {
      const path = `repoStudyPacket.evidenceSpans[${index}]`;
      if (span.corpusRef !== packet.corpusManifestRef) {
        return yield* studyPacketError(`${path}.corpusRef`, "must match packet corpus manifest ref");
      }

      if (span.spanHash !== openAgentsRepoCorpusEvidenceSpanHash(span.corpusRef, span.evidence)) {
        return yield* studyPacketError(`${path}.spanHash`, "must match deterministic evidence span hash");
      }
    }

    for (const [index, section] of packet.sections.entries()) {
      const path = `repoStudyPacket.sections[${index}]`;
      yield* requireNonEmpty(section.ref, `${path}.ref`);
      yield* requireNonEmpty(section.description, `${path}.description`);
      yield* requireNonEmptyRefs(section.sourceAuthorityRefs, `${path}.sourceAuthorityRefs`);
      yield* requireNonEmptyRefs(section.corpusEntryPaths, `${path}.corpusEntryPaths`);
    }

    for (const [index, source] of packet.rationaleSources.entries()) {
      const path = `repoStudyPacket.rationaleSources[${index}]`;
      yield* requireNonEmpty(source.ref, `${path}.ref`);
      yield* requireNonEmpty(source.repo, `${path}.repo`);

      if (source.availability === "available" && source.sourceHash !== undefined) {
        yield* requireSha256(source.sourceHash, `${path}.sourceHash`);
      }
    }
  });
}

function parseGitLog(stdout: string): ReadonlyArray<OpenAgentsRepoStudyPacketCommit> {
  return stdout
    .split("\x1e")
    .map((record) => record.trim())
    .filter((record) => record.length > 0)
    .map((record) => {
      const [commit = "", timestamp = "0", subject = ""] = record.split("\x1f");
      const epochMs = Number.parseInt(timestamp, 10) * 1000;
      return {
        commit,
        committedAt: Number.isFinite(epochMs) ? new Date(epochMs).toISOString() : "1970-01-01T00:00:00.000Z",
        subjectDigest: sha256Ref(subject),
        subjectPreview: publicSafeCommitSubjectPreview(subject),
      };
    });
}

function publicSafeCommitSubjectPreview(subject: string): string {
  const normalized = subject.replace(/\s+/g, " ").trim();
  const preview = normalized.length === 0 ? "empty commit subject" : normalized.slice(0, 180);

  return COMMIT_SUBJECT_UNSAFE_PATTERNS.some((pattern) => pattern.test(preview))
    || containsSecretMaterial(preview)
    ? "redacted public-unsafe commit subject"
    : preview;
}

function safeEvidenceEndLine(rootDir: string, path: string): Effect.Effect<number, ProbeBenchmarkContractError> {
  return Effect.tryPromise({
    try: async () => {
      const text = await readFile(resolve(rootDir, path), "utf8");
      const lineCount = Math.max(1, text.replaceAll("\r\n", "\n").split("\n").length);
      return Math.min(6, lineCount);
    },
    catch: (error) =>
      new ProbeBenchmarkContractError({
        path,
        reason: error instanceof Error ? error.message : String(error),
      }),
  });
}

function selectManifestPaths(
  manifest: OpenAgentsRepoCorpusManifest,
  selectors: ReadonlyArray<string>,
): ReadonlyArray<string> {
  const selected = manifest.entries
    .map((entry) => entry.path)
    .filter((path) =>
      selectors.some((selector) =>
        selector.endsWith("/") ? path.startsWith(selector) : path === selector,
      ),
    )
    .sort((left, right) => left.localeCompare(right));

  return [...new Set(selected)].slice(0, 80);
}

function manifestHasPath(manifest: OpenAgentsRepoCorpusManifest, path: string): boolean {
  return manifest.entries.some((entry) => entry.path === path);
}

function statPath(
  absolutePath: string,
  errorPath: string,
): Effect.Effect<Awaited<ReturnType<typeof lstat>>, ProbeBenchmarkContractError> {
  return Effect.tryPromise({
    try: () => lstat(absolutePath),
    catch: (error) =>
      new ProbeBenchmarkContractError({
        path: errorPath,
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

function decodeStudyPacketSchema<A, I>(
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
  return value.trim().length === 0 ? studyPacketError(path, "must be a non-empty string") : Effect.void;
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (refs.length === 0) {
    return studyPacketError(path, "must include at least one ref");
  }

  const blankIndex = refs.findIndex((ref) => ref.trim().length === 0);
  return blankIndex === -1 ? Effect.void : studyPacketError(`${path}[${blankIndex}]`, "must be a non-empty ref");
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:") ? Effect.void : studyPacketError(path, "must be a sha256 ref");
}

function slugPath(path: string): string {
  return path
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 96);
}

function studyPacketError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
