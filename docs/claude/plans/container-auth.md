# Plan: Claude Code OAuth Credential Injection for Sandboxes

## Overview

Inject Claude Code OAuth credentials from Mac Keychain into macOS Container sandboxes at runtime, enabling MechaCoder and TerminalBench to authenticate with Claude Code CLI inside containers.

## Credential Flow

```
Mac Keychain ("Claude Code-credentials")
         |
         v
Extract JSON via `security find-generic-password`
         |
         v
Write to temp file on host (/tmp/mechacoder-creds-<uuid>/.credentials.json)
         |
         v
Mount into container (-v /tmp/...:/root/.claude:ro)
         |
         v
Claude Code CLI finds ~/.claude/.credentials.json and authenticates
```

## Implementation

### Task 1: MechaCoder Sandbox Credential Injection

#### 1.1 Create `src/sandbox/credentials.ts`

```typescript
/**
 * Claude Code credential extraction and injection for containers.
 *
 * Extracts OAuth credentials from Mac Keychain at runtime.
 * Creates temporary credential files for container mounting.
 */

import { Effect, Scope } from "effect";
import { FileSystem, Path } from "@effect/platform";
import * as crypto from "crypto";

const KEYCHAIN_SERVICE = "Claude Code-credentials";
const CREDENTIAL_FILENAME = ".credentials.json";

export class CredentialError extends Error {
  readonly _tag = "CredentialError";
  constructor(
    readonly reason: "not_found" | "access_denied" | "invalid_format" | "extraction_failed",
    message: string,
  ) {
    super(message);
  }
}

/**
 * Extract raw credentials JSON from Mac Keychain.
 */
export const extractCredentialsFromKeychain = (): Effect.Effect<string, CredentialError> =>
  Effect.gen(function* () {
    if (process.platform !== "darwin") {
      return yield* Effect.fail(new CredentialError("not_found", "Mac Keychain only available on macOS"));
    }

    const proc = Bun.spawn([
      "security", "find-generic-password",
      "-s", KEYCHAIN_SERVICE,
      "-g"
    ], { stdout: "pipe", stderr: "pipe" });

    const stderr = yield* Effect.promise(() => new Response(proc.stderr).text());
    const exitCode = yield* Effect.promise(() => proc.exited);

    if (exitCode !== 0) {
      if (stderr.includes("could not be found")) {
        return yield* Effect.fail(new CredentialError("not_found", "Claude Code credentials not found in Keychain"));
      }
      return yield* Effect.fail(new CredentialError("access_denied", `Keychain access failed: ${stderr}`));
    }

    // Parse: password: "{\"claudeAiOauth\":{...}}"
    const match = stderr.match(/^password:\s*"(.+)"$/m);
    if (!match) {
      return yield* Effect.fail(new CredentialError("invalid_format", "Could not parse password from Keychain output"));
    }

    // Unescape the JSON (Keychain escapes quotes)
    const jsonStr = match[1].replace(/\\"/g, '"').replace(/\\\\/g, '\\');

    // Validate it's valid JSON
    try {
      JSON.parse(jsonStr);
    } catch {
      return yield* Effect.fail(new CredentialError("invalid_format", "Keychain password is not valid JSON"));
    }

    return jsonStr;
  });

/**
 * Create a temporary credential mount for container use.
 * Returns paths and a cleanup function.
 */
export interface CredentialMount {
  hostDir: string;          // e.g., /tmp/mechacoder-creds-abc123
  hostFilePath: string;     // e.g., /tmp/mechacoder-creds-abc123/.credentials.json
  containerDir: string;     // /root/.claude
  volumeMount: string;      // hostDir:containerDir:ro
}

export const createCredentialMount = (): Effect.Effect<
  CredentialMount,
  CredentialError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;

    // Extract credentials
    const credentialsJson = yield* extractCredentialsFromKeychain();

    // Create temp directory
    const uuid = crypto.randomUUID().slice(0, 8);
    const hostDir = path.join("/tmp", `mechacoder-creds-${uuid}`);
    const hostFilePath = path.join(hostDir, CREDENTIAL_FILENAME);

    yield* fs.makeDirectory(hostDir, { recursive: true }).pipe(
      Effect.mapError(() => new CredentialError("extraction_failed", `Failed to create temp dir: ${hostDir}`))
    );

    // Write credentials file
    yield* fs.writeFileString(hostFilePath, credentialsJson).pipe(
      Effect.mapError(() => new CredentialError("extraction_failed", `Failed to write credentials to ${hostFilePath}`))
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

/**
 * Remove credential mount directory.
 */
export const cleanupCredentialMount = (
  mount: CredentialMount
): Effect.Effect<void, never, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    yield* fs.remove(mount.hostDir, { recursive: true }).pipe(Effect.ignore);
  });
```

#### 1.2 Modify `src/sandbox/schema.ts`

Add `volumeMounts` field to `ContainerConfigSchema` (after line 53):

```typescript
/** Additional volume mounts (e.g., ["/tmp/creds:/root/.claude:ro"]) */
volumeMounts: S.optional(S.Array(S.String)),
```

#### 1.3 Modify `src/sandbox/macos-container.ts`

Add volume mounts support in the `run()` method (after line 88, workspace mount):

```typescript
// Additional volume mounts (for credentials, etc.)
if (config.volumeMounts) {
  for (const mount of config.volumeMounts) {
    args.push("-v", mount);
  }
}
```

#### 1.4 Modify `src/agent/orchestrator/sandbox-runner.ts`

Add credential injection to `runCommand`:

```typescript
import { createCredentialMount, cleanupCredentialMount, CredentialError } from "../../sandbox/credentials.js";

// In runCommand(), before container execution:
const credentialMount = yield* createCredentialMount().pipe(
  Effect.provide(BunContext.layer),
  Effect.catchAll((err: CredentialError) => {
    console.warn(`[sandbox] Credential injection failed: ${err.message}`);
    return Effect.succeed(null);
  })
);

const containerConfig = buildContainerConfig(config.sandboxConfig, config.cwd, {
  ...env,
  ...(credentialMount ? { credentialMount: credentialMount.volumeMount } : {}),
});

// After container execution (in finally block):
if (credentialMount) {
  yield* cleanupCredentialMount(credentialMount).pipe(Effect.provide(BunContext.layer));
}
```

#### 1.5 Export from `src/sandbox/index.ts`

```typescript
export {
  extractCredentialsFromKeychain,
  createCredentialMount,
  cleanupCredentialMount,
  CredentialError,
  type CredentialMount,
} from "./credentials.js";
```

### Task 2: TerminalBench Sandbox Credential Injection

After MechaCoder is working, apply the same pattern to TerminalBench:

#### 2.1 Local Mode (`src/cli/tbench-local.ts`)

For local mode, credentials should already work since Claude Code SDK uses the host's auth. Verify this works.

#### 2.2 Harbor Mode (Docker)

For Harbor/Docker mode, the same credential injection pattern applies:
1. Extract credentials from Keychain
2. Write to temp file
3. Mount into Docker container

Modify `src/terminalbench/harbor/mechacoder_agent.py` or the Docker run configuration to include the credential volume mount.

## Files to Modify

| File | Change |
|------|--------|
| `src/sandbox/credentials.ts` | **NEW** - Credential extraction and mount creation |
| `src/sandbox/schema.ts` | Add `volumeMounts` field to ContainerConfigSchema |
| `src/sandbox/macos-container.ts` | Add loop to process additional volume mounts |
| `src/agent/orchestrator/sandbox-runner.ts` | Inject credentials before container run, cleanup after |
| `src/sandbox/index.ts` | Export credential functions |

## Testing

1. **Unit test**: Verify `extractCredentialsFromKeychain()` returns valid JSON
2. **Integration test**: Verify `createCredentialMount()` creates temp file with correct content
3. **E2E test**: Run MechaCoder in sandbox, verify Claude Code CLI authenticates

```bash
# Manual test - extract credentials
bun -e "
import { extractCredentialsFromKeychain } from './src/sandbox/credentials.js';
import { Effect } from 'effect';
const result = await Effect.runPromise(extractCredentialsFromKeychain());
console.log('Credentials extracted:', result.slice(0, 50) + '...');
"

# Manual test - run sandbox with credentials
bun run mechacoder --sandbox --task <task-id>
```

## Security Notes

- Credentials are extracted fresh from Keychain each run (not cached)
- Temp files have mode 600 (owner read-only)
- Mount is read-only (`:ro` flag)
- Temp directory cleaned up after container exits
- Never log credential content

## Tasks to Create

1. `oa-sandbox-creds-mechacoder` - Implement credential injection for MechaCoder sandboxes
2. `oa-sandbox-creds-tbench` - Implement credential injection for TerminalBench sandboxes
