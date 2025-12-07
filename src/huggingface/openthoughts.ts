/**
 * OpenThoughts Service - Adapter for OpenThoughts SFT Dataset
 *
 * Downloads and parses the OpenThoughts-Agent-v1-SFT dataset,
 * converting rows to ATIF Trajectory format for Effuse UI ingestion.
 */

import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Context, Effect, Layer } from "effect";
import {
  type OpenThoughtsSftRow,
  HFDatasetError,
  OPENTHOUGHTS_SFT_CONFIG,
} from "./schema.js";
import { HFDatasetService } from "./service.js";
import {
  readParquetFile,
  getParquetRowCount,
  streamParquetRows,
} from "./parquet.js";
import {
  type Trajectory,
  type Step,
  ATIF_SCHEMA_VERSION,
} from "../atif/schema.js";

// ============================================================================
// Service Interface
// ============================================================================

export interface IOpenThoughtsService {
  /** Ensure dataset is downloaded, return local path */
  ensureDownloaded(): Effect.Effect<string, HFDatasetError, FileSystem.FileSystem>;

  /** Get total count of trajectories */
  count(): Effect.Effect<number, HFDatasetError, FileSystem.FileSystem>;

  /** Get a single trajectory by index */
  getTrajectory(index: number): Effect.Effect<Trajectory | null, HFDatasetError, FileSystem.FileSystem>;

  /** Get a trajectory by run_id */
  getTrajectoryByRunId(runId: string): Effect.Effect<Trajectory | null, HFDatasetError, FileSystem.FileSystem>;

  /** Get multiple trajectories with pagination */
  getTrajectories(
    offset?: number,
    limit?: number,
  ): Effect.Effect<Trajectory[], HFDatasetError, FileSystem.FileSystem>;

  /** Stream all trajectories (memory efficient) */
  streamTrajectories(): Effect.Effect<AsyncIterable<Trajectory>, HFDatasetError, FileSystem.FileSystem>;

  /** Get the parquet file path */
  getParquetPath(): Effect.Effect<string | null, HFDatasetError>;
}

// ============================================================================
// Service Tag
// ============================================================================

export class OpenThoughtsService extends Context.Tag("OpenThoughtsService")<
  OpenThoughtsService,
  IOpenThoughtsService
>() { }

// ============================================================================
// Conversion: SFT Row -> ATIF Trajectory
// ============================================================================

/**
 * Convert an OpenThoughts SFT row to an ATIF Trajectory
 */
export function sftRowToTrajectory(row: OpenThoughtsSftRow, index: number): Trajectory {
  // Generate session ID from run_id or task+episode
  const sessionId = row.run_id
    ? `openthoughts-${row.run_id}`
    : `openthoughts-${row.task}__${row.episode}__${index}`;

  // Convert conversations to ATIF steps
  // Note: Arrow Vector needs to be converted to array
  const conversations = Array.isArray(row.conversations)
    ? row.conversations
    : Array.from(row.conversations as Iterable<{ content: string; role: "user" | "assistant" }>);

  const steps: Step[] = conversations.map((msg, idx) => ({
    step_id: idx + 1,
    timestamp: row.date || new Date().toISOString(),
    source: msg.role === "assistant" ? "agent" : "user",
    message: msg.content,
  }));

  return {
    schema_version: ATIF_SCHEMA_VERSION,
    session_id: sessionId,
    agent: {
      name: row.agent || "unknown",
      version: "1.0",
      model_name: row.model || "unknown",
    },
    steps,
    extra: {
      source_dataset: "open-thoughts/OpenThoughts-Agent-v1-SFT",
      task: row.task,
      episode: row.episode,
      run_id: row.run_id,
      trial_name: row.trial_name,
      model_provider: row.model_provider,
      original_date: row.date,
    },
  };
}

// ============================================================================
// Implementation
// ============================================================================

export const makeOpenThoughtsService = () =>
  Effect.gen(function* () {
    const hfService = yield* HFDatasetService;
    const pathService = yield* Path.Path;

    const repo = OPENTHOUGHTS_SFT_CONFIG.repo;

    /**
     * Get path to the main parquet file
     */
    const getParquetFilePath = (): Effect.Effect<string | null, HFDatasetError> =>
      Effect.gen(function* () {
        const info = yield* hfService.getDownloadInfo(repo);
        if (!info) return null;

        // Find the parquet file
        const parquetFile = info.files.find((f) => f.endsWith(".parquet"));
        if (!parquetFile) return null;

        return pathService.join(info.localPath, parquetFile);
      });

    const service: IOpenThoughtsService = {
      ensureDownloaded: () =>
        Effect.gen(function* () {
          const isDownloaded = yield* hfService.isDownloaded(repo);

          if (!isDownloaded) {
            console.log(`Downloading OpenThoughts SFT dataset...`);
            yield* hfService.download(OPENTHOUGHTS_SFT_CONFIG);
          }

          const path = yield* getParquetFilePath();
          if (!path) {
            return yield* Effect.fail(
              new HFDatasetError(
                "not_found",
                "Dataset downloaded but parquet file not found",
              ),
            );
          }

          return path;
        }),

      count: () =>
        Effect.gen(function* () {
          const parquetPath = yield* service.ensureDownloaded();
          return yield* getParquetRowCount(parquetPath);
        }),

      getTrajectory: (index) =>
        Effect.gen(function* () {
          const parquetPath = yield* service.ensureDownloaded();
          const rows = yield* readParquetFile<OpenThoughtsSftRow>(parquetPath, {
            offset: index,
            limit: 1,
          });

          if (rows.length === 0) return null;
          return sftRowToTrajectory(rows[0]!, index);
        }),

      getTrajectoryByRunId: (runId) =>
        Effect.gen(function* () {
          const parquetPath = yield* service.ensureDownloaded();

          // Stream through to find matching run_id
          const result = yield* Effect.tryPromise({
            try: async () => {
              let index = 0;
              for await (const row of streamParquetRows<OpenThoughtsSftRow>(parquetPath)) {
                if (row.run_id === runId) {
                  return sftRowToTrajectory(row, index);
                }
                index++;
              }
              return null;
            },
            catch: (e) =>
              new HFDatasetError("parse_error", `Failed to search by run_id: ${e}`),
          });

          return result;
        }),

      getTrajectories: (offset = 0, limit = 100) =>
        Effect.gen(function* () {
          const parquetPath = yield* service.ensureDownloaded();
          const rows = yield* readParquetFile<OpenThoughtsSftRow>(parquetPath, {
            offset,
            limit,
          });

          return rows.map((row, idx) => sftRowToTrajectory(row, offset + idx));
        }),

      streamTrajectories: () =>
        Effect.gen(function* () {
          const parquetPath = yield* service.ensureDownloaded();

          async function* generator(): AsyncGenerator<Trajectory, void, unknown> {
            let index = 0;
            for await (const row of streamParquetRows<OpenThoughtsSftRow>(parquetPath)) {
              yield sftRowToTrajectory(row, index);
              index++;
            }
          }

          return generator();
        }),

      getParquetPath: () => getParquetFilePath(),
    };

    return service;
  });

// ============================================================================
// Layer
// ============================================================================

export const OpenThoughtsServiceLive = Layer.effect(
  OpenThoughtsService,
  makeOpenThoughtsService(),
);
