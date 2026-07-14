import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import {
  QA_SWARM_RUN_PROJECTION_SCHEMA,
  assertResolverBackedQaSwarmProjection,
  buildResolverBackedQaSwarmBoardGraph,
  type QaSwarmProjectionEvidence,
  type QaSwarmRunProjection,
  type QaSwarmVerdict,
} from "@openagentsinc/qa-swarm-contract";

import type { Job, JobReceipt, JobStatus, SubmitRunInput } from "./control";

export { QA_SWARM_RUN_PROJECTION_SCHEMA };
export type { QaSwarmRunProjection, QaSwarmVerdict };
export type QaSwarmTierStatus = "scheduled" | "running" | "passed" | "failed" | "skipped";

export interface SubmitSwarmRunInput {
  readonly target: string;
  readonly targetName?: string;
  readonly title?: string;
  readonly runRef?: string;
  readonly scenario?: SubmitRunInput["scenario"];
  readonly real?: boolean;
  readonly tokenBudget?: number;
  readonly maxWorkers?: number;
  readonly maxRuns?: number;
  readonly includeLiveTiers?: boolean;
}

export interface QaSwarmRunTier {
  readonly backend: "fixture" | "gce-tier-2" | "cf-browser-rendering";
  readonly jobId?: string;
  readonly reason?: string;
  readonly status: QaSwarmTierStatus;
}

export interface QaSwarmRunSummary {
  readonly runRef: string;
  readonly shareUrl: string;
  readonly projectionPath: string;
  readonly projection: QaSwarmRunProjection;
  readonly caps: {
    readonly maxWorkers: number;
    readonly maxRuns: number;
    readonly tokenBudget: number;
  };
  readonly tiers: readonly QaSwarmRunTier[];
  readonly childRunIds: readonly string[];
}

export interface QaSwarmRunArtifactsResponse {
  readonly jobId: string;
  readonly status: JobStatus;
  readonly proUrl: string;
  readonly qaShareUrl: string | null;
  readonly jobReceipt: JobReceipt;
  readonly swarm: QaSwarmRunSummary | null;
}

const PRIVATE_MATERIAL_PATTERN =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|macaroon|mnemonic|oauth|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i;

const PUBLIC_REF_PATTERN =
  /^(artifact|coverage|frontier|perf|poster|projection|qa-run|redaction|test|trace|video)\.[a-z0-9][a-z0-9._-]*$/i;

const slug = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/^https?:\/\//, "")
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64) || "target";

const isPublicRef = (value: string): boolean =>
  PUBLIC_REF_PATTERN.test(value) && !PRIVATE_MATERIAL_PATTERN.test(value);

const assertPublicRefs = (refs: readonly string[], field: string): void => {
  const unsafe = refs.find(ref => !isPublicRef(ref));
  if (unsafe !== undefined) {
    throw new Error(`Unsafe QA Swarm projection ref in ${field}: ${unsafe}`);
  }
};

export const assertQaSwarmProjectionPublicSafe = (
  projection: QaSwarmRunProjection,
): QaSwarmRunProjection => {
  if (PRIVATE_MATERIAL_PATTERN.test(JSON.stringify(projection))) {
    throw new Error("QA Swarm projection contains private material");
  }
  assertPublicRefs([projection.projectionRef], "projectionRef");
  if (projection.nightlyArtifactRef !== undefined) {
    assertPublicRefs([projection.nightlyArtifactRef], "nightlyArtifactRef");
  }
  assertPublicRefs(projection.opaqueTargetRefs, "opaqueTargetRefs");
  assertPublicRefs(projection.publicSafetyRefs, "publicSafetyRefs");
  assertPublicRefs(projection.traceRefs, "traceRefs");
  assertPublicRefs(projection.verdictWall.map(item => item.receiptRef), "verdictWall.receiptRef");
  assertPublicRefs(
    projection.coverageFrontier.map(item => item.receiptRef),
    "coverageFrontier.receiptRef",
  );
  assertPublicRefs(projection.perfBudgets.map(item => item.receiptRef), "perfBudgets.receiptRef");
  assertPublicRefs(
    projection.videoRefs.flatMap(item => [item.videoRef, item.posterRef]),
    "videoRefs",
  );
  assertPublicRefs(
    projection.distilledTests.map(item => item.receiptRef),
    "distilledTests.receiptRef",
  );
  if (projection.target.ref !== undefined) {
    assertPublicRefs([projection.target.ref], "target.ref");
  }
  return projection;
};

export const swarmRunRefFor = (input: Readonly<{
  id: string;
  target: string;
  runRef?: string | undefined;
}>): string => input.runRef ?? `qa-run.swarm.${slug(input.target)}.${slug(input.id)}`;

const runVerdict = (job: Job, resultStatus: string | undefined): QaSwarmVerdict => {
  if (job.status === "failed") return "failed";
  if (job.status !== "succeeded") return "inconclusive";
  if (resultStatus === "pass") return "passed";
  if (resultStatus === "fail") return "failed";
  return "inconclusive";
};

const readRunStatus = (storeDir: string, jobId: string): string | undefined => {
  const path = join(storeDir, jobId, "result.json");
  if (!existsSync(path)) return undefined;
  const parsed = JSON.parse(readFileSync(path, "utf8")) as { status?: unknown };
  return typeof parsed.status === "string" ? parsed.status : undefined;
};

export const writeQaSwarmRunSummary = (input: Readonly<{
  artifactDir: string;
  childJobs: readonly Job[];
  generatedAt: string;
  includeLiveTiers: boolean;
  maxRuns: number;
  maxWorkers: number;
  proBaseUrl: string;
  runRef: string;
  storeDir: string;
  target: string;
  targetName: string;
  tokenBudget: number;
}>): QaSwarmRunSummary => {
  const targetSlug = slug(input.targetName || input.target);
  const childRunIds = input.childJobs.map(job => job.id);
  const childVerdicts = input.childJobs.map(job =>
    runVerdict(job, readRunStatus(input.storeDir, job.id)),
  );
  const failedCount = childVerdicts.filter(verdict => verdict === "failed").length;
  const inconclusiveCount = childVerdicts.filter(verdict => verdict === "inconclusive").length;
  const fixtureVerdict: QaSwarmVerdict =
    failedCount > 0 ? "failed" : inconclusiveCount > 0 ? "inconclusive" : "passed";
  const observedVerdict: QaSwarmVerdict = fixtureVerdict;
  const blockerRefs = [
    "blocker.qa_swarm.receipts.not_resolved",
    "blocker.qa_swarm.artifacts.not_published",
    ...(input.includeLiveTiers ? ["blocker.qa_swarm.live_tiers.receipts_missing"] : []),
  ];
  const projectionEvidence: QaSwarmProjectionEvidence = {
    blockerRefs,
    coverageFrontier: [],
    distilledTests: [],
    generatedAt: input.generatedAt,
    opaqueTargetRefs: [],
    perfBudgets: [],
    projectionRef: `projection.qa_swarm.run.${targetSlug}.${slug(input.runRef)}`,
    publicSafetyRefs: [],
    runRef: input.runRef,
    schemaVersion: QA_SWARM_RUN_PROJECTION_SCHEMA,
    staleness: {
      contractVersion: "projection_staleness.v1",
      maxAgeHours: 24,
      mode: "artifact_snapshot",
    },
    target: {
      label: input.targetName,
      visibility: "opaque",
    },
    title: `QA Swarm hosted run: ${input.targetName}`,
    traceRefs: [],
    verdict: observedVerdict,
    verdictWall: [],
    videoRefs: [],
  };
  const resolverBacked = buildResolverBackedQaSwarmBoardGraph(projectionEvidence, {
    resolve: () => ({
      status: "unavailable",
      blockerRef: "blocker.qa_swarm.receipt_resolver.not_configured",
    }),
  });
  const projection = assertQaSwarmProjectionPublicSafe(
    assertResolverBackedQaSwarmProjection({
      ...projectionEvidence,
      ...resolverBacked,
      blockerRefs: resolverBacked.evidenceAdmission.blockerRefs,
    }),
  );

  const tiers: readonly QaSwarmRunTier[] = [
    ...input.childJobs.map(job => ({
      backend: "fixture" as const,
      jobId: job.id,
      status: job.status === "succeeded"
        ? readRunStatus(input.storeDir, job.id) === "pass" ? "passed" as const : "failed" as const
        : job.status === "failed" ? "failed" as const : "running" as const,
    })),
    {
      backend: "gce-tier-2",
      reason: input.includeLiveTiers
        ? "live tier requested; external runner receipt not attached in fixture composition"
        : "skip-safe fixture tier: set includeLiveTiers with an armed daemon for live evidence",
      status: "skipped",
    },
    {
      backend: "cf-browser-rendering",
      reason: input.includeLiveTiers
        ? "live tier requested; CF Browser Rendering receipt not attached in fixture composition"
        : "skip-safe fixture tier: set includeLiveTiers with an armed daemon for live evidence",
      status: "skipped",
    },
  ];

  const summary: QaSwarmRunSummary = {
    caps: {
      maxRuns: input.maxRuns,
      maxWorkers: input.maxWorkers,
      tokenBudget: input.tokenBudget,
    },
    childRunIds,
    projection,
    projectionPath: join(input.artifactDir, "qa-swarm-projection.json"),
    runRef: input.runRef,
    shareUrl: `${input.proBaseUrl}/qa/${input.runRef}`,
    tiers,
  };

  writeFileSync(summary.projectionPath, `${JSON.stringify(projection, null, 2)}\n`);
  writeFileSync(join(input.artifactDir, "qa-swarm-run.json"), `${JSON.stringify(summary, null, 2)}\n`);
  return summary;
};

export const readQaSwarmRunSummary = (artifactDir: string): QaSwarmRunSummary | null => {
  const path = join(artifactDir, "qa-swarm-run.json");
  if (!existsSync(path)) return null;
  return JSON.parse(readFileSync(path, "utf8")) as QaSwarmRunSummary;
};
