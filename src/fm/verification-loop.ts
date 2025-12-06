/**
 * Aggressive Verification Loop for FM
 *
 * Makes tests/typecheck the source of truth, not FM reasoning.
 * FM produces minimal edits, orchestrator verifies, feeds errors back.
 *
 * Based on docs/logs/20251206/1421-coding-thoughts.md section 7:
 * "Verify aggressively, not thoroughly"
 */

// --- Constants ---

/**
 * Maximum error message length to feed back to FM.
 */
export const MAX_ERROR_CHARS = 200;

/**
 * Maximum context around error to include.
 */
export const MAX_ERROR_CONTEXT_CHARS = 100;

/**
 * Maximum verification attempts before giving up.
 */
export const MAX_VERIFY_ATTEMPTS = 3;

// --- Error Extraction ---

/**
 * Extract the most relevant part of an error message.
 * Focuses on actionable information, discards stack traces.
 */
export function extractErrorCore(error: string, maxChars = MAX_ERROR_CHARS): string {
  // Remove ANSI codes
  const clean = error.replace(/\x1b\[[0-9;]*m/g, "");

  // Common error patterns to extract
  const patterns = [
    // TypeScript errors: "error TS2345: ..."
    /error TS\d+:\s*(.+?)(?:\n|$)/i,
    // Jest/test errors: "Expected ... Received ..."
    /Expected:?\s*(.+?)\s*Received:?\s*(.+?)(?:\n|$)/i,
    // Generic errors: "Error: ..."
    /Error:\s*(.+?)(?:\n|$)/i,
    // Assertion errors
    /AssertionError:\s*(.+?)(?:\n|$)/i,
    // First meaningful line
    /^(.+?)(?:\n|$)/,
  ];

  for (const pattern of patterns) {
    const match = clean.match(pattern);
    if (match) {
      const extracted = match[1] || match[0];
      if (extracted.length <= maxChars) {
        return extracted.trim();
      }
      return extracted.slice(0, maxChars - 3).trim() + "...";
    }
  }

  // Fallback: first maxChars chars
  return clean.slice(0, maxChars).trim();
}

/**
 * Extract file and line number from error message.
 */
export function extractErrorLocation(error: string): { file?: string; line?: number } {
  // Common patterns: "file.ts:123" or "at file.ts line 123"
  const patterns = [
    /([.\w/]+\.(?:ts|js|tsx|jsx)):(\d+)/,
    /at\s+([.\w/]+\.(?:ts|js|tsx|jsx))\s+line\s+(\d+)/i,
    /in\s+([.\w/]+\.(?:ts|js|tsx|jsx)):(\d+)/i,
  ];

  for (const pattern of patterns) {
    const match = error.match(pattern);
    if (match) {
      return {
        file: match[1],
        line: parseInt(match[2], 10),
      };
    }
  }

  return {};
}

// --- Verification Result ---

export interface VerificationResult {
  success: boolean;
  errorMessage?: string;
  errorLocation?: { file?: string; line?: number };
  rawOutput: string;
  duration_ms: number;
}

/**
 * Run a verification command and extract relevant error info.
 */
export async function runVerification(
  command: string,
  workDir: string,
): Promise<VerificationResult> {
  const startTime = Date.now();

  try {
    const proc = Bun.spawn(["sh", "-c", command], {
      cwd: workDir,
      stdout: "pipe",
      stderr: "pipe",
    });

    const [stdout, stderr] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
    ]);

    const exitCode = await proc.exited;
    const duration_ms = Date.now() - startTime;
    const rawOutput = stdout + stderr;

    if (exitCode === 0) {
      return {
        success: true,
        rawOutput,
        duration_ms,
      };
    }

    // Extract error info
    const errorMessage = extractErrorCore(rawOutput);
    const errorLocation = extractErrorLocation(rawOutput);

    return {
      success: false,
      errorMessage,
      errorLocation,
      rawOutput,
      duration_ms,
    };
  } catch (e) {
    const duration_ms = Date.now() - startTime;
    const errorMessage = e instanceof Error ? e.message : String(e);

    return {
      success: false,
      errorMessage: extractErrorCore(errorMessage),
      rawOutput: errorMessage,
      duration_ms,
    };
  }
}

// --- Feedback Formatting ---

/**
 * Format verification failure as FM feedback.
 * Keeps it short and actionable.
 */
export function formatVerificationFeedback(result: VerificationResult): string {
  if (result.success) {
    return "Verification passed.";
  }

  const parts: string[] = ["Verification failed."];

  if (result.errorMessage) {
    parts.push(`Error: ${result.errorMessage}`);
  }

  if (result.errorLocation?.file) {
    parts.push(`Location: ${result.errorLocation.file}${result.errorLocation.line ? `:${result.errorLocation.line}` : ""}`);
  }

  return parts.join(" ");
}

// --- Verification Loop ---

export interface VerifyLoopOptions {
  /** Verification command to run (e.g., "bun test" or "bun typecheck") */
  command: string;
  /** Working directory */
  workDir: string;
  /** Maximum attempts before giving up */
  maxAttempts?: number;
  /** Callback when verification fails - should return fix attempt or null to abort */
  onFailure?: (result: VerificationResult, attempt: number) => Promise<string | null>;
  /** Callback on success */
  onSuccess?: (result: VerificationResult) => Promise<void>;
}

export interface VerifyLoopResult {
  success: boolean;
  attempts: number;
  lastResult: VerificationResult;
  totalDuration_ms: number;
}

/**
 * Run verification loop until success or max attempts reached.
 * This is the core of "aggressive verification" - let tests be the truth.
 */
export async function runVerifyLoop(options: VerifyLoopOptions): Promise<VerifyLoopResult> {
  const { command, workDir, maxAttempts = MAX_VERIFY_ATTEMPTS, onFailure, onSuccess } = options;
  const startTime = Date.now();
  let attempts = 0;

  while (attempts < maxAttempts) {
    attempts++;

    const result = await runVerification(command, workDir);

    if (result.success) {
      if (onSuccess) {
        await onSuccess(result);
      }
      return {
        success: true,
        attempts,
        lastResult: result,
        totalDuration_ms: Date.now() - startTime,
      };
    }

    // Failed - ask for fix
    if (onFailure) {
      const fix = await onFailure(result, attempts);
      if (fix === null) {
        // Abort
        break;
      }
      // Continue to next attempt (fix should have been applied)
    } else {
      // No failure handler, stop
      break;
    }
  }

  // Failed after all attempts
  const lastResult = await runVerification(command, workDir);
  return {
    success: false,
    attempts,
    lastResult,
    totalDuration_ms: Date.now() - startTime,
  };
}

// --- Quick Verification Helpers ---

/**
 * Quick check if a file exists.
 */
export async function verifyFileExists(path: string): Promise<boolean> {
  try {
    await Bun.file(path).text();
    return true;
  } catch {
    return false;
  }
}

/**
 * Quick check if a file contains expected content.
 */
export async function verifyFileContains(path: string, expected: string): Promise<boolean> {
  try {
    const content = await Bun.file(path).text();
    return content.includes(expected);
  } catch {
    return false;
  }
}

/**
 * Quick check if command succeeds.
 */
export async function verifyCommand(command: string, workDir: string): Promise<boolean> {
  const result = await runVerification(command, workDir);
  return result.success;
}
