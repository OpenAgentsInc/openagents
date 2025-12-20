# Autopilot Improvement Dimensions

A comprehensive list of measurable dimensions on which Autopilot can be evaluated and improved. Each dimension includes what to measure, why it matters, and how to observe it.

---

## 1. Performance & Speed

### 1.1 Inference Latency
- **Metric:** Time from sending prompt to receiving first token
- **Why:** Directly impacts user experience and throughput
- **Measure:** `first_token_ts - request_ts` per turn
- **Target:** <2s for Sonnet, <5s for Opus

### 1.2 Time Between Tool Calls
- **Metric:** Elapsed time between consecutive tool invocations
- **Why:** Slow tool sequencing drags out sessions; fast agents parallelize
- **Measure:** Parse `.json` trajectory, compute `tool_call[n+1].ts - tool_result[n].ts`
- **Target:** <1s between dependent calls, 0s for parallelizable calls

### 1.3 Total Task Duration
- **Metric:** Wall-clock time from session start to completion
- **Why:** End-to-end efficiency matters for throughput
- **Measure:** `@end ts - @start ts` in rlog
- **Target:** Varies by task complexity; establish baselines per task type

### 1.4 Time to First Tool Call
- **Metric:** How quickly agent moves from thinking to action
- **Why:** Indicates whether agent is over-analyzing vs. acting
- **Measure:** First `t!:` line timestamp - `@start` timestamp
- **Target:** <5s for simple tasks

### 1.5 Tool Call Parallelization Rate
- **Metric:** % of independent tool calls made in parallel vs sequential
- **Why:** Parallel calls complete faster; sequential wastes time
- **Measure:** Count tool calls with same timestamp vs different timestamps
- **Target:** >80% parallelization for independent calls

---

## 2. Reliability & Error Handling

### 2.1 Tool Error Rate
- **Metric:** % of tool calls that return errors
- **Why:** Errors waste tokens and time; indicate poor planning
- **Measure:** Count `[error]` vs `[ok]` outcomes in rlog
- **Common errors observed:**
  - `EISDIR` - trying to read a directory as file
  - `File has not been read yet` - edit without prior read
  - `File does not exist` - invalid paths
  - `No changes to make` - redundant edits
- **Target:** <5% error rate

### 2.2 Crash Recovery Rate
- **Metric:** % of crashed sessions that successfully resume
- **Why:** Crashes are inevitable; recovery is critical
- **Measure:** Track `resume` commands and their outcomes
- **Target:** >95% successful resume

### 2.3 Log Completeness
- **Metric:** % of trajectories with complete metadata
- **Why:** Incomplete logs break analysis and resume
- **Measure:** Check for missing `id`, `tokens_total_*` in headers
- **Observed issues:** Blank session_id, zero token counts
- **Target:** 100% completeness

### 2.4 Finalization on Abnormal Termination
- **Metric:** Whether `@end` is written on crash/budget exhaustion
- **Why:** Partial logs are hard to analyze
- **Measure:** Check if rlog ends with `@end` or abruptly
- **Target:** Always write `@end` with reason

### 2.5 Duplicate Log Prevention
- **Metric:** Number of duplicate logs for same session
- **Why:** Duplicates waste storage and confuse analysis
- **Observed:** Multiple logs for same run (2113/2122, 2125/2138, 2205/2231)
- **Target:** Exactly one log per session

---

## 3. Cost Efficiency

### 3.1 Tokens Per Task
- **Metric:** Total input + output tokens per completed task
- **Why:** Tokens = cost; efficiency saves money
- **Measure:** `tokens_total_in + tokens_total_out` from trajectory
- **Target:** Establish baselines per task type, aim for -10% quarter-over-quarter

### 3.2 Cache Hit Rate
- **Metric:** `tokens_cached / (tokens_cached + tokens_total_in)`
- **Why:** Cache hits are cheaper than fresh tokens
- **Measure:** From trajectory metadata
- **Observed:** ~98% in good runs
- **Target:** >90% cache hit rate

### 3.3 Model Selection Efficiency
- **Metric:** Cost per completed task by model tier
- **Why:** Using Opus for simple tasks wastes money
- **Measure:** Track `model` field and task outcomes
- **Target:** Use Haiku for exploration, Sonnet for implementation, Opus for complex architecture

### 3.4 Redundant Tool Calls
- **Metric:** Tool calls that don't contribute to task completion
- **Why:** Wasted tokens and time
- **Examples:**
  - Reading same file multiple times
  - Searching `target/` directory
  - Failed edits followed by re-reads
- **Target:** <10% redundant calls

### 3.5 Context Window Utilization
- **Metric:** How much of context window is used before compaction
- **Why:** Early compaction loses information; late compaction hits limits
- **Target:** Compact at 80-90% utilization

---

## 4. Quality of Output

### 4.1 Task Completion Rate
- **Metric:** % of claimed issues that reach `completed` status
- **Why:** The ultimate measure of agent utility
- **Measure:** `issue_complete` calls / `issue_claim` calls
- **Target:** >90% completion rate

### 4.2 Build Success Rate
- **Metric:** % of code changes that pass `cargo build`
- **Why:** Code that doesn't compile is useless
- **Measure:** Track build outcomes after edits
- **Target:** >95% first-time build success

### 4.3 Test Pass Rate
- **Metric:** % of code changes that pass `cargo test`
- **Why:** Tests validate correctness
- **Measure:** Track test outcomes after implementation
- **Observed issues:** Pre-existing test failures blamed; new failures not fixed
- **Target:** 100% test pass for new code

### 4.4 Fix Iteration Count
- **Metric:** Number of edit cycles before success
- **Why:** More iterations = more tokens and time
- **Measure:** Count `Edit` → `Bash (build)` → `Edit` cycles
- **Target:** <3 iterations per fix

### 4.5 Commit Revert Rate
- **Metric:** % of autopilot commits that get reverted
- **Why:** Measures real-world code quality
- **Measure:** Track `git revert` of autopilot commits
- **Target:** <5% revert rate

---

## 5. Autonomy & Self-Direction

### 5.1 Issues Completed Per Session
- **Metric:** Number of issues finished in a single autopilot run
- **Why:** True autonomy means multi-issue completion
- **Measure:** Count `issue_complete` calls per session
- **Target:** >3 issues per session (for full-auto mode)

### 5.2 Self-Recovery Rate
- **Metric:** % of errors that agent recovers from without human help
- **Why:** Autonomous agents should handle errors
- **Measure:** Track error → recovery sequences
- **Target:** >80% self-recovery

### 5.3 Issue Discovery Rate
- **Metric:** Issues created when queue is empty
- **Why:** True autonomy includes finding work
- **Measure:** `issue_create` calls when `issue_ready` returns empty
- **Target:** 1+ discovery per empty-queue encounter

### 5.4 Appropriate Blocking Behavior
- **Metric:** % of unrecoverable failures that call `issue_block`
- **Why:** Blocking with context enables human intervention
- **Observed issues:** Runs end without blocking
- **Target:** 100% blocking on unrecoverable failures

### 5.5 Loop Continuation Rate
- **Metric:** % of full-auto sessions that continue after first issue
- **Why:** Full-auto should keep working
- **Measure:** Track sessions with multiple issue cycles
- **Target:** 100% continuation until budget exhausted

---

## 6. Safety & Guardrails

### 6.1 Unsafe Operation Prevention Rate
- **Metric:** % of unsafe operations blocked
- **Why:** Prevent data loss and integrity issues
- **Operations to block:**
  - Direct sqlite3 writes (INSERT/UPDATE/DELETE)
  - `git push --force`
  - `git reset --hard`
  - `rm -rf` in critical paths
- **Observed issues:** sqlite3 deletes used to "fix" data
- **Target:** 100% block rate

### 6.2 Secret Exposure Rate
- **Metric:** Secrets appearing in logs or commits
- **Why:** Security risk
- **Measure:** Scan trajectories for API keys, passwords
- **Target:** 0% exposure

### 6.3 Read-Before-Edit Enforcement
- **Metric:** % of edits preceded by reads
- **Why:** Editing without reading causes errors
- **Observed:** "File has not been read yet" errors
- **Target:** 100% read-before-edit

### 6.4 Path Validation Rate
- **Metric:** % of file operations with valid paths
- **Why:** Invalid paths waste tool calls
- **Observed:** EISDIR errors, reading directories
- **Target:** 100% valid paths

### 6.5 Git Safety Compliance
- **Metric:** Adherence to git safety rules
- **Why:** Prevent repository damage
- **Rules:**
  - Never push --force to main
  - Never skip hooks
  - Never commit secrets
- **Target:** 100% compliance

---

## 7. Observability & Debugging

### 7.1 Trajectory Completeness Score
- **Metric:** % of required fields present in trajectory
- **Why:** Complete trajectories enable debugging and analysis
- **Required fields:** session_id, model, cwd, tokens, steps
- **Target:** 100% completeness

### 7.2 Thinking Block Preservation
- **Metric:** % of thinking blocks captured with signatures
- **Why:** Thinking blocks explain decision-making
- **Measure:** Count `th:` lines with `sig=`
- **Target:** 100% preservation

### 7.3 Tool Input/Output Fidelity
- **Metric:** Complete capture of tool inputs and outputs
- **Why:** Enables replay and debugging
- **Measure:** Compare `.json` tool_call/tool_result completeness
- **Target:** 100% fidelity

### 7.4 Replay Accuracy
- **Metric:** Can a trajectory be replayed to understand the session?
- **Why:** Post-hoc debugging requires accurate replay
- **Measure:** Human evaluation of replay usefulness
- **Target:** Full session reconstructable from logs

### 7.5 Cost Attribution Accuracy
- **Metric:** Token counts per step sum to session total
- **Why:** Accurate cost tracking for budgeting
- **Measure:** `sum(step.tokens) == session.tokens_total`
- **Target:** 100% accuracy

---

## 8. Workflow Compliance

### 8.1 Git Workflow Adherence
- **Metric:** % of changes following branch → PR → merge pattern
- **Why:** Proper workflow enables review and rollback
- **Observed issues:** Direct commits to main
- **Target:** 100% branch/PR workflow

### 8.2 Issue Tracking Integrity
- **Metric:** No duplicate issue numbers, no NULL IDs
- **Why:** Data integrity for issue tracking
- **Observed:** Duplicate numbers, NULL ids, failed claims
- **Target:** 0% integrity violations

### 8.3 Documentation Completeness
- **Metric:** % of code changes with appropriate documentation
- **Why:** Maintainability
- **Measure:** Check for commit messages, code comments where needed
- **Target:** All non-trivial changes documented

### 8.4 Commit Message Quality
- **Metric:** Commit messages explain "why" not just "what"
- **Why:** Future maintainability
- **Measure:** Human evaluation or keyword analysis
- **Target:** >90% quality score

### 8.5 PR Description Quality
- **Metric:** PRs have summary, test plan, context
- **Why:** Enables effective review
- **Measure:** Check for required sections
- **Target:** 100% complete PR descriptions

---

## 9. Prompt & Instruction Effectiveness

### 9.1 First-Try Success Rate
- **Metric:** % of tasks completed without prompt refinement
- **Why:** Good prompts reduce iterations
- **Measure:** Track re-runs of same task with modified prompts
- **Target:** >80% first-try success

### 9.2 Instruction Adherence Rate
- **Metric:** % of explicit instructions followed
- **Why:** Agent should follow user intent
- **Measure:** Compare outputs to instructions
- **Observed issues:** Ignoring AGENTS.md rules
- **Target:** 100% adherence

### 9.3 Context Utilization
- **Metric:** Use of relevant information from context
- **Why:** Good agents leverage available information
- **Measure:** Track references to context in reasoning
- **Target:** Appropriate context usage

### 9.4 Tool Selection Accuracy
- **Metric:** % of tool calls using optimal tool
- **Why:** Wrong tools waste time
- **Examples:**
  - Using Bash grep instead of Grep tool
  - Reading directories instead of globbing
- **Target:** >95% optimal tool selection

### 9.5 Compaction Quality
- **Metric:** Critical information preserved after compaction
- **Why:** Poor compaction loses important context
- **Measure:** Check for required info in compacted summaries
- **Target:** 100% preservation of critical info

---

## 10. Resource Utilization

### 10.1 Memory Footprint
- **Metric:** Peak memory usage during session
- **Why:** Excessive memory causes crashes
- **Observed:** Terminal memory leak crash
- **Target:** <2GB peak memory

### 10.2 CPU Efficiency
- **Metric:** CPU usage during idle periods
- **Why:** Idle polling wastes resources
- **Target:** Near-zero CPU when waiting for API

### 10.3 Network Efficiency
- **Metric:** Bytes transferred per token
- **Why:** Network overhead adds latency
- **Target:** Minimal overhead

### 10.4 Concurrent Agent Efficiency
- **Metric:** Performance with multiple subagents
- **Why:** Parallelism should scale
- **Measure:** Time savings from parallel subagents
- **Target:** Near-linear scaling up to 3 agents

### 10.5 MCP Server Startup Time
- **Metric:** Time to start MCP server and connect
- **Why:** Startup overhead affects short tasks
- **Target:** <1s startup

---

## Measurement Infrastructure Needed

To track these dimensions, we need:

1. **Trajectory Analyzer** (`cargo autopilot analyze`)
   - Parse .json/.rlog files
   - Compute all metrics above
   - Output summary report

2. **Session Comparator** (`cargo autopilot compare`)
   - Compare two trajectories
   - Identify regression/improvement

3. **Aggregate Dashboard**
   - Track metrics over time
   - Identify trends
   - Alert on regressions

4. **Benchmark Suite**
   - Standard tasks with known solutions
   - Measure performance on consistent inputs
   - Track changes across versions

---

## Priority Matrix

| Dimension | Impact | Effort | Priority |
|-----------|--------|--------|----------|
| Tool Error Rate | High | Low | P0 |
| Task Completion Rate | High | Medium | P0 |
| Unsafe Operation Prevention | High | Low | P0 |
| Log Completeness | Medium | Low | P1 |
| Read-Before-Edit Enforcement | Medium | Low | P1 |
| Git Workflow Adherence | Medium | Medium | P1 |
| Cache Hit Rate | High | Medium | P1 |
| First-Try Success Rate | High | High | P2 |
| Parallelization Rate | Medium | Medium | P2 |
| Memory Footprint | Low | High | P3 |

---

## Next Steps

1. Implement basic metrics collection in trajectory output
2. Create `cargo autopilot analyze` command
3. Establish baselines for each metric
4. Set up automated regression testing
5. Create dashboard for tracking over time
