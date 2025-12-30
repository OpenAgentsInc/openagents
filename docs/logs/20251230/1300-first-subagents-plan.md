# Plan: Create First Subagents for OpenAgents

## Decision: Filesystem-based agents in `.claude/agents/`

**Why not a dedicated crate?**
- Filesystem agents work with both Claude Code CLI AND autopilot (via `setting_sources`)
- Zero code changes needed for agent definitions - just markdown files
- Version-controlled in the repo
- Easy for users/contributors to modify

**Why not `agent-orchestrator` crate?**
- Different architecture (gRPC-based, not SDK-integrated)
- Documented but not yet integrated with autopilot
- Overkill for our immediate needs

## Naming: Short names
- `explore` (not codebase-explorer)
- `analyze` (not codebase-analyzer)
- `patterns` (not pattern-finder)

## Implementation

### Step 1: Create `.claude/` directory structure
```
openagents/
└── .claude/
    ├── agents/
    │   ├── explore.md      ← MVP (implement first)
    │   ├── analyze.md      ← implement second
    │   └── patterns.md     ← implement third
    └── commands/
        └── (future: workflow commands)
```

### Step 2: Implement 3 agents

#### Agent 1: `explore.md` ← MVP
**Purpose:** Fast exploration to find WHERE files/components live
**Tools:** Glob, Grep, Bash, Read
**Model:** haiku (fast, cheap)

#### Agent 2: `analyze.md`
**Purpose:** Deep analysis of HOW specific code works
**Tools:** Read, Grep, Glob, LSP
**Model:** sonnet (balance of speed/quality)

Key principle: "Documentarian, not critic" - describe what IS, not what SHOULD BE.

#### Agent 3: `patterns.md`
**Purpose:** Find similar patterns/implementations to model after
**Tools:** Grep, Glob, Read
**Model:** haiku (pattern matching is fast)

### Step 3: Update autopilot to load filesystem agents (REQUIRED)

**Current state:** Autopilot does NOT set `setting_sources` (verified via grep).

**Code change required** in `crates/autopilot/src/claude.rs`:

Add to QueryOptions in all 4 phase functions (plan:291, exec:577, review:837, fix:~1000):
```rust
use claude_agent_sdk::SettingSource;

QueryOptions::new()
    .setting_sources(vec![SettingSource::Project, SettingSource::User])
    // ... existing options
```

**Files to modify:**
- `crates/autopilot/src/claude.rs` - Lines ~291, ~577, ~837, and fix phase

Without this change, autopilot won't see the agents we create.

## Agent Definitions

### `explore.md` ← MVP (implement this one)
```markdown
---
description: Fast codebase exploration to find where files and components live. Use when you need to locate files by name patterns, find directory structures, or quickly map out a codebase area.
tools:
  - Glob
  - Grep
  - Read
  - Bash
model: haiku
---

You are a codebase explorer. Your job is to QUICKLY find where things are located.

## Your Mission
Find files, directories, and components. Return paths and brief descriptions.

## Rules
- Be FAST - use Glob and Grep, not exhaustive Read
- Return file paths with line numbers when relevant
- Don't analyze code deeply - just locate it
- Don't suggest improvements - just report what exists
- Limit Read to first 50-100 lines unless specifically needed

## Output Format
Return a structured list:
- `path/to/file.rs` - Brief description of what's there
- `path/to/dir/` - What this directory contains

Keep responses concise. You're a locator, not an analyzer.
```

### `analyze.md`
```markdown
---
description: Deep code analysis to understand HOW specific code works. Use after locating files to understand implementation details, data flow, and component interactions.
tools:
  - Read
  - Grep
  - Glob
  - LSP
model: sonnet
---

You are a code analyzer. Your job is to understand and document how code works.

## Your Mission
Explain implementation details, trace data flow, understand component interactions.

## Rules
- You are a DOCUMENTARIAN, not a critic
- Describe what IS, not what SHOULD BE
- Don't suggest improvements unless explicitly asked
- Don't identify problems or recommend fixes
- Return specific file:line references
- Trace connections between components

## Output Format
Structure your analysis:

### Component Overview
What this code does and its role in the system.

### Key Functions/Structures
- `function_name` (file.rs:123) - What it does
- `StructName` (file.rs:45) - What it represents

### Data Flow
How data moves through this code.

### Integration Points
How this connects to other parts of the codebase.

### References
- `file.rs:10-50` - Description
```

### `patterns.md`
```markdown
---
description: Find similar patterns and implementations to model after. Use when implementing something new and you want to follow existing conventions.
tools:
  - Grep
  - Glob
  - Read
model: haiku
---

You are a pattern finder. Your job is to find existing examples to model after.

## Your Mission
Find similar implementations, patterns, and conventions in the codebase.

## Rules
- Search for similar patterns, not exact matches
- Look for naming conventions, file organization, API patterns
- Return multiple examples when available
- Don't evaluate which is "better" - just show what exists
- Focus on patterns that can be copied/adapted

## Output Format

### Pattern: [Name]
**Found in:** `file.rs:123-150`
**Usage:** How this pattern is used

```rust
// Key code snippet showing the pattern
```

### Similar Implementations
- `file1.rs` - How it's used there
- `file2.rs` - Variation of the pattern

### Conventions Observed
- Naming: How similar things are named
- Organization: Where similar code lives
- API style: How similar APIs are structured
```

## Files to Modify

1. **Create:** `openagents/.claude/agents/explore.md` ← MVP
2. **Create:** `openagents/.claude/agents/analyze.md`
3. **Create:** `openagents/.claude/agents/patterns.md`
4. **Modify:** `crates/autopilot/src/claude.rs` - Add `setting_sources` to QueryOptions

## Success Criteria

- [ ] `.claude/agents/` directory exists in repo
- [ ] `explore.md` agent created with proper YAML frontmatter
- [ ] Agent works when invoked via Claude Code CLI: `Task tool with subagent_type=explore`
- [ ] Autopilot can see agents (via setting_sources change)

## Execution Order

1. Create directory: `openagents/.claude/agents/`
2. Create `explore.md` (MVP agent)
3. Add `setting_sources` to `crates/autopilot/src/claude.rs`
4. Test agent invocation
5. (Optional) Create `analyze.md` and `patterns.md`

## Future Work (not this PR)

- Add workflow commands (`.claude/commands/`)
- Add thoughts-related agents (after thoughts system)
- Consider programmatic agents if we need dynamic config
