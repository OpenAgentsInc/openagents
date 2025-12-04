/**
 * Claude Code credential extraction and injection for containers.
 *
 * Extracts OAuth credentials from Mac Keychain at runtime.
 * Creates temporary credential files for container mounting.
 */

import { Effect } from "effect";
import { FileSystem, Path } from "@effect/platform";
import * as crypto from "crypto";

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const KEYCHAIN_SERVICE = "Claude Code-credentials";
const CREDENTIAL_FILENAME = ".credentials.json";

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

export type CredentialErrorReason =
  | "not_found"
  | "access_denied"
  | "invalid_format"
  | "extraction_failed";

export class CredentialError extends Error {
  readonly _tag = "CredentialError";
  constructor(
    readonly reason: CredentialErrorReason,
    message: string,
  ) {
    super(message);
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Credential Mount Interface
// ─────────────────────────────────────────────────────────────────────────────

export interface CredentialMount {
  /** Host directory (e.g., /tmp/mechacoder-creds-abc123) */
  hostDir: string;
  /** Host file path (e.g., /tmp/mechacoder-creds-abc123/.credentials.json) */
  hostFilePath: string;
  /** Container directory (/root/.claude) */
  containerDir: string;
  /** Volume mount string (hostDir:containerDir:ro) */
  volumeMount: string;
}

// ─────────────────────────────────────────────────────────────────────────────
// Credential Extraction
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Extract raw credentials JSON from Mac Keychain.
 *
 * Uses `security find-generic-password` to extract Claude Code OAuth credentials.
 * Returns the raw JSON string containing the credentials.
 */
export const extractCredentialsFromKeychain = (): Effect.Effect<
  string,
  CredentialError
> =>
  Effect.gen(function* () {
    if (process.platform !== "darwin") {
      return yield* Effect.fail(
        new CredentialError(
          "not_found",
          "Mac Keychain only available on macOS",
        ),
      );
    }

    const proc = Bun.spawn(
      ["security", "find-generic-password", "-s", KEYCHAIN_SERVICE, "-g"],
      { stdout: "pipe", stderr: "pipe" },
    );

    const stderr = yield* Effect.promise(() => new Response(proc.stderr).text());
    const exitCode = yield* Effect.promise(() => proc.exited);

    if (exitCode !== 0) {
      if (stderr.includes("could not be found")) {
        return yield* Effect.fail(
          new CredentialError(
            "not_found",
            "Claude Code credentials not found in Keychain. Please authenticate with Claude Code first.",
          ),
        );
      }
      return yield* Effect.fail(
        new CredentialError("access_denied", `Keychain access failed: ${stderr}`),
      );
    }

    // Parse: password: "{\"claudeAiOauth\":{...}}"
    const match = stderr.match(/^password:\s*"(.+)"$/m);
    if (!match) {
      return yield* Effect.fail(
        new CredentialError(
          "invalid_format",
          "Could not parse password from Keychain output",
        ),
      );
    }

    // Unescape the JSON (Keychain escapes quotes and backslashes)
    const jsonStr = match[1].replace(/\\"/g, '"').replace(/\\\\/g, "\\");

    // Validate it's valid JSON
    try {
      JSON.parse(jsonStr);
    } catch {
      return yield* Effect.fail(
        new CredentialError("invalid_format", "Keychain password is not valid JSON"),
      );
    }

    return jsonStr;
  });

// ─────────────────────────────────────────────────────────────────────────────
// Credential Mount Creation
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Create a temporary credential mount for container use.
 *
 * Extracts credentials from Keychain and writes them to a temp directory
 * that can be mounted into the container at /root/.claude.
 *
 * Returns paths and the volume mount string for use with container run.
 */
export const createCredentialMount = (): Effect.Effect<
  CredentialMount,
  CredentialError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;

    // Extract credentials from Keychain
    const credentialsJson = yield* extractCredentialsFromKeychain();

    // Create temp directory with unique name
    const uuid = crypto.randomUUID().slice(0, 8);
    const hostDir = path.join("/tmp", `mechacoder-creds-${uuid}`);
    const hostFilePath = path.join(hostDir, CREDENTIAL_FILENAME);

    yield* fs.makeDirectory(hostDir, { recursive: true }).pipe(
      Effect.mapError(
        () =>
          new CredentialError(
            "extraction_failed",
            `Failed to create temp dir: ${hostDir}`,
          ),
      ),
    );

    // Write credentials file
    yield* fs.writeFileString(hostFilePath, credentialsJson).pipe(
      Effect.mapError(
        () =>
          new CredentialError(
            "extraction_failed",
            `Failed to write credentials to ${hostFilePath}`,
          ),
      ),
    );

    // Set restrictive permissions (owner read-only)
    yield* Effect.promise(() => Bun.$`chmod 600 ${hostFilePath}`.quiet());

    const containerDir = "/root/.claude";

    return {
      hostDir,
      hostFilePath,
      containerDir,
      volumeMount: `${hostDir}:${containerDir}:ro`,
    };
  });

// ─────────────────────────────────────────────────────────────────────────────
// Cleanup
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Remove credential mount directory.
 *
 * Should be called after container execution completes to clean up
 * temporary credential files.
 */
export const cleanupCredentialMount = (
  mount: CredentialMount,
): Effect.Effect<void, never, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    yield* fs.remove(mount.hostDir, { recursive: true }).pipe(Effect.ignore);
  });
