/**
 * Parquet Utilities
 *
 * Effect-friendly wrapper around parquet-wasm + apache-arrow for reading parquet files.
 */

import * as FileSystem from "@effect/platform/FileSystem";
import { Effect } from "effect";
import { readParquet } from "parquet-wasm";
import { tableFromIPC, type Table } from "apache-arrow";
import { HFDatasetError } from "./schema.js";

// ============================================================================
// Types
// ============================================================================

export interface ParquetReadOptions {
  /** Maximum number of rows to read (default: all) */
  limit?: number;
  /** Number of rows to skip (default: 0) */
  offset?: number;
}

// ============================================================================
// Internal Helpers
// ============================================================================

/**
 * Read parquet file into Arrow table
 */
const readParquetToArrow = (
  filePath: string,
): Effect.Effect<Table, HFDatasetError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    // Check file exists
    const exists = yield* fs.exists(filePath).pipe(
      Effect.catchAll(() => Effect.succeed(false)),
    );

    if (!exists) {
      return yield* Effect.fail(
        new HFDatasetError("not_found", `Parquet file not found: ${filePath}`),
      );
    }

    // Read file into buffer
    const fileContent = yield* fs.readFile(filePath).pipe(
      Effect.mapError(
        (e) => new HFDatasetError("parse_error", `Failed to read file: ${e.message}`),
      ),
    );

    // Parse parquet to Arrow table
    const table = yield* Effect.try({
      try: () => {
        const parquetTable = readParquet(new Uint8Array(fileContent));
        return tableFromIPC(parquetTable.intoIPCStream());
      },
      catch: (e) => {
        const error = e as Error;
        return new HFDatasetError("parse_error", `Failed to parse parquet: ${error.message}`, e);
      },
    });

    return table;
  });

// ============================================================================
// Reading Functions
// ============================================================================

/**
 * Read all rows from a parquet file
 */
export const readParquetFile = <T>(
  filePath: string,
  options: ParquetReadOptions = {},
): Effect.Effect<T[], HFDatasetError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const table = yield* readParquetToArrow(filePath);

    const { limit, offset = 0 } = options;
    const endIndex = limit !== undefined ? offset + limit : table.numRows;
    const rows: T[] = [];

    for (let i = offset; i < Math.min(endIndex, table.numRows); i++) {
      const row = table.get(i);
      if (row) {
        rows.push(row.toJSON() as T);
      }
    }

    return rows;
  });

/**
 * Get row count from a parquet file
 */
export const getParquetRowCount = (
  filePath: string,
): Effect.Effect<number, HFDatasetError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const table = yield* readParquetToArrow(filePath);
    return table.numRows;
  });

/**
 * Get parquet file schema/metadata
 */
export const getParquetSchema = (
  filePath: string,
): Effect.Effect<Record<string, unknown>, HFDatasetError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const table = yield* readParquetToArrow(filePath);
    return {
      numRows: table.numRows,
      numColumns: table.numCols,
      columns: table.schema.fields.map((f) => ({
        name: f.name,
        type: f.type.toString(),
        nullable: f.nullable,
      })),
    };
  });

/**
 * Stream rows from a parquet file (memory efficient for large files)
 * Note: Currently reads all into memory - for true streaming, use batched reads
 */
export async function* streamParquetRows<T>(
  filePath: string,
): AsyncGenerator<T, void, unknown> {
  const file = Bun.file(filePath);
  const buffer = new Uint8Array(await file.arrayBuffer());

  const parquetTable = readParquet(buffer);
  const table = tableFromIPC(parquetTable.intoIPCStream());

  for (let i = 0; i < table.numRows; i++) {
    const row = table.get(i);
    if (row) {
      yield row.toJSON() as T;
    }
  }
}
