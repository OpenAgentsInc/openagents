# FM Capability Analysis: Can FM Do Real Terminal-Bench?

**Date**: 2024-12-07 23:49
**Author**: Claude (implementing agent)
**Purpose**: Steelman analysis of FM's potential for real Terminal-Bench tasks

---

## The Claim I Made

> "But honestly, most real TB tasks (path-tracing, video-processing, COBOL, etc.) are way beyond FM's capability"

This document examines whether this claim is justified or premature dismissal.

---

## Part 1: What FM Has Actually Demonstrated

### Successes (fm-mini-suite 100%)

| Task | What FM Did | Complexity |
|------|-------------|------------|
| hello-world | `write_file("hello.txt", "Hello, world!")` | Single tool call |
| read-and-echo | `cp source.txt echo.txt` (via hint) | Shell command |
| append-to-file | `echo "[DONE]" >> log.txt` (via hint) | Shell command |
| list-directory | `ls -la` then `write_file` | Multi-step |
| create-and-run | write script → run → capture output → write result | 4+ steps, error recovery |
| simple-edit | `edit_file` with old/new text | Single tool call |
| word-count | `wc -w file.txt` (via hint) | Shell command |

### Key Insight: FM + Shell = Powerful

FM's native capabilities are limited, but **FM can invoke arbitrary shell commands**. This means:

- `wc -w` for counting
- `cp` for exact file copying
- `grep`, `sed`, `awk` for text processing
- `python3 -c "..."` for computation
- `gcc`, `make` for compilation
- `pytest` for testing

**FM is not doing the work. FM is orchestrating tools that do the work.**

---

## Part 2: Why I Said FM Can't Do Real TB

### Reason 1: Task Description Complexity

Real TB task descriptions are 500-2000+ characters of dense technical requirements.

Example (regex-log):
```
Write a regex expression that matches dates in the format YYYY-MM-DD 
appearing in lines that contain an IPv4 address in a log file.
If multiple dates are present in a line, the regex should match only 
the last date in that line...
```

**My assumption**: FM can't understand complex multi-part requirements.

**Counter-argument**: 
- FM doesn't need to understand everything at once
- Orchestrator can decompose into steps
- Hints can guide specific sub-tasks
- FM just needs to pick the right tool for each step

### Reason 2: Cognitive Tasks (Reasoning, Math, Planning)

FM failed at counting words without `wc -w` hint.

**My assumption**: FM can't do any reasoning-heavy tasks.

**Counter-argument**:
- We SOLVED this with shell commands
- Path-tracing doesn't need FM to understand ray tracing - FM can write code that a compiler understands
- COBOL doesn't need FM to understand COBOL - FM can use existing conversion tools or write Python incrementally
- The pattern: **FM writes, tools verify, FM iterates**

### Reason 3: Large Context Requirements

Real TB tasks have:
- Multi-file codebases
- Large input files
- Complex verification suites

**My assumption**: FM's 4096 token limit can't handle this.

**Counter-argument**:
- FM only needs to see ONE thing at a time
- Orchestrator manages state externally
- Chunked reading is already implemented
- We've shown prompts of ~1000-2000 chars work fine

### Reason 4: Iterative Debugging

Real tasks require:
- Write code → compile → see errors → fix → repeat
- This seems to need "understanding"

**My assumption**: FM can't debug.

**Counter-argument**:
- fm-create-and-run SHOWED FM can iterate on errors:
  - Turn 1: `run greet.sh` → "No such file"
  - Turn 2: `write_file greet.sh` → created
  - Turn 3: `./greet.sh` → "Permission denied"
  - Turn 4: `chmod +x` or `bash greet.sh`
- **FM doesn't need to "understand" the error. FM just needs to try something different.**
- Error messages in `Previous` field guide next action

### Reason 5: Domain Knowledge

Tasks like DNA assembly, video processing, path tracing require specialized knowledge.

**My assumption**: FM doesn't have domain knowledge.

**Counter-argument**:
- FM doesn't need domain knowledge if we provide it via hints
- Shell commands exist for many domains (ffmpeg for video, bioinformatics tools, etc.)
- For path-tracing: FM writes C code, gcc compiles it, we check output
- FM can iterate: write → compile → run → compare → adjust

---

## Part 3: What Would FM Actually Need?

### For regex-log (medium):
1. Hint: "Use Python re module to build and test regex"
2. FM writes: `python3 -c "import re; pattern = '...'; print(pattern)" > regex.txt`
3. FM tests: `python3 test_regex.py`
4. FM iterates based on errors

**Verdict**: Plausibly doable with hints

### For path-tracing (hard):
1. Task: Write C code that generates specific PPM image
2. Steps FM would need:
   - Read reference image (or get description of it)
   - Write C code with ray tracing math
   - Compile with gcc
   - Run and compare output
   - Iterate on differences

**Challenge**: The math/algorithm is the hard part. FM would need to either:
- Have ray tracing formulas in hints
- Copy from example code
- Get lucky with generation

**Verdict**: Very hard, but not impossible if we provide algorithmic hints

### For video-processing (hard):
1. Task: Analyze video, detect jump frames
2. Steps:
   - Use ffmpeg to extract frames
   - Use Python + OpenCV to analyze
   - Detect motion patterns
   - Write TOML output

**Challenge**: Computer vision algorithms need to be correct.

**Verdict**: Hard, but ffmpeg + opencv are powerful. With the right hints about frame differencing, possibly doable.

### For COBOL modernization (easy according to TB):
1. Task: Convert COBOL to Python
2. Steps:
   - Read COBOL file
   - Write Python equivalent
   - Test with same inputs

**Challenge**: Understanding COBOL semantics.

**Counter**: FM could:
- Use existing COBOL-to-Python tools if they exist
- Write Python incrementally, testing each part
- The test suite tells FM if it's right

**Verdict**: Possibly doable with iteration

---

## Part 4: The Real Blockers

### 1. Hint Engineering at Scale

We manually wrote hints for 7 tasks. Real TB has 89 tasks. 

**Problem**: We can't hand-write hints for every task type.
**Solution needed**: Automatic hint generation or task classification

### 2. Token Efficiency

FM uses ~1000 tokens per turn. Complex tasks might need 50+ turns.
50 turns × 1000 tokens = 50K tokens per task.

**Problem**: Slow and potentially expensive at scale.
**Solution needed**: Better early termination, smarter orchestration

### 3. Flakiness

We saw 100% pass rate sometimes, 71% other times on the SAME suite.

**Problem**: FM's output is non-deterministic.
**Solution needed**: Retry logic, ensemble approaches, or acceptance of ~80% reliability

### 4. Verification Environment

Real TB tasks need Docker environments with specific tools installed.

**Problem**: Local runs don't have these environments.
**Solution needed**: Use tbench-sandbox.ts or tbench-iterate.ts with proper Docker setup

---

## Part 5: Revised Assessment

### What FM CAN'T Do (Fundamental Limits)
1. Complex mathematical reasoning without hints/tools
2. Understanding novel algorithms from scratch
3. Tasks requiring >4096 tokens of simultaneous context
4. Tasks with no shell-tool solution

### What FM CAN Do (With Proper Orchestration)
1. File operations (read, write, edit, copy)
2. Shell command orchestration (any Unix tool)
3. Iterative development (write → test → fix loop)
4. Multi-step tasks (proven with fm-create-and-run)
5. Error recovery (proven with permission denied → chmod)

### What FM MIGHT Do (Needs Testing)
1. Simple regex tasks (with Python hints)
2. File format conversions (with tool hints)
3. Log analysis (with grep/awk hints)
4. Simple code generation (with iteration)
5. Build/compile tasks (with Makefile hints)

---

## Part 6: Concrete Next Steps

### Immediate (Test FM on Real TB)
1. Set up Docker environment for TB tasks
2. Run FM on 5-10 "easy" TB tasks
3. Collect failure modes
4. Add hints for common patterns

### Medium Term (Scale Hints)
1. Classify TB tasks by type (file-op, code-gen, analysis, etc.)
2. Create hint templates per type
3. Auto-detect task type from description
4. Inject appropriate hints

### Long Term (Self-Improving FM)
1. Learn from failures (what hints helped?)
2. Build skill library from successes
3. FM suggests its own hints based on task similarity

---

## Conclusion

**My original claim was premature.**

I said FM can't do real TB tasks based on:
- FM's cognitive limitations (can't count, can't reason)
- Task complexity (long descriptions, domain knowledge)
- Environment requirements (Docker, tools)

But I failed to account for:
- **FM + Shell = arbitrary computation**
- **Orchestrator handles state and iteration**
- **Hints can encode domain knowledge**
- **Error feedback enables debugging**

The question is not "Can FM understand path-tracing?"
The question is "Can FM write C code that compiles, run it, and iterate until it works?"

With sufficient hint engineering and proper Docker setup, **FM might be able to do more TB tasks than I assumed.**

**Recommended action**: Actually try FM on real TB tasks before concluding it can't work.

---

## Appendix: Tasks to Try First

Based on this analysis, FM has the best chance at:

1. **regex-log** - Python + re module, iteration possible
2. **fix-git** - Git commands, error messages guide fixes
3. **log-summary-date-ranges** - grep/awk/Python file processing
4. **polyglot-c-py** - Write code, compile, test, iterate
5. **large-scale-text-editing** - Vim macros, could use sed/awk instead

Tasks to avoid initially:
1. **path-tracing** - Requires correct algorithm
2. **video-processing** - Requires CV knowledge
3. **dna-assembly** - Requires biology knowledge
4. **model-extraction** - Requires ML knowledge

The "easy" rated TB tasks might be misleading - "easy" often means "easy for a skilled human" not "easy for an LLM."
