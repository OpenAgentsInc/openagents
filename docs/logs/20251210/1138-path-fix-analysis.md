# TestGen/HillClimber Analysis: Docker Path Conversion Fix

**Date:** 2024-12-10 11:38
**Task:** `regex-log` from Terminal-Bench 2
**Issue:** Tests used Docker paths (`/app/regex.txt`) but HillClimber runs locally
**Status:** FIX IMPLEMENTED - FM still not creating files

---

## The Problem

Generated tests were using absolute Docker container paths:

```python
# BEFORE: Absolute Docker path
pattern = Path("/app/regex.txt").read_text().strip()
```

When pytest runs locally from the workspace:
- `/app/regex.txt` doesn't exist (that's a Docker container path)
- FM writes to `/app/regex.txt` which gets resolved to `workspace/regex.txt` via `resolve_path()`
- But the tests check the LITERAL path `/app/regex.txt`

This mismatch meant tests always failed with "file not found" even if FM wrote the correct file.

---

## The Fix

Added `docker_path_to_relative()` function to convert Docker paths to relative paths:

```rust
/// Convert Docker paths to relative paths for local execution.
fn docker_path_to_relative(path: &str) -> String {
    if path.starts_with("/app/") {
        // Strip /app/ prefix - pytest runs from workspace which maps to /app/
        path[5..].to_string()
    } else {
        path.to_string()
    }
}
```

Updated all test body generators to use this conversion:
- `generate_existence_body()`
- `generate_correctness_regex()`
- `generate_integration_regex()`

### Generated Tests Now

```python
# AFTER: Relative path (pytest runs from workspace)
pattern = Path("regex.txt").read_text().strip()
```

---

## Files Changed

| File | Changes |
|------|---------|
| `crates/hillclimber/src/testgen_writer.rs` | Added `docker_path_to_relative()`, updated body generators |
| `crates/testgen/src/formatter.rs` | Same changes (parallel implementation) |

### Test Updates

Updated unit tests to expect relative paths:
- `test_format_as_pytest_existence`: Now expects `Path("output.txt")` not `Path("/app/output.txt")`
- Added `test_docker_path_to_relative()` to verify conversion

---

## Verification

All tests pass:
```
cargo test -p testgen -p hillclimber --lib
test result: ok. 43 passed; 0 failed
```

---

## HillClimber Runs

### Run 1: 5 Turns
```
Result: FAIL (score: 95, turns: 5)
```

### Run 2: 15 Turns
```
Result: FAIL (score: 85, turns: 15)
```

### Issue: FM Not Creating Files

The FM is NOT creating `regex.txt` at all:
```bash
ls /Users/christopherdavid/code/terminal-bench-2/regex-log/*.txt
# No .txt files found
```

The path fix is correct, but there's a deeper issue: **the FM is not understanding or following the task instructions**.

---

## Next Steps

The path conversion fix is complete and verified. The remaining issue is FM behavior:

### 1. Debug FM Response Parsing
The FM should return JSON like:
```json
{
  "tool_name": "write_file",
  "tool_args": {"path": "/app/regex.txt", "content": "\\d{4}-\\d{2}-\\d{2}"},
  "reasoning": "Writing initial regex"
}
```

Need to verify:
- Is FM generating valid JSON?
- Is `parse_fm_response()` successfully extracting tool calls?
- Is the orchestrator executing the tools?

### 2. Add Debug Logging
Add logging to orchestrator to see:
- What prompts FM receives
- What FM responses contain
- What actions get executed

### 3. Check FM Bridge
The FM bridge (`localhost:3030`) may be:
- Returning empty responses
- Returning refusals due to safety filters
- Timing out

### 4. Fallback: Add Verbose Logging Mode
Create `--debug` flag to log:
- Full prompts sent to FM
- Full FM responses received
- Parsed actions
- Execution results

---

## Summary

| Fix | Status |
|-----|--------|
| Docker path conversion | COMPLETE |
| Tests use relative paths | COMPLETE |
| Tests have real assertions | COMPLETE (from previous fix) |
| FM creates regex.txt | NOT WORKING |
| FM iterates on feedback | UNKNOWN (never gets that far) |

**The path fix is correct.** The remaining work is debugging why the FM doesn't create files.

---

## Technical Details

### Path Resolution Flow

```
Task Description: "Save your regex in /app/regex.txt"
                           ↓
   detect_task_type() extracts: "/app/regex.txt"
                           ↓
   docker_path_to_relative() converts: "regex.txt"
                           ↓
   Generated test uses: Path("regex.txt")
                           ↓
   Pytest runs from workspace, finds: workspace/regex.txt
```

### WorkspaceExecutor Path Resolution (Already Correct)

```rust
fn resolve_path(&self, path: &str) -> PathBuf {
    if path.starts_with('/') {
        if path.starts_with("/app/") {
            self.workspace.join(&path[5..])  // /app/regex.txt -> workspace/regex.txt
        } else {
            PathBuf::from(path)
        }
    } else {
        self.workspace.join(path)
    }
}
```

Both FM writes and test reads now map to the same location: `workspace/regex.txt`.
