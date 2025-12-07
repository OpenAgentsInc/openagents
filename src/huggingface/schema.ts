/**
 * HuggingFace Dataset Service Schema
 *
 * Types for downloading and managing HuggingFace datasets.
 */

// ============================================================================
// Configuration Types
// ============================================================================

export interface HFDatasetConfig {
  /** Repository in format "owner/name" (e.g. "open-thoughts/OpenThoughts-Agent-v1-SFT") */
  repo: string;
  /** Branch or commit hash (default: "main") */
  revision?: string;
  /** HuggingFace access token (optional for public datasets) */
  accessToken?: string;
  /** Glob pattern for files to download (default: all parquet files) */
  filePattern?: string;
}

export interface DownloadedDataset {
  /** Repository in format "owner/name" */
  repo: string;
  /** Local path where dataset is stored */
  localPath: string;
  /** List of downloaded file paths (relative to localPath) */
  files: string[];
  /** Total bytes downloaded */
  totalBytes: number;
  /** ISO timestamp of when download completed */
  downloadedAt: string;
  /** Revision that was downloaded */
  revision: string;
}

export interface DatasetIndex {
  /** Map of repo -> download info */
  datasets: Record<string, DownloadedDataset>;
  /** Last updated timestamp */
  updatedAt: string;
}

// ============================================================================
// Error Types
// ============================================================================

export class HFDatasetError extends Error {
  readonly _tag = "HFDatasetError";

  constructor(
    readonly reason:
      | "network_error"
      | "not_found"
      | "parse_error"
      | "write_error"
      | "auth_required"
      | "rate_limited"
      | "invalid_config",
    override readonly message: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "HFDatasetError";
  }
}

// ============================================================================
// OpenThoughts SFT Row Type
// ============================================================================

/**
 * Row from OpenThoughts-Agent-v1-SFT parquet dataset.
 * Based on schema from https://huggingface.co/datasets/open-thoughts/OpenThoughts-Agent-v1-SFT
 */
export interface OpenThoughtsSftRow {
  /** Conversation history */
  conversations: Array<{
    content: string;
    role: "user" | "assistant";
  }>;
  /** Task identifier */
  task: string;
  /** Episode identifier */
  episode: string;
  /** Unique run identifier */
  run_id: string;
  /** Trial name */
  trial_name: string;
  /** Agent name */
  agent: string;
  /** Model name */
  model: string;
  /** Model provider */
  model_provider: string;
  /** ISO date string */
  date: string;
}

// ============================================================================
// Constants
// ============================================================================

export const DEFAULT_DATASETS_DIR = ".openagents/datasets";
export const DATASETS_INDEX_FILE = "index.json";

/** OpenThoughts SFT dataset config */
export const OPENTHOUGHTS_SFT_CONFIG: HFDatasetConfig = {
  repo: "open-thoughts/OpenThoughts-Agent-v1-SFT",
  revision: "main",
  filePattern: ".parquet",
};
