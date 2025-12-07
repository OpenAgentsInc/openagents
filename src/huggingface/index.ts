/**
 * HuggingFace Dataset Service
 *
 * Download and manage HuggingFace datasets for Effuse UI ingestion.
 */

// Schema types
export {
  type HFDatasetConfig,
  type DownloadedDataset,
  type DatasetIndex,
  type OpenThoughtsSftRow,
  HFDatasetError,
  DEFAULT_DATASETS_DIR,
  DATASETS_INDEX_FILE,
  OPENTHOUGHTS_SFT_CONFIG,
} from "./schema.js";

// Core download service
export {
  type IHFDatasetService,
  type HFDatasetServiceConfig,
  HFDatasetService,
  HFDatasetServiceLive,
  makeHFDatasetService,
} from "./service.js";

// Parquet utilities
export {
  type ParquetReadOptions,
  readParquetFile,
  getParquetRowCount,
  getParquetSchema,
  streamParquetRows,
} from "./parquet.js";

// OpenThoughts adapter
export {
  type IOpenThoughtsService,
  OpenThoughtsService,
  OpenThoughtsServiceLive,
  makeOpenThoughtsService,
  sftRowToTrajectory,
} from "./openthoughts.js";
