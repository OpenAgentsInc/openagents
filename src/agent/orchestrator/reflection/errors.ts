/**
 * Reflection Errors
 *
 * Error types for the ReflectionService.
 */

/**
 * Reasons a reflection operation can fail.
 */
export type ReflectionErrorReason =
  | "generation_failed" // LLM failed to generate reflection
  | "parse_error" // Failed to parse LLM response
  | "storage_error" // Failed to read/write storage
  | "not_found" // Reflection not found
  | "timeout"; // Operation timed out

/**
 * Error class for ReflectionService operations.
 */
export class ReflectionError extends Error {
  readonly _tag = "ReflectionError";

  constructor(
    readonly reason: ReflectionErrorReason,
    message: string
  ) {
    super(message);
    this.name = "ReflectionError";
  }

  /**
   * Create a generation failed error.
   */
  static generationFailed(message: string): ReflectionError {
    return new ReflectionError("generation_failed", message);
  }

  /**
   * Create a parse error.
   */
  static parseError(message: string): ReflectionError {
    return new ReflectionError("parse_error", message);
  }

  /**
   * Create a storage error.
   */
  static storageError(message: string): ReflectionError {
    return new ReflectionError("storage_error", message);
  }

  /**
   * Create a not found error.
   */
  static notFound(id: string): ReflectionError {
    return new ReflectionError("not_found", `Reflection not found: ${id}`);
  }

  /**
   * Create a timeout error.
   */
  static timeout(operation: string): ReflectionError {
    return new ReflectionError("timeout", `Operation timed out: ${operation}`);
  }
}
