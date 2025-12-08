import { Database } from "bun:sqlite"
import { Context, Effect, Layer } from "effect"

import type { ChatRequest, ChatResponse } from "./openrouter-types.js";

/**
 * InferenceStoreError - Error type for inference store operations
 */
export class InferenceStoreError extends Error {
  readonly _tag = "InferenceStoreError";
  constructor(
    readonly reason: "connection" | "query" | "insert" | "not_found",
    override readonly message: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "InferenceStoreError";
  }
}

/**
 * InferenceRecord - Stored inference data
 */
export interface InferenceRecord {
  id: number;
  model: string;
  requestId: string | null;
  requestMessages: Array<{ role: string; content: string }>;
  requestOptions: Record<string, unknown> | null;
  responseData: ChatResponse & { usage?: any };
  responseId: string | null;
  responseModel: string | null;
  responseContent: string | null;
  promptTokens: number | null;
  completionTokens: number | null;
  totalTokens: number | null;
  costUsd: number | null;
  createdAt: string;
}

/**
 * InferenceStore - Service for storing and querying inference records
 */
export class InferenceStore extends Context.Tag("InferenceStore")<
  InferenceStore,
  {
    readonly db: Database;
    readonly save: (
      model: string,
      request: ChatRequest,
      response: ChatResponse & { usage?: any },
    ) => Effect.Effect<number, InferenceStoreError>; // Returns inserted ID
    readonly getById: (
      id: number,
    ) => Effect.Effect<InferenceRecord | null, InferenceStoreError>;
    readonly listByModel: (
      model: string,
      limit?: number,
    ) => Effect.Effect<InferenceRecord[], InferenceStoreError>;
    readonly listRecent: (
      limit?: number,
    ) => Effect.Effect<InferenceRecord[], InferenceStoreError>;
    readonly getStats: () => Effect.Effect<
      {
        total: number;
        totalCost: number;
        byModel: Record<string, { count: number; cost: number }>;
      },
      InferenceStoreError
    >;
  }
>() { }

/**
 * Helper: Extract usage metrics from response
 */
const extractUsage = (response: ChatResponse & { usage?: any }) => {
  const usage = response.usage;
  return {
    promptTokens: usage?.prompt_tokens ?? null,
    completionTokens: usage?.completion_tokens ?? null,
    totalTokens: usage?.total_tokens ?? null,
    costUsd: usage?.cost ?? null,
  };
};

/**
 * Helper: Extract response content for search
 */
const extractResponseContent = (response: ChatResponse): string | null => {
  const choice = response.choices[0];
  const message = choice?.message;
  const content = message?.content;
  return typeof content === "string" ? content : null;
};

/**
 * Helper: Convert DB row to InferenceRecord
 */
const rowToInference = (row: any): InferenceRecord => ({
  id: row.id,
  model: row.model,
  requestId: row.request_id ?? null,
  requestMessages: JSON.parse(row.request_messages),
  requestOptions: row.request_options ? JSON.parse(row.request_options) : null,
  responseData: JSON.parse(row.response_data),
  responseId: row.response_id ?? null,
  responseModel: row.response_model ?? null,
  responseContent: row.response_content ?? null,
  promptTokens: row.prompt_tokens ?? null,
  completionTokens: row.completion_tokens ?? null,
  totalTokens: row.total_tokens ?? null,
  costUsd: row.cost_usd ?? null,
  createdAt: row.created_at,
});

/**
 * InferenceStoreLive - Implementation of InferenceStore
 */
export const makeInferenceStoreLive = (
  dbPath: string,
): Layer.Layer<InferenceStore, InferenceStoreError> =>
  Layer.effect(
    InferenceStore,
    Effect.gen(function* () {
      // Ensure parent directory exists
      const path = require("node:path");
      const dir = path.dirname(dbPath);
      if (dir !== ".") {
        const fs = require("node:fs");
        fs.mkdirSync(dir, { recursive: true });
      }

      // Open database connection
      const db = yield* Effect.try({
        try: () => new Database(dbPath),
        catch: (e) =>
          new InferenceStoreError(
            "connection",
            `Failed to open database: ${e}`,
          ),
      });

      // Ensure table exists (run migration if needed)
      yield* Effect.try({
        try: () => {
          // Check if table exists
          const tableExists = db
            .prepare(
              "SELECT name FROM sqlite_master WHERE type='table' AND name='inferences'",
            )
            .get();

          if (!tableExists) {
            // Table doesn't exist, need to run migration
            // For now, we'll let the migration system handle it
            // But we can create it here as a fallback
            console.warn(
              "inferences table does not exist - ensure migrations are run",
            );
          }
        },
        catch: (e) =>
          new InferenceStoreError("query", `Failed to check table: ${e}`),
      });

      // Helper: Run SQL in a try-catch with error mapping
      const runSQL = <T>(fn: () => T): Effect.Effect<T, InferenceStoreError> =>
        Effect.try({
          try: fn,
          catch: (e) => new InferenceStoreError("query", String(e), e),
        });

      // Save inference
      const save = (
        model: string,
        request: ChatRequest,
        response: ChatResponse & { usage?: any },
      ): Effect.Effect<number, InferenceStoreError> =>
        runSQL(() => {
          const usage = extractUsage(response);
          const responseContent = extractResponseContent(response);

          const stmt = db.prepare(`
            INSERT INTO inferences (
              model, request_id, request_messages, request_options,
              response_data, response_id, response_model, response_content,
              prompt_tokens, completion_tokens, total_tokens, cost_usd
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          `);

          const result = stmt.run(
            model,
            response.id || null,
            JSON.stringify(request.messages),
            request.temperature !== undefined ||
              request.maxTokens !== undefined ||
              request.tools !== undefined ||
              request.toolChoice !== undefined
              ? JSON.stringify({
                temperature: request.temperature,
                maxTokens: request.maxTokens,
                tools: request.tools,
                toolChoice: request.toolChoice,
              })
              : null,
            JSON.stringify(response),
            response.id || null,
            response.model || null,
            responseContent,
            usage.promptTokens,
            usage.completionTokens,
            usage.totalTokens,
            usage.costUsd,
          );

          return result.lastInsertRowid as number;
        });

      // Get by ID
      const getById = (
        id: number,
      ): Effect.Effect<InferenceRecord | null, InferenceStoreError> =>
        runSQL(() => {
          const stmt = db.prepare("SELECT * FROM inferences WHERE id = ?");
          const row = stmt.get(id) as any;
          return row ? rowToInference(row) : null;
        });

      // List by model
      const listByModel = (
        model: string,
        limit: number = 100,
      ): Effect.Effect<InferenceRecord[], InferenceStoreError> =>
        runSQL(() => {
          const stmt = db.prepare(
            "SELECT * FROM inferences WHERE model = ? ORDER BY created_at DESC LIMIT ?",
          );
          const rows = stmt.all(model, limit) as any[];
          return rows.map(rowToInference);
        });

      // List recent
      const listRecent = (
        limit: number = 100,
      ): Effect.Effect<InferenceRecord[], InferenceStoreError> =>
        runSQL(() => {
          const stmt = db.prepare(
            "SELECT * FROM inferences ORDER BY created_at DESC LIMIT ?",
          );
          const rows = stmt.all(limit) as any[];
          return rows.map(rowToInference);
        });

      // Get stats
      const getStats = (): Effect.Effect<
        {
          total: number;
          totalCost: number;
          byModel: Record<string, { count: number; cost: number }>;
        },
        InferenceStoreError
      > =>
        runSQL(() => {
          // Total count
          const totalStmt = db.prepare("SELECT COUNT(*) as count FROM inferences");
          const totalRow = totalStmt.get() as { count: number };

          // Total cost
          const costStmt = db.prepare(
            "SELECT COALESCE(SUM(cost_usd), 0) as total_cost FROM inferences",
          );
          const costRow = costStmt.get() as { total_cost: number };

          // By model
          const modelStmt = db.prepare(`
            SELECT
              model,
              COUNT(*) as count,
              COALESCE(SUM(cost_usd), 0) as cost
            FROM inferences
            GROUP BY model
          `);
          const modelRows = modelStmt.all() as Array<{
            model: string;
            count: number;
            cost: number;
          }>;

          const byModel: Record<string, { count: number; cost: number }> = {};
          for (const row of modelRows) {
            byModel[row.model] = {
              count: row.count,
              cost: row.cost,
            };
          }

          return {
            total: totalRow.count,
            totalCost: costRow.total_cost,
            byModel,
          };
        });

      return {
        db,
        save,
        getById,
        listByModel,
        listRecent,
        getStats,
      };
    }),
  );

/**
 * Default inference store layer (uses .openagents/openagents.db)
 */
export const InferenceStoreLive = makeInferenceStoreLive(
  ".openagents/openagents.db",
);
