/**
 * HFDatasetService - Generic HuggingFace Dataset Download Service
 *
 * Downloads datasets from HuggingFace Hub to local storage.
 * Follows Effect service patterns from src/atif/service.ts
 */

import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Context, Effect, Layer } from "effect";
import { listFiles, downloadFile } from "@huggingface/hub";
import type { ListFileEntry } from "@huggingface/hub";
import {
  type HFDatasetConfig,
  type DownloadedDataset,
  type DatasetIndex,
  HFDatasetError,
  DEFAULT_DATASETS_DIR,
  DATASETS_INDEX_FILE,
} from "./schema.js";

// ============================================================================
// Service Interface
// ============================================================================

export interface IHFDatasetService {
  /** Download dataset files to local storage */
  download(config: HFDatasetConfig): Effect.Effect<DownloadedDataset, HFDatasetError>;

  /** Check if dataset is already downloaded */
  isDownloaded(repo: string): Effect.Effect<boolean, HFDatasetError>;

  /** Get download info for a dataset */
  getDownloadInfo(repo: string): Effect.Effect<DownloadedDataset | null, HFDatasetError>;

  /** Get local path for a dataset */
  getLocalPath(repo: string): Effect.Effect<string | null, HFDatasetError>;

  /** List all downloaded datasets */
  listDownloaded(): Effect.Effect<DownloadedDataset[], HFDatasetError>;

  /** Delete a downloaded dataset */
  delete(repo: string): Effect.Effect<void, HFDatasetError>;

  /** Get the base datasets directory */
  getDatasetsDir(): string;
}

// ============================================================================
// Service Tag
// ============================================================================

export class HFDatasetService extends Context.Tag("HFDatasetService")<
  HFDatasetService,
  IHFDatasetService
>() {}

// ============================================================================
// Configuration
// ============================================================================

export interface HFDatasetServiceConfig {
  /** Base directory for datasets (default: .openagents/datasets) */
  datasetsDir?: string;
  /** Default access token (can be overridden per-download) */
  accessToken?: string;
}

// ============================================================================
// Implementation
// ============================================================================

export const makeHFDatasetService = (config: HFDatasetServiceConfig = {}) =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const pathService = yield* Path.Path;

    const datasetsDir = config.datasetsDir ?? DEFAULT_DATASETS_DIR;
    const defaultToken = config.accessToken ?? process.env.HF_TOKEN;

    // Ensure base directory exists
    yield* fs.makeDirectory(datasetsDir, { recursive: true }).pipe(
      Effect.catchAll(() => Effect.void),
    );

    /**
     * Get path to index file
     */
    const getIndexPath = (): string =>
      pathService.join(datasetsDir, DATASETS_INDEX_FILE);

    /**
     * Load dataset index
     */
    const loadIndex = (): Effect.Effect<DatasetIndex, HFDatasetError> =>
      Effect.gen(function* () {
        const indexPath = getIndexPath();
        const exists = yield* fs.exists(indexPath).pipe(
          Effect.catchAll(() => Effect.succeed(false)),
        );

        if (!exists) {
          return { datasets: {}, updatedAt: new Date().toISOString() };
        }

        const content = yield* fs.readFileString(indexPath).pipe(
          Effect.mapError(
            (e) => new HFDatasetError("parse_error", `Failed to read index: ${e.message}`),
          ),
        );

        try {
          return JSON.parse(content) as DatasetIndex;
        } catch (e) {
          return yield* Effect.fail(
            new HFDatasetError("parse_error", `Failed to parse index: ${e}`),
          );
        }
      });

    /**
     * Save dataset index
     */
    const saveIndex = (index: DatasetIndex): Effect.Effect<void, HFDatasetError> =>
      Effect.gen(function* () {
        const indexPath = getIndexPath();
        const content = JSON.stringify(index, null, 2);

        yield* fs.writeFileString(indexPath, content).pipe(
          Effect.mapError(
            (e) => new HFDatasetError("write_error", `Failed to save index: ${e.message}`),
          ),
        );
      });

    /**
     * Get local directory path for a repo
     */
    const getRepoDir = (repo: string): string => {
      const [owner, name] = repo.split("/");
      return pathService.join(datasetsDir, owner ?? repo, name ?? "");
    };

    /**
     * Match file path against pattern (simple suffix/contains check)
     */
    const matchesPattern = (filePath: string, pattern: string): boolean => {
      if (!pattern) return true;

      // Simple suffix matching for file extensions
      if (pattern.startsWith(".")) {
        return filePath.endsWith(pattern);
      }

      // Contains matching
      return filePath.includes(pattern);
    };

    const service: IHFDatasetService = {
      getDatasetsDir: () => datasetsDir,

      download: (downloadConfig) =>
        Effect.gen(function* () {
          const { repo, revision = "main", filePattern = "**/*.parquet" } = downloadConfig;
          const accessToken = downloadConfig.accessToken ?? defaultToken;

          // Create repo directory
          const repoDir = getRepoDir(repo);
          yield* fs.makeDirectory(repoDir, { recursive: true }).pipe(
            Effect.mapError(
              (e) => new HFDatasetError("write_error", `Failed to create directory: ${e.message}`),
            ),
          );

          // List files in the repository
          const files = yield* Effect.tryPromise({
            try: async () => {
              const result: ListFileEntry[] = [];
              for await (const file of listFiles({
                repo: { type: "dataset", name: repo },
                revision,
                credentials: accessToken ? { accessToken } : undefined,
                recursive: true,
              })) {
                if (file.type === "file" && matchesPattern(file.path, filePattern)) {
                  result.push(file);
                }
              }
              return result;
            },
            catch: (e) => {
              const error = e as Error;
              if (error.message?.includes("401") || error.message?.includes("403")) {
                return new HFDatasetError("auth_required", `Authentication required for ${repo}`);
              }
              if (error.message?.includes("404")) {
                return new HFDatasetError("not_found", `Dataset not found: ${repo}`);
              }
              if (error.message?.includes("429")) {
                return new HFDatasetError("rate_limited", `Rate limited. Try again later.`);
              }
              return new HFDatasetError("network_error", `Failed to list files: ${error.message}`, e);
            },
          });

          if (files.length === 0) {
            return yield* Effect.fail(
              new HFDatasetError(
                "not_found",
                `No files matching pattern "${filePattern}" found in ${repo}`,
              ),
            );
          }

          // Download each file
          const downloadedFiles: string[] = [];
          let totalBytes = 0;

          for (const file of files) {
            const localFilePath = pathService.join(repoDir, file.path);
            const localFileDir = pathService.dirname(localFilePath);

            // Create subdirectory if needed
            yield* fs.makeDirectory(localFileDir, { recursive: true }).pipe(
              Effect.catchAll(() => Effect.void),
            );

            // Download file
            console.log(`Downloading ${file.path} (${formatBytes(file.size ?? 0)})...`);

            const buffer = yield* Effect.tryPromise({
              try: async () => {
                const response = await downloadFile({
                  repo: { type: "dataset", name: repo },
                  path: file.path,
                  revision,
                  credentials: accessToken ? { accessToken } : undefined,
                });

                if (!response) {
                  throw new Error("No response");
                }

                const arrayBuffer = await response.arrayBuffer();
                return new Uint8Array(arrayBuffer);
              },
              catch: (e) => {
                const error = e as Error;
                return new HFDatasetError(
                  "network_error",
                  `Failed to download ${file.path}: ${error.message}`,
                  e,
                );
              },
            });

            yield* fs.writeFile(localFilePath, buffer).pipe(
              Effect.mapError(
                (e) =>
                  new HFDatasetError("write_error", `Failed to write ${file.path}: ${e.message}`),
              ),
            );

            downloadedFiles.push(file.path);
            totalBytes += buffer.length;

            console.log(`  Saved to ${localFilePath}`);
          }

          // Create download record
          const downloadInfo: DownloadedDataset = {
            repo,
            localPath: repoDir,
            files: downloadedFiles,
            totalBytes,
            downloadedAt: new Date().toISOString(),
            revision,
          };

          // Update index
          const index = yield* loadIndex();
          index.datasets[repo] = downloadInfo;
          index.updatedAt = new Date().toISOString();
          yield* saveIndex(index);

          console.log(
            `\nDownload complete: ${downloadedFiles.length} files, ${formatBytes(totalBytes)}`,
          );

          return downloadInfo;
        }),

      isDownloaded: (repo) =>
        Effect.gen(function* () {
          const index = yield* loadIndex();
          return repo in index.datasets;
        }),

      getDownloadInfo: (repo) =>
        Effect.gen(function* () {
          const index = yield* loadIndex();
          return index.datasets[repo] ?? null;
        }),

      getLocalPath: (repo) =>
        Effect.gen(function* () {
          const index = yield* loadIndex();
          const info = index.datasets[repo];
          return info?.localPath ?? null;
        }),

      listDownloaded: () =>
        Effect.gen(function* () {
          const index = yield* loadIndex();
          return Object.values(index.datasets);
        }),

      delete: (repo) =>
        Effect.gen(function* () {
          const index = yield* loadIndex();
          const info = index.datasets[repo];

          if (!info) {
            return yield* Effect.fail(
              new HFDatasetError("not_found", `Dataset not downloaded: ${repo}`),
            );
          }

          // Remove directory
          yield* fs.remove(info.localPath, { recursive: true }).pipe(
            Effect.mapError(
              (e) => new HFDatasetError("write_error", `Failed to delete: ${e.message}`),
            ),
          );

          // Update index
          delete index.datasets[repo];
          index.updatedAt = new Date().toISOString();
          yield* saveIndex(index);
        }),
    };

    return service;
  });

// ============================================================================
// Layer
// ============================================================================

export const HFDatasetServiceLive = (config: HFDatasetServiceConfig = {}) =>
  Layer.effect(HFDatasetService, makeHFDatasetService(config));

// ============================================================================
// Helpers
// ============================================================================

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}
