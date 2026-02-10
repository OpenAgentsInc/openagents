import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

/**
 * Convex-first MVP schema for Autopilot chat (threads/messages/messageParts).
 *
 * Canonical store:
 * - threads/messages/messageParts/receipts/blueprints live in Convex.
 * - Cloudflare Worker is compute/enforcement and writes chunked deltas.
 */
export default defineSchema({
  users: defineTable({
    userId: v.string(),
    email: v.optional(v.string()),
    createdAtMs: v.number(),
    defaultThreadId: v.optional(v.string()),
  }).index("by_userId", ["userId"]),

  threads: defineTable({
    threadId: v.string(),
    ownerId: v.optional(v.string()),
    anonKey: v.optional(v.string()),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index("by_threadId", ["threadId"])
    .index("by_ownerId", ["ownerId"]),

  messages: defineTable({
    threadId: v.string(),
    messageId: v.string(),
    role: v.union(v.literal("user"), v.literal("assistant"), v.literal("system")),
    status: v.union(
      v.literal("draft"),
      v.literal("streaming"),
      v.literal("final"),
      v.literal("error"),
      v.literal("canceled"),
    ),
    text: v.optional(v.string()),
    runId: v.optional(v.string()),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index("by_threadId_createdAtMs", ["threadId", "createdAtMs"])
    .index("by_threadId_messageId", ["threadId", "messageId"])
    .index("by_runId", ["runId"]),

  /**
   * Chunked streaming parts (idempotent by (runId, seq)).
   *
   * We store `@effect/ai/Response` StreamPartEncoded objects under `part`.
   */
  messageParts: defineTable({
    threadId: v.string(),
    runId: v.string(),
    messageId: v.string(),
    seq: v.number(),
    part: v.any(),
    createdAtMs: v.number(),
  })
    .index("by_runId_seq", ["runId", "seq"])
    .index("by_messageId_seq", ["messageId", "seq"])
    .index("by_threadId_createdAtMs", ["threadId", "createdAtMs"]),

  runs: defineTable({
    threadId: v.string(),
    runId: v.string(),
    assistantMessageId: v.string(),
    status: v.union(
      v.literal("streaming"),
      v.literal("final"),
      v.literal("error"),
      v.literal("canceled"),
    ),
    cancelRequested: v.boolean(),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index("by_runId", ["runId"])
    .index("by_threadId_updatedAtMs", ["threadId", "updatedAtMs"]),

  blueprints: defineTable({
    threadId: v.string(),
    blueprint: v.any(),
    updatedAtMs: v.number(),
  }).index("by_threadId", ["threadId"]),

  receipts: defineTable({
    threadId: v.string(),
    runId: v.string(),
    kind: v.union(v.literal("model"), v.literal("tool"), v.literal("dse.predict")),
    json: v.any(),
    // Optional receipt metadata (used by DSE predict receipts).
    receiptId: v.optional(v.string()),
    signatureId: v.optional(v.string()),
    compiled_id: v.optional(v.string()),
    createdAtMs: v.number(),
  })
    .index("by_runId_createdAtMs", ["runId", "createdAtMs"])
    .index("by_threadId_createdAtMs", ["threadId", "createdAtMs"])
    .index("by_signatureId_createdAtMs", ["signatureId", "createdAtMs"])
    .index("by_receiptId", ["receiptId"]),

  /**
   * DSE per-run BlobStore + VarSpace persistence.
   *
   * These back RLM-lite execution in Workers while keeping token space bounded.
   * Scope: (threadId, runId).
   */
  dseBlobs: defineTable({
    threadId: v.string(),
    runId: v.string(),
    blobId: v.string(),
    mime: v.optional(v.string()),
    text: v.string(),
    size: v.number(),
    createdAtMs: v.number(),
  }).index("by_threadId_runId_blobId", ["threadId", "runId", "blobId"]),

  dseVarSpace: defineTable({
    threadId: v.string(),
    runId: v.string(),
    name: v.string(),
    kind: v.union(v.literal("json"), v.literal("blob")),
    json: v.optional(v.any()),
    approxChars: v.optional(v.number()),
    blob: v.optional(v.any()),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index("by_threadId_runId_name", ["threadId", "runId", "name"])
    .index("by_threadId_runId_updatedAtMs", ["threadId", "runId", "updatedAtMs"]),

  /**
   * DSE compiled artifact store and active pointer registry (global, not per-thread).
   *
   * These tables back the `PolicyRegistryService` used by DSE `Predict`.
   */
  dseArtifacts: defineTable({
    signatureId: v.string(),
    compiled_id: v.string(),
    json: v.any(),
    createdAtMs: v.number(),
  })
    .index("by_signatureId_compiled_id", ["signatureId", "compiled_id"])
    .index("by_signatureId_createdAtMs", ["signatureId", "createdAtMs"]),

  dseActiveArtifacts: defineTable({
    signatureId: v.string(),
    compiled_id: v.string(),
    updatedAtMs: v.number(),
  }).index("by_signatureId", ["signatureId"]),

  dseActiveArtifactHistory: defineTable({
    signatureId: v.string(),
    action: v.union(v.literal("set"), v.literal("clear"), v.literal("rollback")),
    fromCompiledId: v.optional(v.string()),
    toCompiledId: v.optional(v.string()),
    reason: v.optional(v.string()),
    actorUserId: v.optional(v.string()),
    createdAtMs: v.number(),
  }).index("by_signatureId_createdAtMs", ["signatureId", "createdAtMs"]),

  /**
   * DSE labeled examples dataset (global, not per-thread).
   *
   * This is the minimal storage needed to run evaluation/compile jobs in the
   * Convex-first MVP without Durable Objects.
   */
  dseExamples: defineTable({
    signatureId: v.string(),
    exampleId: v.string(),
    inputJson: v.any(),
    expectedJson: v.any(),
    // "dev" was the initial name; "holdout" matches the DSE compile terminology.
    split: v.optional(v.union(v.literal("train"), v.literal("dev"), v.literal("holdout"), v.literal("test"))),
    tags: v.optional(v.array(v.string())),
    source: v.optional(v.string()),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index("by_signatureId_exampleId", ["signatureId", "exampleId"])
    .index("by_signatureId_updatedAtMs", ["signatureId", "updatedAtMs"]),

  /**
   * DSE compile run reports (global, not per-thread).
   *
   * Reports are immutable and keyed by (signatureId, jobHash, datasetHash).
   * This makes compile runs replayable/auditable without a DO-backed store.
   */
  dseCompileReports: defineTable({
    signatureId: v.string(),
    jobHash: v.string(),
    datasetId: v.string(),
    datasetHash: v.string(),
    compiled_id: v.string(),
    json: v.any(),
    createdAtMs: v.number(),
  })
    .index("by_signatureId_jobHash_datasetHash", ["signatureId", "jobHash", "datasetHash"])
    .index("by_signatureId_createdAtMs", ["signatureId", "createdAtMs"]),

  /**
   * DSE canary rollout config (global, not per-thread).
   *
   * Used by the Worker `PolicyRegistryService` to select between control and
   * canary compiled artifacts deterministically per thread.
   */
  dseCanaries: defineTable({
    signatureId: v.string(),
    control_compiled_id: v.string(),
    canary_compiled_id: v.string(),
    rolloutPct: v.number(), // 0..100
    salt: v.string(),
    enabled: v.boolean(),
    // Basic health counters to support MVP auto-stop behavior.
    okCount: v.number(),
    errorCount: v.number(),
    minSamples: v.number(),
    maxErrorRate: v.number(), // 0..1
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  }).index("by_signatureId", ["signatureId"]),

  dseCanaryHistory: defineTable({
    signatureId: v.string(),
    action: v.union(v.literal("start"), v.literal("stop"), v.literal("auto_stop"), v.literal("update")),
    control_compiled_id: v.optional(v.string()),
    canary_compiled_id: v.optional(v.string()),
    rolloutPct: v.optional(v.number()),
    okCount: v.optional(v.number()),
    errorCount: v.optional(v.number()),
    reason: v.optional(v.string()),
    actorUserId: v.optional(v.string()),
    createdAtMs: v.number(),
  }).index("by_signatureId_createdAtMs", ["signatureId", "createdAtMs"]),

  /**
   * DSE overnight ops runs (global, admin-only).
   *
   * These records exist so agents can run the improvement loop headlessly and
   * persist a single “what happened” trail in Convex for later visualization.
   */
  dseOpsRuns: defineTable({
    runId: v.string(),
    status: v.union(v.literal("running"), v.literal("finished"), v.literal("failed")),
    startedAtMs: v.number(),
    endedAtMs: v.optional(v.number()),
    // Metadata (all optional; keep bounded).
    commitSha: v.optional(v.string()),
    baseUrl: v.optional(v.string()),
    actorUserId: v.optional(v.string()),
    signatureIds: v.optional(v.array(v.string())),
    notes: v.optional(v.string()),
    links: v.optional(v.any()),
    summaryJson: v.optional(v.any()),
    createdAtMs: v.number(),
    updatedAtMs: v.number(),
  })
    .index("by_runId", ["runId"])
    .index("by_createdAtMs", ["createdAtMs"]),

  dseOpsRunEvents: defineTable({
    runId: v.string(),
    tsMs: v.number(),
    level: v.union(v.literal("info"), v.literal("warn"), v.literal("error")),
    phase: v.optional(v.string()),
    message: v.string(),
    json: v.optional(v.any()),
    createdAtMs: v.number(),
  }).index("by_runId_createdAtMs", ["runId", "createdAtMs"]),
});
