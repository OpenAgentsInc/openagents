import { CanonicalJson, CompileJob, EvalMetric, EvalReward } from "@openagentsinc/dse";

import { signatures as dseCatalogSignatures } from "../../../autopilot-worker/src/dseCatalog";

import { THREAD_SUMMARY_JUDGE_ARTIFACT_V1 } from "./dsePinnedArtifacts";

export const SELECT_TOOL_SIGNATURE_ID = "@openagents/autopilot/blueprint/SelectTool.v1";
export const RECAP_THREAD_SIGNATURE_ID = "@openagents/autopilot/canary/RecapThread.v1";
export const SUMMARIZE_THREAD_SIGNATURE_ID = "@openagents/autopilot/rlm/SummarizeThread.v1";

export const convexDatasetIdForExamples = (signatureId: string): string => `convex:dseExamples:${signatureId}`;

export const rewardExactJsonMatch = (): EvalReward.RewardBundle<any, any, any> => {
  const metric = EvalMetric.deterministic<any, any>({
    metricId: "exact_json_match.v1",
    metricVersion: 1,
    score: (pred, expected) => (CanonicalJson.canonicalJson(pred) === CanonicalJson.canonicalJson(expected) ? 1 : 0),
    notes: (pred, expected) =>
      CanonicalJson.canonicalJson(pred) === CanonicalJson.canonicalJson(expected) ? undefined : "mismatch",
  });

  return EvalReward.makeBundle({
    rewardId: "reward_exact_json_match.v1",
    rewardVersion: 1,
    signals: [
      EvalReward.signalFormatValidity({ weight: 0.2 }),
      EvalReward.signalMetric(metric, { weight: 0.8, signalId: "exact_json_match.signal.v1" }),
    ],
  });
};

export const rewardThreadSummaryJudge = (): EvalReward.RewardBundle<any, any, any> => {
  const judgeSig = dseCatalogSignatures.judge_thread_summary_quality;

  const metric = EvalMetric.judge<any, any, any, any, any>({
    metricId: "thread_summary_judge.v1",
    metricVersion: 1,
    judgeSignature: judgeSig,
    judgeArtifact: THREAD_SUMMARY_JUDGE_ARTIFACT_V1,
    buildJudgeInput: ({ input, pred, expected }) => ({
      question: String((input as any)?.question ?? ""),
      predSummary: String((pred as any)?.summary ?? ""),
      expectedSummary: String((expected as any)?.summary ?? ""),
    }),
    scoreFromJudgeOutput: (o) => Number((o as any)?.score ?? 0),
    notesFromJudgeOutput: (o) => (typeof (o as any)?.notes === "string" ? String((o as any).notes) : undefined),
  });

  return EvalReward.makeBundle({
    rewardId: "reward_thread_summary_judge.v1",
    rewardVersion: 1,
    signals: [
      EvalReward.signalFormatValidity({ weight: 0.2 }),
      EvalReward.signalMetric(metric, { weight: 0.8, signalId: "thread_summary_judge.signal.v1" }),
      // Phase 9: make budget profiles meaningfully selectable by including a small cost/latency penalty.
      EvalReward.signalPredictCostPenalty({
        weight: 0.05,
        targetDurationMs: 800,
        targetLmCalls: 2,
        targetToolCalls: 0,
      }),
    ],
  });
};

const selectToolInstructionVariants = (): ReadonlyArray<CompileJob.InstructionVariantV1> => [
  {
    id: "v1.base_router_rules",
    text:
      "Decide whether the user's message requires a Blueprint update tool.\n" +
      "\n" +
      "Output MUST be JSON only.\n" +
      "\n" +
      "Choose action=tool ONLY when the user is explicitly asking to change Autopilot's Blueprint.\n" +
      "\n" +
      "Tool selection:\n" +
      "- identity_update: changes to Autopilot identity/name/vibe/voice/formatting preferences.\n" +
      "- user_update: changes to what Autopilot calls the user (handle/nickname).\n" +
      "- character_update: changes to boundaries, safety rules, persistent constraints.\n" +
      "- blueprint_export: user asks to export/show/download the Blueprint.\n" +
      "\n" +
      "Otherwise, action=none.\n" +
      "\n" +
      "Never select a tool for personal info requests (address, email, phone, legal name, etc.).\n" +
      "Pick exactly one tool when action=tool.",
  },
  {
    id: "v1.none_by_default_strict",
    text:
      "Route the message.\n" +
      "\n" +
      "Default: action=none.\n" +
      "Only use action=tool when the user is clearly requesting a Blueprint change or Blueprint export.\n" +
      "\n" +
      "If action=tool, pick one:\n" +
      "- identity_update (name/vibe/voice/style)\n" +
      "- user_update (what to call the user)\n" +
      "- character_update (boundaries/rules)\n" +
      "- blueprint_export (export/show the Blueprint)\n" +
      "\n" +
      "Do not use tools for normal questions, tasks, or chit-chat.\n" +
      "Never use tools for personal info requests.\n" +
      "JSON only.",
  },
  {
    id: "v1.boundaries_vs_identity",
    text:
      "Classify whether to run a Blueprint update tool.\n" +
      "\n" +
      "Guidance:\n" +
      "- identity_update: how Autopilot speaks/behaves stylistically (tone, vibe, verbosity, formatting), and what it calls itself.\n" +
      "- character_update: hard rules and boundaries (what Autopilot will/won't do, safety constraints, 'never ask X').\n" +
      "- user_update: what to call the user.\n" +
      "- blueprint_export: export/show/download the blueprint.\n" +
      "\n" +
      "If the user is NOT asking for a Blueprint change/export, return {\"action\":\"none\"}.\n" +
      "\n" +
      "JSON only; pick exactly one tool when action=tool; never select a tool for personal info requests.",
  },
  {
    id: "v1_common_phrasings",
    text:
      "Route intent to Blueprint tool selection.\n" +
      "\n" +
      "Examples of tool triggers:\n" +
      "- \"Call me X\" => user_update\n" +
      "- \"Rename yourself to X\" / \"Use the name X\" => identity_update\n" +
      "- \"New rule/boundary: ...\" / \"Never ask me for ...\" => character_update\n" +
      "- \"Export/show/download my blueprint\" => blueprint_export\n" +
      "\n" +
      "If it's anything else, action=none.\n" +
      "Never select a tool for personal info requests.\n" +
      "Output JSON only.",
  },
];

const recapInstructionVariants = (): ReadonlyArray<CompileJob.InstructionVariantV1> => [
  {
    id: "v1.base_bullets",
    text:
      "Goal: recap prior context in this thread relevant to Input.question.\n" +
      "\n" +
      "Rules:\n" +
      "- Ground your recap only in the provided thread context.\n" +
      "- Do NOT invent details.\n" +
      "- Prefer short bullets.\n" +
      "- Keep it short and actionable.\n" +
      "\n" +
      "Output JSON { summary } with summary <= 1200 chars.",
  },
  {
    id: "v1.hallucination_penalty",
    text:
      "Recap relevant prior context.\n" +
      "\n" +
      "Hard rules:\n" +
      "- If you are not sure a detail is in the provided thread context, do NOT include it.\n" +
      "- Prefer 'unknown' over guessing.\n" +
      "- Keep it terse (bullets).\n" +
      "\n" +
      "Output JSON only: { summary } where summary <= 1200 chars.",
  },
];

const recapRlmControllerInstructionVariants = (): ReadonlyArray<CompileJob.InstructionVariantV1> => [
  {
    id: "v1.json_strict_and_budgeted",
    text:
      "Controller goals:\n" +
      "- Keep token space bounded; use RLM ops to inspect BlobRefs.\n" +
      "- Use extract_over_chunks for bulk extraction (avoid O(N) sub_lm calls).\n" +
      "- When ready, produce Final with {summary} only.\n" +
      "\n" +
      "Hard rules:\n" +
      "- Output must be valid JSON matching the action schema.\n" +
      "- If you cannot find support in the blobs, omit the detail (prefer 'unknown').",
  },
  {
    id: "v1.search_first_then_extract",
    text:
      "Preferred approach:\n" +
      "1) Search across likely blobs for question keywords.\n" +
      "2) Preview only the most relevant hits.\n" +
      "3) If needed, ExtractOverChunks over a small set of chunk refs.\n" +
      "4) Final.\n" +
      "\n" +
      "Avoid scanning everything unless required by the question.",
  },
];

const recapRlmChunkingPolicyVariants = (): ReadonlyArray<CompileJob.ChunkingPolicyVariantV1> => [
  { id: "v1.small", chunkChars: 2_000, overlapChars: 200, maxChunks: 40 },
  { id: "v1.medium", chunkChars: 4_000, overlapChars: 300, maxChunks: 60 },
];

const recapRlmSubRoleVariants = (): ReadonlyArray<CompileJob.SubRoleVariantV1> => [
  { id: "v1.sub", subRole: "sub" },
  { id: "v1.main", subRole: "main" },
];

const recapBudgetProfiles = (): ReadonlyArray<CompileJob.BudgetProfileV1> => [
  {
    id: "v1.fast",
    budgets: {
      maxTimeMs: 10_000,
      maxLmCalls: 20,
      maxOutputChars: 80_000,
      maxRlmIterations: 6,
      maxSubLmCalls: 12,
    },
  },
  {
    id: "v1.default",
    budgets: {
      maxTimeMs: 15_000,
      maxLmCalls: 40,
      maxOutputChars: 120_000,
      maxRlmIterations: 10,
      maxSubLmCalls: 20,
    },
  },
];

export type DseCompileJob = {
  readonly jobSpec: CompileJob.CompileJobSpecV1;
  readonly reward: EvalReward.RewardBundle<any, any, any>;
  readonly searchSpace: CompileJob.CompileSearchSpaceV1;
  readonly optimizer: CompileJob.CompileOptimizerV1;
};

export const compileJobForSignature = (input: { readonly signatureId: string; readonly datasetId: string }): DseCompileJob => {
  let reward: EvalReward.RewardBundle<any, any, any> = rewardExactJsonMatch();

  let searchSpace: CompileJob.CompileSearchSpaceV1 = {};
  let optimizer: CompileJob.CompileOptimizerV1 = { id: "instruction_grid.v1" };

  if (input.signatureId === SELECT_TOOL_SIGNATURE_ID) {
    // Overnight: make SelectTool compile non-trivial by evaluating multiple instruction variants.
    searchSpace = { instructionVariants: selectToolInstructionVariants() };
    optimizer = { id: "instruction_grid.v1" };
  }
  if (input.signatureId === RECAP_THREAD_SIGNATURE_ID || input.signatureId === SUMMARIZE_THREAD_SIGNATURE_ID) {
    // Phase 7: judge-based reward (subjective output). Start with a small instruction grid.
    reward = rewardThreadSummaryJudge();
    searchSpace = {
      // Keep a small instruction grid for the authored instruction block.
      instructionVariants: recapInstructionVariants(),
      // Phase 9: RLM-lite compile knobs (controller, chunking, sub-role, budgets).
      // Only include rlm_lite strategy as a variant so the base direct.v1 params remain
      // a single baseline candidate (avoid expanding direct across RLM-only knobs).
      strategyVariants: [{ id: "v1.rlm_lite", strategyId: "rlm_lite.v1" }],
      rlmControllerInstructionVariants: recapRlmControllerInstructionVariants(),
      rlmChunkingPolicyVariants: recapRlmChunkingPolicyVariants(),
      rlmSubRoleVariants: recapRlmSubRoleVariants(),
      budgetProfiles: recapBudgetProfiles(),
    };
    optimizer = { id: "knobs_grid_refine.v1", config: { maxCandidates: 96 } };
  }

  const jobSpec: CompileJob.CompileJobSpecV1 = {
    format: "openagents.dse.compile_job",
    formatVersion: 1,
    signatureId: input.signatureId,
    datasetId: input.datasetId,
    metricId: reward.rewardId,
    searchSpace,
    optimizer,
  };

  return { jobSpec, reward, searchSpace, optimizer };
};
