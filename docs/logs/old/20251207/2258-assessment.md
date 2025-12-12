# FM Mini Suite Assessment - 2258

## Run: fm-mini-20251207-225800

**Pass Rate: 71.4% (5/7)** - New best! fm-list-directory now passes.

## Passed Tasks (5)
| Task | Turns | Notes |
|------|-------|-------|
| fm-hello-world | 2 | Simple write |
| fm-append-to-file | 4 | Simple write |
| fm-list-directory | 4 | **NEW PASS** - 500 char limit helped |
| fm-create-and-run | 7 | Multi-step |
| fm-simple-edit | 2 | Simple edit |

## Failed Tasks (2)

### fm-read-and-echo (5 turns) - Content Mangling
```
source.txt (74 bytes, 3 lines):
This is the source content.
It has multiple lines.
Please copy it exactly.

echo.txt (50 bytes, 1 line):
This is the source content. It has multiple lines.
```

**Problem**: FM collapses newlines to spaces and drops the 3rd line.
**Analysis**: FM sees the content correctly in Previous field but mangles it when writing.

### fm-word-count (5 turns) - Doesn't Read First
```
document.txt: "The quick brown fox jumps over the lazy dog." (9 words)
count.txt: "100" (wrong - FM guessed)
```

**Problem**: FM writes random number without reading document.txt first.
**Analysis**: FM doesn't understand "count words" requires reading the file.

## Root Cause Analysis

### fm-read-and-echo
FM correctly reads the file and gets content in Previous:
```
Previous: source.txt contains: This is the source content.
It has multiple lines.
Please copy it exactly.
```

But when FM generates write_file, it outputs:
```json
{"path":"echo.txt","content":"This is the source content. It has multiple lines."}
```

**Theory**: FM's tokenizer or generation is collapsing newlines. The content in the JSON output has no `\n` characters.

**Potential fix**: Explicitly tell FM to preserve newlines, or encode content differently.

### fm-word-count
FM never calls read_file on document.txt. It jumps straight to write_file with a guess.

**Theory**: FM doesn't understand semantic meaning of "count words" - it just sees "write number to count.txt".

**Potential fix**: Add hint for tasks containing "count" or "number of" to read first.

## Attempted Fixes

### Fix 1: Explicit newline preservation hint
Add to prompt: "When copying file content, preserve exact formatting including newlines."

### Fix 2: Read-first hint for counting tasks
Detect "count" or "number of" in task description and add hint to read first.

## Progress

| Run | Pass Rate | Change |
|-----|-----------|--------|
| 2232 | 14.3% | Initial |
| 2238 | 42.9% | +3 |
| 2248 | 57.1% | +1 |
| 2255 | 57.1% | = |
| 2258 | 71.4% | +1 (fm-list-directory) |

**Target**: 85%+ (6/7) or 100% (7/7)
