# TB2 Architecture Fix - Claude on Host, Not in Container

## The Problem

Current implementation tries to run `claude` CLI inside TB2 Docker containers:

```
Docker Container (alexgshaw/regex-log:20251031)
  └─ bash -c "claude --verbose ..."  ❌ claude: command not found
```

**Why it fails:**
- TB2 Docker images are minimal task environments (Python, bash, etc.)
- They don't have Node.js or Claude CLI installed
- Installing them at runtime would be slow (~30s per run)

## The Correct Architecture

```
┌─────────────────────────────────────────┐
│ HOST MACHINE                            │
│                                         │
│  1. claude CLI (Node.js)                │
│     └─ Working dir: /tmp/.tmpXXX        │
│     └─ Output: stream-json              │
│     └─ Credentials: ~/.claude           │
│                                         │
│  2. After Claude completes:             │
│     └─ Docker run alexgshaw/regex-log   │
│        └─ Mount workspace as /app       │
│        └─ Run: bash /tests/test.sh      │
│        └─ Check: reward.txt (0 or 1)    │
│                                         │
└─────────────────────────────────────────┘
```

## Implementation

### 1. Run Claude on Host

```rust
// NOT: docker run <image> bash -c "claude ..."
// YES: claude --verbose --working-directory /tmp/workspace ...

let mut child = Command::new("claude")
    .args([
        "--verbose",
        "--output-format", "stream-json",
        "--dangerously-skip-permissions",
        "-p", &instruction,
        "--allowedTools", &allowed_tools,
        "--max-turns", &max_turns.to_string(),
    ])
    .current_dir(&workspace_dir)  // Run in workspace
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
```

### 2. Use Docker Only for Verification

```rust
// After Claude completes, verify with Docker
docker run --rm \
  -v /tmp/workspace:/app \
  -v /tmp/logs:/logs \
  -v ~/code/terminal-bench-2/tasks/regex-log/tests:/tests:ro \
  alexgshaw/regex-log:20251031 \
  bash -c "bash /tests/test.sh"
```

## Benefits

✅ **No Node.js needed in containers** - Claude runs on host
✅ **Faster startup** - No npm install in containers
✅ **Simpler credential handling** - Use host credentials
✅ **Same isolation for tests** - Docker still used for verification
✅ **Cleaner separation** - Agent (host) vs Environment (Docker)

## File Changes Needed

### `docker_runner.rs`

**Remove:**
- `build_claude_command()` - was for running in container
- Credential mounting to container - not needed
- Complex Docker command building

**Add:**
- `run_claude_on_host()` - spawn Claude CLI directly
- Stream JSON parsing (already exists)
- Pass workspace_dir to Claude via --current-dir or CWD

### `verifier.rs`

**Keep as-is** - Already runs Docker correctly for verification

## Migration Path

1. **Short term:** Run Claude on host, keep verification in Docker
2. **Long term:** Consider if we even need Docker for verification
   - Could run test.sh on host too
   - Docker provides isolation but adds complexity
   - TB2 spec requires Docker for official evaluation

## Testing

```bash
# Manual test - what we're automating
cd /tmp/test-workspace
claude --verbose -p "Write a regex that matches IP addresses" --max-turns 5

# Then verify
docker run --rm -v $(pwd):/app alexgshaw/regex-log:20251031 bash /tests/test.sh
cat /tmp/test-workspace/logs/verifier/reward.txt  # Should show 0 or 1
```
