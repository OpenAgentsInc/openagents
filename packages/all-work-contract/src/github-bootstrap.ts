import { createHash } from "node:crypto";
import { Effect, Schema as S } from "effect";

import {
  decodePlanningGraph,
  decodeWorkSnapshot,
  type PlanningGraph,
  type PlanningResource,
  PlanningResourceSchema,
  type PlanningTextRecord,
  PlanningTextRecordSchema,
  type ProjectionIssue,
  ProjectionIssueSchema,
  type SourceCoordinate,
  SourceCoordinateSchema,
  type WorkPlanningLink,
  WorkPlanningLinkSchema,
  type WorkRelation,
  WorkRelationSchema,
  type WorkSnapshot,
} from "./generated.ts";
import { encodeAllWorkCanonicalJson } from "./semantic.ts";

const GitHubCommentSchema = S.Struct({
  id: S.String,
  body: S.String,
  authorRef: S.String,
  createdAt: S.String,
  sourceRevision: S.String,
});

const GitHubRelationSchema = S.Struct({
  kind: S.Literals([
    "parent",
    "child",
    "blocks",
    "blocked_by",
    "related",
    "duplicate",
    "supersedes",
  ]),
  targetRepository: S.String,
  targetNumber: S.Number.check(S.isInt(), S.isGreaterThan(0)),
});

const GitHubPlanningCoordinatesSchema = S.Struct({
  projectRef: S.NullOr(S.String),
  projectMilestoneRef: S.NullOr(S.String),
  cycleRef: S.NullOr(S.String),
  workflowStateRef: S.NullOr(S.String),
  releasePlanningRecordRef: S.NullOr(S.String),
});

const GitHubIssueSchema = S.Struct({
  repository: S.String,
  number: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  title: S.String,
  body: S.optionalKey(S.NullOr(S.String)),
  state: S.Literals(["open", "closed"]),
  priority: S.Literals(["none", "urgent", "high", "normal", "low"]),
  updatedAt: S.String,
  sourceRevision: S.String,
  url: S.String,
  labels: S.Array(S.String),
  comments: S.Array(GitHubCommentSchema),
  relations: S.Array(GitHubRelationSchema),
  planning: GitHubPlanningCoordinatesSchema,
});
export type GitHubIssue = typeof GitHubIssueSchema.Type;

const GitHubPageSchema = S.Struct({
  repository: S.String,
  page: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  cursor: S.NullOr(S.String),
  nextCursor: S.NullOr(S.String),
  complete: S.Boolean,
  issues: S.Array(GitHubIssueSchema),
});

export const GitHubBootstrapBatchSchema = S.Struct({
  bootstrapRef: S.String,
  fetchedAt: S.String,
  expectedRepositories: S.Array(S.String),
  pages: S.Array(GitHubPageSchema),
  resources: S.Array(S.Unknown),
});
export type GitHubBootstrapBatch = typeof GitHubBootstrapBatchSchema.Type;

export const GitHubBootstrapReceiptSchema = S.Struct({
  bootstrapRef: S.String,
  reconciliationDigest: S.String,
  previousRevision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  revision: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  imported: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  updated: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  unchanged: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  unavailable: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  duplicateDeliveries: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  pageGapCount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  noOp: S.Boolean,
  githubWriteCount: S.Literal(0),
  reconciledAt: S.String,
});
export type GitHubBootstrapReceipt = typeof GitHubBootstrapReceiptSchema.Type;

export class GitHubBootstrapError extends S.TaggedErrorClass<GitHubBootstrapError>()(
  "AllWorkPlanningAuthority.GitHubBootstrapError",
  {
    reason: S.Literals([
      "invalid_batch",
      "conflicting_duplicate",
      "invalid_resource",
      "invalid_projection",
    ]),
    detail: S.String,
  },
) {}

const decodeBatch = (input: unknown) =>
  S.decodeUnknownEffect(GitHubBootstrapBatchSchema)(input, { onExcessProperty: "error" });

const sha256 = (value: unknown): string =>
  createHash("sha256").update(encodeAllWorkCanonicalJson(value)).digest("hex");

const repositorySlug = (repository: string): string =>
  repository
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/gu, "-")
    .replaceAll(/^-+|-+$/gu, "");

export const githubSourceRef = (repository: string, number: number): string =>
  `github:${repositorySlug(repository)}:${number}`;

export const githubWorkRef = (repository: string, number: number): string =>
  `work:github:${repositorySlug(repository)}:${number}`;

const issueKey = (issue: GitHubIssue): string => githubSourceRef(issue.repository, issue.number);

const issueCore = (issue: GitHubIssue): unknown => ({
  repository: issue.repository,
  number: issue.number,
  title: issue.title,
  body: issue.body ?? null,
  state: issue.state,
  priority: issue.priority,
  updatedAt: issue.updatedAt,
  sourceRevision: issue.sourceRevision,
  url: issue.url,
  planning: issue.planning,
});

const mergeIssueDeliveries = (
  previous: GitHubIssue,
  next: GitHubIssue,
): Effect.Effect<GitHubIssue, GitHubBootstrapError> =>
  Effect.gen(function* () {
    if (
      previous.sourceRevision === next.sourceRevision &&
      sha256(issueCore(previous)) !== sha256(issueCore(next))
    ) {
      return yield* new GitHubBootstrapError({
        reason: "conflicting_duplicate",
        detail: issueKey(previous),
      });
    }
    const newest = previous.updatedAt > next.updatedAt ? previous : next;
    const comments = new Map(
      [...previous.comments, ...next.comments].map((comment) => [comment.id, comment]),
    );
    const relations = new Map(
      [...previous.relations, ...next.relations].map((relation) => [sha256(relation), relation]),
    );
    return yield* S.decodeUnknownEffect(GitHubIssueSchema)({
      ...newest,
      labels: [...new Set([...previous.labels, ...next.labels])].sort(),
      comments: [...comments.values()].sort((left, right) => left.id.localeCompare(right.id)),
      relations: [...relations.values()].sort((left, right) =>
        sha256(left).localeCompare(sha256(right)),
      ),
    }).pipe(
      Effect.mapError(
        () => new GitHubBootstrapError({ reason: "invalid_batch", detail: issueKey(previous) }),
      ),
    );
  });

const normalizeIssues = (
  pages: GitHubBootstrapBatch["pages"],
): Effect.Effect<
  { readonly issues: ReadonlyArray<GitHubIssue>; readonly duplicates: number },
  GitHubBootstrapError
> =>
  Effect.gen(function* () {
    const bySource = new Map<string, GitHubIssue>();
    let duplicates = 0;
    for (const page of pages) {
      for (const issue of page.issues) {
        const key = issueKey(issue);
        const previous = bySource.get(key);
        if (previous === undefined) {
          bySource.set(key, issue);
          continue;
        }
        duplicates += 1;
        bySource.set(key, yield* mergeIssueDeliveries(previous, issue));
      }
    }
    return {
      issues: [...bySource.values()].sort((left, right) =>
        issueKey(left).localeCompare(issueKey(right)),
      ),
      duplicates,
    };
  });

const pageGapIssues = (batch: GitHubBootstrapBatch): ReadonlyArray<ProjectionIssue> => {
  const repositoriesWithPages = new Set(batch.pages.map((page) => page.repository));
  const missingRepositories = batch.expectedRepositories.filter(
    (repository) => !repositoriesWithPages.has(repository),
  );
  const incompletePages = batch.pages.filter((page) => !page.complete);
  return [
    ...missingRepositories.map((repository, index) =>
      S.decodeUnknownSync(ProjectionIssueSchema)({
        issueRef: `projection-issue:missing-repository:${index + 1}`,
        kind: "page_gap",
        sourceRef: null,
        detail: `Missing repository page: ${repository}`,
        observedAt: batch.fetchedAt,
      }),
    ),
    ...incompletePages.map((page) =>
      S.decodeUnknownSync(ProjectionIssueSchema)({
        issueRef: `projection-issue:page-gap:${repositorySlug(page.repository)}:${page.page}`,
        kind: "page_gap",
        sourceRef: page.cursor === null ? null : `github-cursor:${page.cursor}`,
        detail: `Incomplete page ${page.page} for ${page.repository}`,
        observedAt: batch.fetchedAt,
      }),
    ),
  ];
};

const graphIsComplete = (batch: GitHubBootstrapBatch): boolean =>
  batch.expectedRepositories.every((repository) =>
    batch.pages.some((page) => page.repository === repository),
  ) && batch.pages.every((page) => page.complete);

const importedSnapshot = (
  issue: GitHubIssue,
  previous: WorkSnapshot | undefined,
  fetchedAt: string,
): WorkSnapshot => {
  if (
    previous !== undefined &&
    previous.summary.sourceAuthority.sourceRef === issueKey(issue) &&
    previous.summary.completeness.cursor === `cursor:github:${issue.sourceRevision}`
  ) {
    return previous;
  }
  const revision = previous === undefined ? 1 : previous.summary.revision + 1;
  const state = issue.state === "closed" ? "completed" : "planned";
  const workRef = githubWorkRef(issue.repository, issue.number);
  const relations: ReadonlyArray<WorkRelation> = issue.relations.map((relation) =>
    S.decodeUnknownSync(WorkRelationSchema)({
      kind: relation.kind,
      targetWorkRef: githubWorkRef(relation.targetRepository, relation.targetNumber),
    }),
  );
  return decodeWorkSnapshot({
    summary: {
      contractVersion: "openagents.all_work_boundary.v1",
      workRef,
      title: issue.title,
      ...(issue.body === undefined || issue.body === null ? {} : { description: issue.body }),
      domain: "development",
      workClass: "task",
      state,
      priority: issue.priority,
      ownerRef: "principal:organization:openagents",
      assignee: null,
      agentDelegate: null,
      portfolio: null,
      sourceAuthority: {
        kind: "imported_read_only",
        sourceRef: issueKey(issue),
        adapterVersion: "github-bootstrap-v1",
        writable: false,
      },
      revision,
      updatedAt: issue.updatedAt,
      freshness: {
        state: "fresh",
        observedAt: fetchedAt,
        sourceUpdatedAt: issue.updatedAt,
      },
      completeness: {
        state: "complete",
        cursor: `cursor:github:${issue.sourceRevision}`,
        gapRefs: [],
      },
      redaction: {
        privacyClass: "public",
        redactedFieldCount: 0,
        policyRef: "policy:github-public-bootstrap-v1",
      },
    },
    issue: {
      workRef,
      identifier: `${repositorySlug(issue.repository)}#${issue.number}`,
      state,
      revision,
    },
    relations,
    threadRefs: [],
    sessionRefs: [],
    agentSessionRefs: [],
    agentActivityRefs: [],
    runRefs: [],
    intentRefs: [],
    eventRefs: [],
    receiptRefs: [],
    evidenceRefs: [],
    verificationRefs: [],
    ownerDispositionRefs: [],
  });
};

const unavailableSnapshot = (snapshot: WorkSnapshot, fetchedAt: string): WorkSnapshot =>
  decodeWorkSnapshot({
    ...snapshot,
    summary: {
      ...snapshot.summary,
      freshness: {
        ...snapshot.summary.freshness,
        state: "stale",
        observedAt: fetchedAt,
      },
      completeness: {
        ...snapshot.summary.completeness,
        state: "partial",
        gapRefs: [`event:source-unavailable:${snapshot.summary.workRef}`],
      },
    },
  });

const planningLink = (issue: GitHubIssue): WorkPlanningLink =>
  S.decodeUnknownSync(WorkPlanningLinkSchema)({
    workRef: githubWorkRef(issue.repository, issue.number),
    ...issue.planning,
  });

const sourceCoordinate = (issue: GitHubIssue, fetchedAt: string): SourceCoordinate =>
  S.decodeUnknownSync(SourceCoordinateSchema)({
    workRef: githubWorkRef(issue.repository, issue.number),
    sourceRef: issueKey(issue),
    repository: issue.repository,
    identifier: `${repositorySlug(issue.repository)}#${issue.number}`,
    url: issue.url,
    sourceRevision: issue.sourceRevision,
    fetchedAt,
    available: true,
  });

const textRecords = (issue: GitHubIssue): ReadonlyArray<PlanningTextRecord> =>
  issue.comments.map((comment) =>
    S.decodeUnknownSync(PlanningTextRecordSchema)({
      recordRef: `comment:github:${repositorySlug(issue.repository)}:${issue.number}:${repositorySlug(comment.id)}`,
      kind: "comment",
      workRef: githubWorkRef(issue.repository, issue.number),
      resourceRef: null,
      body: comment.body,
      authorRef: comment.authorRef,
      createdAt: comment.createdAt,
      sourceRef: `github-comment:${comment.id}`,
    }),
  );

const importedResources = (
  resources: ReadonlyArray<unknown>,
): Effect.Effect<ReadonlyArray<PlanningResource>, GitHubBootstrapError> =>
  Effect.forEach(resources, (resource) =>
    Effect.try({
      try: () => S.decodeUnknownSync(PlanningResourceSchema)(resource),
      catch: () => new GitHubBootstrapError({ reason: "invalid_resource", detail: "resource" }),
    }),
  );

export const reconcileGitHubBootstrap = Effect.fn(
  "AllWorkPlanningAuthority.reconcileGitHubBootstrap",
)(function* (previous: PlanningGraph, input: unknown) {
  const batch = yield* decodeBatch(input).pipe(
    Effect.mapError(() => new GitHubBootstrapError({ reason: "invalid_batch", detail: "decode" })),
  );
  const normalized = yield* normalizeIssues(batch.pages);
  const resources = yield* importedResources(batch.resources);
  const previousBySource = new Map(
    previous.work.map((snapshot) => [snapshot.summary.sourceAuthority.sourceRef, snapshot]),
  );
  const observedSources = new Set(normalized.issues.map(issueKey));
  const imported = normalized.issues.filter(
    (issue) => !previousBySource.has(issueKey(issue)),
  ).length;
  const unchanged = normalized.issues.filter((issue) => {
    const snapshot = previousBySource.get(issueKey(issue));
    return snapshot?.summary.completeness.cursor === `cursor:github:${issue.sourceRevision}`;
  }).length;
  const updated = normalized.issues.length - imported - unchanged;
  const retainedNative = previous.work.filter(
    (snapshot) => snapshot.summary.sourceAuthority.kind !== "imported_read_only",
  );
  const importedWork = normalized.issues.map((issue) =>
    importedSnapshot(issue, previousBySource.get(issueKey(issue)), batch.fetchedAt),
  );
  const missingImported = previous.work.filter(
    (snapshot) =>
      snapshot.summary.sourceAuthority.kind === "imported_read_only" &&
      !observedSources.has(snapshot.summary.sourceAuthority.sourceRef),
  );
  const complete = graphIsComplete(batch);
  const previousCoordinateBySource = new Map(
    previous.sourceCoordinates.map((coordinate) => [coordinate.sourceRef, coordinate]),
  );
  const newlyUnavailable = complete
    ? missingImported.filter(
        (snapshot) =>
          previousCoordinateBySource.get(snapshot.summary.sourceAuthority.sourceRef)?.available !==
          false,
      )
    : [];
  const preservedMissing = missingImported.map((snapshot) =>
    newlyUnavailable.includes(snapshot) ? unavailableSnapshot(snapshot, batch.fetchedAt) : snapshot,
  );
  const gaps = pageGapIssues(batch);
  const unavailableIssues: ReadonlyArray<ProjectionIssue> = complete
    ? missingImported.map((snapshot, index) =>
        S.decodeUnknownSync(ProjectionIssueSchema)({
          issueRef: `projection-issue:source-unavailable:${index + 1}`,
          kind: "source_unavailable",
          sourceRef: snapshot.summary.sourceAuthority.sourceRef,
          detail: `Source unavailable for ${snapshot.summary.workRef}`,
          observedAt: batch.fetchedAt,
        }),
      )
    : [];
  const sourceCoordinates = [
    ...previous.sourceCoordinates
      .filter((coordinate) => !observedSources.has(coordinate.sourceRef))
      .map((coordinate) =>
        complete
          ? S.decodeUnknownSync(SourceCoordinateSchema)({ ...coordinate, available: false })
          : coordinate,
      ),
    ...normalized.issues.map((issue) => sourceCoordinate(issue, batch.fetchedAt)),
  ];
  const comments = normalized.issues.flatMap(textRecords);
  const retainedNativeText = previous.textRecords.filter(
    (record) => record.sourceRef === undefined || record.sourceRef === null,
  );
  const reconciliationMaterial = {
    resources,
    issues: normalized.issues,
    completeness: complete,
    gaps,
  };
  const reconciliationDigest = sha256(reconciliationMaterial);
  const noOp =
    reconciliationDigest === previous.reconciliationDigest &&
    newlyUnavailable.length === 0 &&
    gaps.length === previous.projectionIssues.filter((issue) => issue.kind === "page_gap").length;
  const revision = noOp ? previous.revision : previous.revision + 1;
  const graph = decodePlanningGraph({
    ...previous,
    revision,
    eventCursor: noOp ? previous.eventCursor : `cursor:planning:${revision}`,
    reconciliationDigest,
    generatedAt: noOp ? previous.generatedAt : batch.fetchedAt,
    resources,
    work: [...retainedNative, ...importedWork, ...preservedMissing],
    planningLinks: [
      ...previous.planningLinks.filter((link) =>
        retainedNative.some((snapshot) => snapshot.summary.workRef === link.workRef),
      ),
      ...normalized.issues.map(planningLink),
    ],
    labelLinks: [
      ...previous.labelLinks.filter((link) =>
        retainedNative.some((snapshot) => snapshot.summary.workRef === link.workRef),
      ),
      ...normalized.issues.flatMap((issue) =>
        issue.labels.map((label) => ({
          workRef: githubWorkRef(issue.repository, issue.number),
          labelRef: `label:${repositorySlug(label)}`,
        })),
      ),
    ],
    textRecords: [...retainedNativeText, ...comments],
    releaseScopeLinks: [
      ...previous.releaseScopeLinks.filter((link) =>
        retainedNative.some((snapshot) => snapshot.summary.workRef === link.workRef),
      ),
      ...normalized.issues.flatMap((issue) =>
        issue.planning.releasePlanningRecordRef === null
          ? []
          : [
              {
                releasePlanningRecordRef: issue.planning.releasePlanningRecordRef,
                workRef: githubWorkRef(issue.repository, issue.number),
              },
            ],
      ),
    ],
    sourceCoordinates,
    projectionIssues: [...gaps, ...unavailableIssues],
    completeness: {
      state: complete ? "complete" : "gap",
      cursor: `cursor:github-bootstrap:${reconciliationDigest}`,
      gapRefs: gaps.map((gap) => gap.issueRef),
    },
    freshness: { state: "fresh", observedAt: batch.fetchedAt },
  });
  const receipt = S.decodeUnknownSync(GitHubBootstrapReceiptSchema)({
    bootstrapRef: batch.bootstrapRef,
    reconciliationDigest,
    previousRevision: previous.revision,
    revision,
    imported,
    updated,
    unchanged,
    unavailable: newlyUnavailable.length,
    duplicateDeliveries: normalized.duplicates,
    pageGapCount: gaps.length,
    noOp,
    githubWriteCount: 0,
    reconciledAt: batch.fetchedAt,
  });
  return { graph, receipt };
});
