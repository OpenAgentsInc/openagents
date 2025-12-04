/**
 * Terminal-Bench Output Buffer
 *
 * Aggregates token-by-token output into lines before emitting to HUD.
 * Flushes on newlines, size threshold, or time threshold.
 *
 * @module tbench-hud/output-buffer
 */

import type { TBOutputSource } from "../hud/protocol.js";

// ============================================================================
// Types
// ============================================================================

export interface TBOutputBuffer {
  readonly taskId: string;
  readonly source: TBOutputSource;
  buffer: string;
  lastFlushTime: number;
}

export interface BufferFlushOptions {
  /** Max characters before forcing flush (default: 500) */
  readonly sizeThreshold?: number;
  /** Max milliseconds before forcing flush (default: 500) */
  readonly timeThresholdMs?: number;
}

const DEFAULT_OPTIONS: Required<BufferFlushOptions> = {
  sizeThreshold: 500,
  timeThresholdMs: 500,
};

// ============================================================================
// Buffer Management
// ============================================================================

/**
 * Create a new output buffer for a task/source pair.
 */
export const createBuffer = (
  taskId: string,
  source: TBOutputSource
): TBOutputBuffer => ({
  taskId,
  source,
  buffer: "",
  lastFlushTime: Date.now(),
});

/**
 * Get or create a buffer for a task/source pair.
 */
export const getOrCreateBuffer = (
  buffers: Map<string, TBOutputBuffer>,
  taskId: string,
  source: TBOutputSource
): TBOutputBuffer => {
  const key = `${taskId}:${source}`;
  let buffer = buffers.get(key);
  if (!buffer) {
    buffer = createBuffer(taskId, source);
    buffers.set(key, buffer);
  }
  return buffer;
};

/**
 * Append a chunk to the buffer and flush complete lines.
 *
 * Lines are flushed when:
 * 1. A newline character is encountered (flush complete lines)
 * 2. Buffer exceeds size threshold (force flush)
 * 3. Time since last flush exceeds threshold (force flush)
 *
 * @param buffer - The buffer to append to
 * @param chunk - The text chunk to append
 * @param emit - Callback to emit flushed lines
 * @param options - Flush thresholds
 * @returns Number of lines flushed
 */
export const appendAndFlush = (
  buffer: TBOutputBuffer,
  chunk: string,
  emit: (line: string) => void,
  options?: BufferFlushOptions
): number => {
  const opts = { ...DEFAULT_OPTIONS, ...options };
  const now = Date.now();
  let flushedCount = 0;

  // Append chunk to buffer
  buffer.buffer += chunk;

  // Flush complete lines (split on newlines)
  const lines = buffer.buffer.split("\n");
  if (lines.length > 1) {
    // Emit all complete lines (all except the last one)
    for (let i = 0; i < lines.length - 1; i++) {
      const line = lines[i];
      if (line.length > 0) {
        emit(line);
        flushedCount++;
      }
    }
    // Keep only the incomplete last line
    buffer.buffer = lines[lines.length - 1];
    buffer.lastFlushTime = now;
  }

  // Force flush if buffer is too large or too old
  const timeSinceFlush = now - buffer.lastFlushTime;
  if (
    buffer.buffer.length > 0 &&
    (buffer.buffer.length >= opts.sizeThreshold || timeSinceFlush >= opts.timeThresholdMs)
  ) {
    emit(buffer.buffer);
    flushedCount++;
    buffer.buffer = "";
    buffer.lastFlushTime = now;
  }

  return flushedCount;
};

/**
 * Force flush any remaining content in the buffer.
 *
 * @param buffer - The buffer to flush
 * @param emit - Callback to emit flushed content
 * @returns true if content was flushed, false if buffer was empty
 */
export const forceFlush = (
  buffer: TBOutputBuffer,
  emit: (line: string) => void
): boolean => {
  if (buffer.buffer.length > 0) {
    emit(buffer.buffer);
    buffer.buffer = "";
    buffer.lastFlushTime = Date.now();
    return true;
  }
  return false;
};

/**
 * Flush all buffers in a map and clear them.
 *
 * @param buffers - Map of buffers to flush
 * @param emit - Callback to emit flushed content (receives taskId, source, text)
 * @returns Number of buffers that had content flushed
 */
export const flushAllBuffers = (
  buffers: Map<string, TBOutputBuffer>,
  emit: (taskId: string, source: TBOutputSource, text: string) => void
): number => {
  let flushedCount = 0;
  for (const buffer of buffers.values()) {
    if (buffer.buffer.length > 0) {
      emit(buffer.taskId, buffer.source, buffer.buffer);
      buffer.buffer = "";
      buffer.lastFlushTime = Date.now();
      flushedCount++;
    }
  }
  return flushedCount;
};

/**
 * Clear all buffers without emitting.
 */
export const clearAllBuffers = (buffers: Map<string, TBOutputBuffer>): void => {
  buffers.clear();
};

/**
 * Get combined content from a buffer (for final aggregation).
 */
export const getBufferContent = (buffer: TBOutputBuffer): string => {
  return buffer.buffer;
};
