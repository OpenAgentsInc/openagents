// Standing freshness signal for the OpenAgents repo study artifact (SA-4, EPIC #5337).
//
// The committed digest index (SA-1) pins the exact commit the artifact was
// studied against. Because the artifact studies bounded commit history, its
// `indexHash` is intrinsically per-commit: it changes after *any* later commit,
// even one that touches no studied file. A naive "indexHash drifted => stale"
// gate would therefore be red on essentially every commit and could not be
// trusted by SA-2 / SA-3.
//
// This module derives a *trustworthy* staleness verdict by separating two kinds
// of drift, using the commit-INDEPENDENT `corpusContentHash` (a digest of the
// admitted file content) rather than `corpusManifestHash`/`indexHash`, both of
// which embed the HEAD commit and so change on every commit:
//
//   - content drift  — the corpus CONTENT hash changed, i.e. an admitted source
//                       file actually changed. This is the meaningful re-study
//                       trigger.
//   - commit drift   — HEAD/history moved but the studied content is byte-identical
//                       (corpus content hash unchanged). This is cheap and
//                       expected; the studied knowledge is still correct.
//
// `fresh` means: no content drift AND the correctness gate is green — the studied
// substrate still matches the tree, regardless of how many commits have landed.
// SA-2/SA-3 can consume studied knowledge whenever the verdict is `fresh`. The
// cadence (CI-on-merge or scheduled) refreshes the committed index when the
// verdict is `stale`, which is the cheap-incremental property: we only re-write
// the small index when content actually moved.

import { execFile as execFileCallback } from "node:child_process";
import { resolve } from "node:path";
import { promisify } from "node:util";
import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  OpenAgentsRepoStudyArtifactIndex,
} from "./openagents-study-artifact";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

const execFile = promisify(execFileCallback);

export const OPENAGENTS_REPO_STUDY_FRESHNESS_SCHEMA_REF =
  "openagents.repo_study_artifact_freshness.v0" as const;

// fresh        — studied content matches the live tree and the gate is green.
// stale        — an admitted source file changed since the studied commit; re-study.
// gate_failed  — the regenerated artifact failed the verification correctness gate.
export const OPENAGENTS_REPO_STUDY_FRESHNESS_STATUSES = ["fresh", "gate_failed", "stale"] as const;
export type OpenAgentsRepoStudyFreshnessStatus =
  (typeof OPENAGENTS_REPO_STUDY_FRESHNESS_STATUSES)[number];

// none           — nothing to do.
// refresh_index  — content drifted; re-run `--write` to refresh the committed index.
// reverify_gate  — the correctness gate is red; investigate before refreshing.
export const OPENAGENTS_REPO_STUDY_FRESHNESS_RECOMMENDATIONS = [
  "none",
  "refresh_index",
  "reverify_gate",
] as const;
export type OpenAgentsRepoStudyFreshnessRecommendation =
  (typeof OPENAGENTS_REPO_STUDY_FRESHNESS_RECOMMENDATIONS)[number];

export const OpenAgentsRepoStudyArtifactFreshness = S.Struct({
  // Identity of the freshness verdict itself (pure function of the fields below).
  freshnessHash: S.String,
  freshnessRef: S.String,
  schemaRef: S.Literal(OPENAGENTS_REPO_STUDY_FRESHNESS_SCHEMA_REF),
  sourceBoundary: S.Literal("public_refs_only"),
  // Verdict.
  status: S.Literals([...OPENAGENTS_REPO_STUDY_FRESHNESS_STATUSES]),
  recommendation: S.Literals([...OPENAGENTS_REPO_STUDY_FRESHNESS_RECOMMENDATIONS]),
  // The two drift kinds (see module header).
  contentDrift: S.Boolean,
  commitDrift: S.Boolean,
  correctnessGatePassed: S.Boolean,
  // How stale, and against what.
  commitsBehind: S.Number,
  repo: S.String,
  staleSinceCommit: S.String,
  headCommit: S.String,
  // Drift evidence: committed (studied) vs regenerated (live) digests.
  // The content hashes are commit-independent and drive `contentDrift`; the index
  // hashes embed the commit and are recorded as identity evidence only.
  committedCorpusContentHash: S.String,
  regeneratedCorpusContentHash: S.String,
  committedIndexHash: S.String,
  regeneratedIndexHash: S.String,
});
export type OpenAgentsRepoStudyArtifactFreshness = typeof OpenAgentsRepoStudyArtifactFreshness.Type;

export interface EvaluateOpenAgentsRepoStudyFreshnessInput {
  // The small digest index committed at the studied commit (SA-1 identity).
  readonly committed: OpenAgentsRepoStudyArtifactIndex;
  // The index regenerated from the live tree right now.
  readonly regenerated: OpenAgentsRepoStudyArtifactIndex;
  // HEAD of the live tree (defaults to the regenerated index commit).
  readonly headCommit?: string;
  // Commits between the studied commit and HEAD (defaults to 0). The cadence/CLI
  // supplies this from git; freshness itself stays pure.
  readonly commitsBehind?: number;
}

export function evaluateOpenAgentsRepoStudyFreshness(
  input: EvaluateOpenAgentsRepoStudyFreshnessInput,
): Effect.Effect<
  OpenAgentsRepoStudyArtifactFreshness,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const { committed, regenerated } = input;

    if (committed.repo !== regenerated.repo) {
      return yield* freshnessError(
        "studyFreshness.repo",
        "committed and regenerated index must describe the same repo",
      );
    }

    const commitsBehind = input.commitsBehind ?? 0;
    if (!Number.isInteger(commitsBehind) || commitsBehind < 0) {
      return yield* freshnessError("studyFreshness.commitsBehind", "must be a non-negative integer");
    }

    const contentDrift = committed.corpusContentHash !== regenerated.corpusContentHash;
    const commitDrift = !contentDrift && committed.indexHash !== regenerated.indexHash;
    const correctnessGatePassed = regenerated.correctnessGatePassed;

    const status: OpenAgentsRepoStudyFreshnessStatus = !correctnessGatePassed
      ? "gate_failed"
      : contentDrift
        ? "stale"
        : "fresh";

    const recommendation: OpenAgentsRepoStudyFreshnessRecommendation = !correctnessGatePassed
      ? "reverify_gate"
      : contentDrift
        ? "refresh_index"
        : "none";

    const base = {
      commitDrift,
      commitsBehind,
      committedCorpusContentHash: committed.corpusContentHash,
      committedIndexHash: committed.indexHash,
      contentDrift,
      correctnessGatePassed,
      headCommit: input.headCommit ?? regenerated.commit,
      recommendation,
      regeneratedCorpusContentHash: regenerated.corpusContentHash,
      regeneratedIndexHash: regenerated.indexHash,
      repo: committed.repo,
      schemaRef: OPENAGENTS_REPO_STUDY_FRESHNESS_SCHEMA_REF,
      sourceBoundary: "public_refs_only",
      staleSinceCommit: committed.commit,
      status,
    } satisfies Omit<OpenAgentsRepoStudyArtifactFreshness, "freshnessHash" | "freshnessRef">;

    const freshnessHash = sha256Ref(stableJson(base));
    const freshness: OpenAgentsRepoStudyArtifactFreshness = {
      ...base,
      freshnessHash,
      freshnessRef: `openagents_repo_study_artifact_freshness.${shortHash(freshnessHash)}`,
    };

    return yield* decodeOpenAgentsRepoStudyArtifactFreshness(freshness);
  });
}

export function decodeOpenAgentsRepoStudyArtifactFreshness(
  value: unknown,
): Effect.Effect<
  OpenAgentsRepoStudyArtifactFreshness,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studyFreshness");
    const freshness = yield* S.decodeUnknownEffect(OpenAgentsRepoStudyArtifactFreshness)(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeBenchmarkContractError({
            path: "studyFreshness",
            reason: String(error),
          }),
      ),
    );

    const { freshnessHash: _freshnessHash, freshnessRef: _freshnessRef, ...stable } = freshness;
    if (freshness.freshnessHash !== sha256Ref(stableJson(stable))) {
      return yield* freshnessError(
        "studyFreshness.freshnessHash",
        "must match deterministic freshness content hash",
      );
    }

    return freshness;
  });
}

// Count commits on HEAD that are newer than the studied commit. Returns 0 when
// the studied commit is HEAD, is unreachable, or git is unavailable; the verdict
// already encodes drift via the digest comparison, so the count is advisory.
export function readOpenAgentsRepoCommitsBehind(input: {
  readonly rootDir: string;
  readonly studiedCommit: string;
}): Effect.Effect<number> {
  return Effect.tryPromise(async () => {
    const rootDir = resolve(input.rootDir);
    const { stdout } = await execFile(
      "git",
      ["-C", rootDir, "rev-list", "--count", `${input.studiedCommit}..HEAD`],
      { maxBuffer: 16 * 1024 * 1024 },
    );
    const count = Number.parseInt(stdout.trim(), 10);
    return Number.isInteger(count) && count >= 0 ? count : 0;
  }).pipe(Effect.orElseSucceed(() => 0));
}

export function readOpenAgentsRepoHeadCommit(rootDir: string): Effect.Effect<string | undefined> {
  return Effect.tryPromise(async () => {
    const { stdout } = await execFile("git", ["-C", resolve(rootDir), "rev-parse", "HEAD"], {
      maxBuffer: 1024 * 1024,
    });
    const head = stdout.trim();
    return head.length > 0 ? head : undefined;
  }).pipe(Effect.orElseSucceed(() => undefined));
}

function freshnessError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
