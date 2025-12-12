# FM Mini Suite Assessment - 2255

## Run: fm-mini-20251207-225450

**Pass Rate: 57.1% (4/7)** - Back to previous best!

## Summary

Removing the "call task_complete" hint fixed the regression. fm-create-and-run is passing again.

## Passed Tasks (4)
| Task | Turns | Notes |
|------|-------|-------|
| fm-hello-world | 2 | Simple write |
| fm-append-to-file | 2 | Simple write |
| fm-simple-edit | 2 | Simple edit |
| fm-create-and-run | 7 | Multi-step success! |

## Failed Tasks (3)

### fm-read-and-echo (3 turns) - Content Mangling
**Expected**: Copy source.txt exactly to echo.txt
```
source.txt (74 bytes):
This is the source content.
It has multiple lines.
Please copy it exactly.

echo.txt (50 bytes):
This is the source content. It has multiple lines.
```

**Problem**: FM collapses newlines into spaces and truncates content. Missing "Please copy it exactly." line.

**Root cause**: Unknown - FM is mangling the content when writing. May be an FM model limitation.

### fm-word-count (4 turns) - Never Reads File
**Expected**: Read document.txt (9 words), write "9" to count.txt
```
document.txt: "The quick brown fox jumps over the lazy dog."
count.txt: "0"
```

**Problem**: FM writes "0" without ever reading document.txt.

**Root cause**: FM doesn't understand "count words" requires reading first. Workflow hint exists but FM ignores it.

### fm-list-directory (6 turns) - Output Truncation
**Expected**: listing.txt should contain file1.txt, file2.txt
```
listing.txt content:
total 24
drwxr-xr-x@ 5 christopherdavid  staff  160 Dec  7 22:54 .
drwxr-xr-x@ 3 christopherdavid  staff   96 Dec  7 22:54 ..
-rw-r--r--@ 1 christopherdavid  staff    3 Dec  7 22:54 data.json
```

**Problem**: Output truncated! Shows `total 24` and `5 items` but only 3 files listed.

**Root cause**: Command output preview is 200 chars, truncating the ls output before FM can write it.

## Fixable Issues

### 1. Command Output Truncation (fm-list-directory)
Current limit: 200 chars for command output preview
Fix: Increase to 500+ chars

### 2. Content Mangling (fm-read-and-echo)
FM is collapsing newlines and truncating content.
Possible fixes:
- Preserve exact content in history (not just preview)
- Add explicit "copy EXACTLY including newlines" instruction
- May be FM model limitation

### 3. Planning (fm-word-count)
FM doesn't understand multi-step workflows.
Possible fixes:
- More explicit prompt about reading first
- Task decomposition in orchestrator
- May be FM model limitation

## Progress Tracking

| Run | Pass Rate | Notes |
|-----|-----------|-------|
| 2232 | 14.3% (1/7) | Initial micro-task |
| 2238 | 42.9% (3/7) | Fixed tool parsing |
| 2243 | ~0% | Infinite loops |
| 2248 | 57.1% (4/7) | Repeat detection |
| 2251 | 42.9% (3/7) | Regression from hint |
| 2255 | 57.1% (4/7) | Fixed regression |

## Next Steps

1. **Increase command output limit** - 200 -> 500 chars (may fix fm-list-directory)
2. **Investigate content mangling** - Why does FM collapse newlines?
3. **Accept limitations** - Some tasks may be beyond FM's capability
