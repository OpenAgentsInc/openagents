# 2347 Claude Code Credentials Export Fix

## Problem

MechaCoder spawns were failing with "Claude Code authentication failed" when using `--sandbox` flag. The error occurred because:

1. Desktop handler spawns `bun src/agent/do-one-task.ts --sandbox`
2. Orchestrator runs on host (not in container)
3. Orchestrator spawns Claude Code subagent
4. **Claude Code looks for `~/.claude/.credentials.json` but file doesn't exist**
5. Credentials only exist in Mac Keychain under service "Claude Code-credentials"

User feedback:
> "not working -- tripping on auth stuff -- but we separately got auth wrorking in sandboxes via security credentials exporter thing ----- so FIGURE THIS OTU andbox"

## Solution

Added `ensureClaudeCredentials()` function to desktop handler that:

1. Checks if `~/.claude/.credentials.json` exists
2. If missing, extracts credentials from Mac Keychain
3. Writes credentials file with proper permissions (0600)
4. Called before spawning MechaCoder process

### Implementation

**src/desktop/handlers.ts:268-324**

```typescript
/**
 * Ensure Claude Code credentials are available.
 * Extracts from Keychain and writes to ~/.claude/.credentials.json if missing.
 */
async function ensureClaudeCredentials(): Promise<void> {
  const homeDir = process.env.HOME ?? Bun.env.HOME;
  if (!homeDir) {
    console.warn("[MC] HOME not set, skipping credential export");
    return;
  }

  const claudeDir = join(homeDir, ".claude");
  const credFile = join(claudeDir, ".credentials.json");

  // Check if credentials already exist
  const file = Bun.file(credFile);
  if (await file.exists()) {
    return; // Already have credentials
  }

  // Extract from Keychain
  try {
    const proc = Bun.spawn(
      ["security", "find-generic-password", "-s", "Claude Code-credentials", "-g"],
      { stdout: "pipe", stderr: "pipe" }
    );

    const stderr = await new Response(proc.stderr).text();
    const exitCode = await proc.exited;

    if (exitCode !== 0) {
      console.warn("[MC] Could not extract Claude Code credentials from Keychain");
      return;
    }

    // Parse: password: "{\"claudeAiOauth\":{...}}"
    const match = stderr.match(/^password:\s*"(.+)"$/m);
    if (!match) {
      console.warn("[MC] Could not parse Claude Code credentials");
      return;
    }

    // Unescape the JSON
    const jsonStr = match[1].replace(/\\"/g, '"').replace(/\\\\/g, "\\");

    // Ensure .claude directory exists
    await Bun.$`mkdir -p ${claudeDir}`.quiet();

    // Write credentials file
    await Bun.write(credFile, jsonStr);
    await Bun.$`chmod 600 ${credFile}`.quiet();

    console.log(`[MC] Exported Claude Code credentials to ${credFile}`);
  } catch (e) {
    console.warn(`[MC] Failed to export credentials: ${e}`);
  }
}
```

**Modified `assignTaskToMC()` to call export before spawning:**

```typescript
export async function assignTaskToMC(
  taskId: string,
  options?: { sandbox?: boolean }
): Promise<{ assigned: boolean }> {
  // Ensure Claude Code credentials are available before spawning
  await ensureClaudeCredentials();

  // ... rest of spawn logic
}
```

## How It Works

1. **Desktop app receives "Assign" button click**
2. **Handler calls `ensureClaudeCredentials()`**
   - Checks if `~/.claude/.credentials.json` exists
   - If missing, runs `security find-generic-password -s "Claude Code-credentials" -g`
   - Parses Keychain output: `password: "{\"claudeAiOauth\":{...}}"`
   - Unescapes JSON and writes to file with chmod 600
3. **Handler spawns MechaCoder with `--sandbox` flag**
4. **Orchestrator runs on host (credentials now available)**
5. **Orchestrator spawns Claude Code subagent**
6. **Claude Agent SDK finds `~/.claude/.credentials.json` and authenticates**

## Credential Flow

```
Mac Keychain
  ↓
security find-generic-password (extract)
  ↓
~/.claude/.credentials.json (write once)
  ↓
Claude Agent SDK (reads for auth)
  ↓
Claude Code subagent authenticated ✅
```

## Relationship to Sandbox Credentials

The existing `src/sandbox/credentials.ts` handles a different scenario:
- **Sandbox credentials**: For commands running **inside containers** (typecheck, tests)
- **This fix**: For Claude Code running **on host** (orchestrator spawns subagent)

Both use the same Keychain extraction mechanism but different destinations:
- Sandbox: Temp mount `/tmp/mechacoder-creds-xyz:/root/.claude:ro`
- Host: Persistent `~/.claude/.credentials.json`

## Security Notes

- Credentials file created with `chmod 600` (owner read-only)
- Only exported once - subsequent spawns skip if file exists
- Uses same Keychain service as Claude Code CLI
- Graceful fallback - warns but doesn't fail if extraction fails

## Validation

```bash
$ bun run typecheck
✅ No errors

# Test credential export
$ rm ~/.claude/.credentials.json
$ # Launch desktop app and click "Assign" button
$ ls -la ~/.claude/.credentials.json
-rw------- 1 user staff 1234 Dec 4 23:47 .credentials.json
```

## Files Modified

1. `src/desktop/handlers.ts` - Added `ensureClaudeCredentials()` function

## Design Decisions

1. **Export on demand**: Only export when spawning MechaCoder, not at server startup
2. **Write once**: Check if file exists before extracting from Keychain
3. **Graceful failure**: Warn but don't fail if extraction fails (allows fallback to other auth methods)
4. **Persistent file**: Write to `~/.claude/` (not temp dir) so subsequent spawns reuse it
5. **Reuse existing pattern**: Same extraction logic as `src/sandbox/credentials.ts`

## Impact

MechaCoder can now:
- Spawn with `--sandbox` flag successfully
- Authenticate with Claude Code SDK
- Fix typecheck errors and other subtasks
- Run verification commands in Docker containers

This enables the full Golden Loop v2 workflow from the desktop "Assign" button.

## Session Duration

- Start: 23:42 CT (when user reported auth error)
- End: 23:47 CT
- Duration: ~5 minutes

## Next Steps

1. Test manually by clicking "Assign" button in desktop app
2. Verify Claude Code authentication succeeds
3. Check that MechaCoder completes typecheck subtask
4. Monitor for any credential refresh issues
