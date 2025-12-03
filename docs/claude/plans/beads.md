# Features to Pull from Beads to OpenAgents Task System

Based on comparing the beads codebase (`~/code/beads`) with our OpenAgents task system, here are features we're missing that could be valuable.

## Priority Tiers

### Tier 1: High Value, Low Effort
These are quick wins that improve daily workflow.

| Feature | Beads Command | Value | Effort |
|---------|--------------|-------|--------|
| **Reopen tasks** | `bd reopen <id>` | Undo accidental closes | Low |
| **Task stats** | `bd stats` | Quick project health overview | Low |
| **Stale detection** | `bd stale` | Find forgotten work | Low |
| **Show dependencies** | `bd show <id>` | Visualize what blocks/is-blocked-by | Low |
| **List labels** | `bd label list` | See all labels in use | Low |

### Tier 2: High Value, Medium Effort
Core functionality that significantly improves the system.

| Feature | Beads Command | Value | Effort |
|---------|--------------|-------|--------|
| **Doctor/validate** | `bd doctor`, `bd validate` | Health checks, find broken deps | Medium |
| **Comments system** | `bd comments add/list` | Discussion on tasks | Medium |
| **Delete with cascade** | `bd delete --cascade` | Clean removal of tasks | Medium |
| **Cleanup old tasks** | `bd cleanup --older-than=30d` | Bulk delete closed tasks | Medium |
| **Config CLI** | `bd config set/get/list` | Modify project.json via CLI | Medium |

### Tier 3: Medium Value, Medium Effort
Nice-to-have features.

| Feature | Beads Command | Value | Effort |
|---------|--------------|-------|--------|
| **Duplicate detection** | `bd duplicates` | Find similar tasks | Medium |
| **Merge duplicates** | `bd merge <ids> --into <target>` | Consolidate work | Medium |
| **Rename prefix** | `bd rename-prefix <new>` | Change ID prefix (oa → xyz) | Medium |
| **Advanced search filters** | Date ranges, empty field filters | More precise filtering | Medium |
| **Repair deps** | `bd repair-deps` | Fix orphaned dependencies | Low |

### Tier 4: Medium Value, Higher Effort
More complex features.

| Feature | Beads Command | Value | Effort |
|---------|--------------|-------|--------|
| **Git hooks** | `bd hooks install/uninstall` | Auto-import after pulls | High |
| **Compaction** | `bd compact` | Summarize old closed tasks | High |
| **Restore deleted** | `bd restore <id>` | Recover from git history | High |
| **Deletion tombstones** | `deletions.jsonl` | Track deletions across clones | High |

### Tier 5: Lower Priority
Features that are nice but not critical for our use case.

| Feature | Beads Command | Value | Effort |
|---------|--------------|-------|--------|
| **Templates** | `bd template` | Pre-defined task structures | Medium |
| **Markdown import** | `bd create -f <file.md>` | Bulk create from docs | Medium |
| **Onboarding/tips** | `bd onboard`, `bd tips` | Agent help system | Low |
| **Prime context** | `bd prime` | Inject minimal context | Low |
| **Info command** | `bd info` | Show DB status/version | Low |

## Feature NOT Recommended to Pull

| Feature | Reason |
|---------|--------|
| **Daemon architecture** | Overkill - we don't need background processes |
| **Agent Mail** | Multi-agent coordination not needed (single agent per repo) |
| **Multi-repo routing** | We work in single repos |
| **SQLite storage** | Our JSONL-only approach is simpler and sufficient |
| **MCP server** | We have different integration patterns |
| **Protected branch workflow** | Not needed for our use case |

## Recommended Implementation Order

1. **Quick Wins (1-2 days total)**
   - `tasks:reopen` - Reopen closed tasks
   - `tasks:stats` - Show counts by status/type/priority
   - `tasks:stale` - Find tasks not updated in N days
   - Enhanced `tasks:show` with dependency visualization

2. **Core Improvements (3-5 days total)**
   - `tasks:doctor` - Validate task file integrity
   - `tasks:validate` - Check for orphan deps, conflicts
   - `tasks:delete` with `--cascade` option
   - `tasks:cleanup` - Bulk delete old closed tasks

3. **Collaboration Features (2-3 days)**
   - Comments system (add `comments` field to Task schema)
   - `tasks:comment add/list <id>`

4. **Maintenance Tools (2-3 days)**
   - `tasks:config set/get/list` - CLI config management
   - `tasks:duplicates` - Find similar tasks
   - `tasks:merge` - Consolidate duplicates

5. **Advanced (if needed)**
   - Git hooks for auto-sync
   - Prefix renaming
   - Compaction

## Schema Changes Required

```typescript
// Add to Task schema:
interface Task {
  // ... existing fields ...
  comments?: Comment[];  // New: discussion threads
}

interface Comment {
  id: string;
  text: string;
  author: string;
  createdAt: string;
}
```

## New CLI Commands Summary

```bash
# Tier 1 (Quick Wins)
bun run tasks:reopen --id <id>
bun run tasks:stats
bun run tasks:stale --days 30
bun run tasks:show --id <id>  # Enhanced with dep tree

# Tier 2 (Core)
bun run tasks:doctor
bun run tasks:validate
bun run tasks:delete --id <id> [--cascade]
bun run tasks:cleanup --older-than 30d [--dry-run]
bun run tasks:config get/set/list

# Tier 3 (Nice to Have)
bun run tasks:comment add --id <id> --text "..."
bun run tasks:comment list --id <id>
bun run tasks:duplicates
bun run tasks:merge --ids <id1,id2> --into <target>

# Tier 4 (Advanced)
bun run tasks:hooks install/uninstall
bun run tasks:compact
bun run tasks:restore --id <id>
```

---

## Tasks to Create

The following tasks will be created in `.openagents/tasks.jsonl`:

### Epic: Task System Enhancement (from beads)

**Parent Epic:**
- `oa-beads-epic` - "Enhance task system with features from beads"

**Tier 1 Tasks (Quick Wins):**
1. `tasks:reopen` - Add reopen command to set closed tasks back to open
2. `tasks:stats` - Add stats command showing counts by status/type/priority
3. `tasks:stale` - Add stale command to find tasks not updated in N days
4. `tasks:show` - Enhance show with dependency tree visualization

**Tier 2 Tasks (Core):**
5. `tasks:doctor` - Add doctor command to diagnose task file issues
6. `tasks:validate` - Add validate command to check integrity (orphan deps, conflicts)
7. `tasks:delete` - Add delete command with --cascade option
8. `tasks:cleanup` - Add cleanup command to bulk delete old closed tasks
9. `tasks:config` - Add config get/set/list commands for project.json
10. Comments schema - Add comments field to Task schema
11. `tasks:comment` - Add comment add/list commands

**Tier 3 Tasks (Nice to Have):**
12. `tasks:duplicates` - Add command to find similar/duplicate tasks
13. `tasks:merge` - Add command to consolidate duplicates
14. `tasks:rename-prefix` - Add command to change ID prefix
15. `tasks:repair-deps` - Add command to fix orphaned dependencies

**Tier 4 Tasks (Advanced):**
16. Git hooks system - Add hooks install/uninstall for auto-sync
17. `tasks:compact` - Add compaction for old closed tasks
18. Deletion tombstones - Track deletions in separate file
19. `tasks:restore` - Restore deleted tasks from git history

---

## Critical Files to Modify

| File | Changes |
|------|---------|
| `src/tasks/schema.ts` | Add Comment type, comments field to Task |
| `src/tasks/service.ts` | Add new service functions |
| `src/tasks/cli.ts` | Add all new commands |
| `src/tasks/id.ts` | May need prefix rename utilities |
| `.openagents/project.json` | No schema changes needed |
| `package.json` | Add new npm scripts |

---

## Implementation Notes

**Comments Schema Addition:**
```typescript
const Comment = Schema.Struct({
  id: Schema.String,
  text: Schema.String,
  author: Schema.String,
  createdAt: Schema.String,
});

// Add to Task:
comments: Schema.optionalWith(Schema.Array(Comment), { default: () => [] }),
```

**Stats Output Format:**
```
Tasks by Status:
  open: 12
  in_progress: 3
  blocked: 2
  closed: 45

Tasks by Type:
  feature: 20
  bug: 15
  task: 22
  epic: 3

Tasks by Priority:
  P0 (critical): 1
  P1 (high): 5
  P2 (medium): 30
  P3 (low): 10
  P4 (backlog): 16
```

**Doctor Checks:**
- Parse errors in tasks.jsonl
- Invalid task IDs
- Missing required fields
- Orphan dependencies (reference non-existent tasks)
- Circular dependency detection
- Stale in_progress tasks

**Git Hooks to Install:**
- `post-merge` - Auto-reload tasks after git pull
- `post-checkout` - Auto-reload after branch switch
- `pre-commit` - Validate tasks.jsonl before commit
