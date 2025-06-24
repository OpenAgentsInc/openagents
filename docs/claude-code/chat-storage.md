# Claude Code Chat Storage Location Guide

This document explains where Claude Code stores chat data on the local filesystem and how to access it programmatically.

## Storage Locations

Claude Code stores all chat data in JSONL files within project directories. There are two possible locations:

### Primary Location (Current)
```
~/.claude/projects/
```

### Secondary Location (Newer, XDG-compliant)
```
~/.config/claude/projects/
```

**Note:** The newer XDG location may not exist on all systems. Always check both locations.

## Directory Structure

### Project Directories
Each project gets its own directory named after the project's file path, with slashes converted to hyphens:

```
~/.claude/projects/-Users-username-path-to-project/
```

**Example:**
- Project path: `/Users/christopherdavid/code/nexus`
- Directory name: `-Users-christopherdavid-code-nexus`
- Full path: `~/.claude/projects/-Users-christopherdavid-code-nexus/`

### Chat Session Files
Each chat session is stored as a separate JSONL file with a UUID filename:

```
~/.claude/projects/-Users-username-path-to-project/04bc6e0d-ecc8-4b8c-b180-067004f90221.jsonl
```

## File Contents

Each `.jsonl` file contains:
- Chat messages and responses
- Token usage data
- Cost calculations
- Timestamps and metadata
- Model information

## Programmatic Access

### Finding Claude Directories
Check both possible locations and verify they contain a `projects` subdirectory:

```typescript
import { homedir } from 'node:os';
import { isDirectorySync } from 'path-type';
import path from 'node:path';

function getClaudePaths(): string[] {
  const paths: string[] = [];
  const homeDir = homedir();

  // Check both possible locations
  const candidates = [
    path.join(homeDir, '.config/claude'),  // New XDG location
    path.join(homeDir, '.claude')          // Original location
  ];

  for (const candidate of candidates) {
    if (isDirectorySync(candidate)) {
      const projectsPath = path.join(candidate, 'projects');
      if (isDirectorySync(projectsPath)) {
        paths.push(candidate);
      }
    }
  }

  return paths;
}
```

### Environment Variable Override
You can specify custom paths using the `CLAUDE_CONFIG_DIR` environment variable:

```bash
# Single path
export CLAUDE_CONFIG_DIR="/custom/path/to/claude"

# Multiple paths (comma-separated)
export CLAUDE_CONFIG_DIR="/path1,/path2"
```

### Finding All Chat Files
Use glob patterns to find all JSONL files:

```typescript
import { glob } from 'tinyglobby';
import path from 'node:path';

async function findAllChatFiles(claudePath: string): Promise<string[]> {
  const projectsDir = path.join(claudePath, 'projects');
  const pattern = path.join(projectsDir, '**/*.jsonl');
  return await glob(pattern);
}
```

### Parsing JSONL Files
Each line in a JSONL file is a separate JSON object:

```typescript
import { readFile } from 'node:fs/promises';

async function parseChatFile(filePath: string) {
  const content = await readFile(filePath, 'utf-8');
  const lines = content.split('\n').filter(line => line.trim());

  return lines.map(line => {
    try {
      return JSON.parse(line);
    } catch (error) {
      // Skip malformed lines
      return null;
    }
  }).filter(Boolean);
}
```

## Session Identification

Sessions are identified by the directory structure:
- **Project**: Directory name under `projects/`
- **Session**: Individual JSONL filename (UUID)

Example breakdown:
```
~/.claude/projects/-Users-christopherdavid-code-nexus/04bc6e0d-ecc8-4b8c-b180-067004f90221.jsonl
                   |                                    |
                   Project ID                           Session ID
```

## Reference Implementation

For a complete working implementation, see the `ccusage` tool at `/Users/christopherdavid/code/ccusage/`:
- `src/data-loader.ts` - Core data loading logic
- `src/_consts.ts` - Path constants and configuration
- `src/_session-blocks.ts` - Session identification and grouping

## Additional Storage Locations

Beyond chat data, Claude Code also stores:
- Settings: `~/.claude/settings.json`
- Local settings: `~/.claude/settings.local.json`
- IDE state: `~/.claude/ide/`
- Stats: `~/.claude/statsig/`
- Todos: `~/.claude/todos/`

## Best Practices

1. **Check both locations** - Always search both `~/.claude` and `~/.config/claude`
2. **Handle missing directories gracefully** - Not all locations may exist
3. **Skip malformed JSONL lines** - Parse errors should be handled silently
4. **Use path utilities** - Always use `path.join()` for cross-platform compatibility
5. **Respect environment variables** - Honor `CLAUDE_CONFIG_DIR` if set
