# BlackBox

Flight recorder format for Autopilot sessions.

---

## FORMAT

Line-based. Each line is a step. Prefix indicates type.

```
prefix: content [→ result]
```

File extension: `.bbox` or `.blackbox`

### Prefixes

| Prefix | Meaning | Format |
|--------|---------|--------|
| `u` | User message | `u: message text` |
| `a` | Agent message | `a: response text` |
| `t` | Tool call | `t:tool id=call_1 args → result` |
| `o` | Observation | `o: id=call_1 → async result` |
| `s` | Skill | `s:name action → result` |
| `p` | Plan | `p:action id=p1 "title" → result` |
| `m` | Mode | `m:chat`, `m:auto`, `m:plan` |
| `r` | Recall/memory | `r:query → result` |
| `x` | Subagent | `x:type id=sub_1 "task" → summary` |
| `c` | MCP call | `c:server.method id=call_1 args → result` |
| `q` | Question | `q: "Which approach?" → [selected: OAuth]` |
| `#` | Comment/meta | `# session_id=abc model=sonnet` |
| `@` | Lifecycle | `@start`, `@checkpoint`, `@compact` |

### Call IDs and Observations (ATIF Compatibility)

Every tool/MCP/subagent call can have an `id=` for correlation. Results can be inline OR deferred via `o:` observation lines:

```
# Inline result (simple case, id optional)
t:read src/main.rs → [200 lines]

# With call ID (for async/streaming/correlation)
t:grep id=call_91 pattern="TODO" → [12 matches]

# Deferred observation (result arrives later)
t:test id=call_92 cargo test → [running]
# ... other operations ...
o: id=call_92 → [ok] 128 tests passed

# Multiple observations per call (streaming)
t:build id=call_93 cargo build → [started]
o: id=call_93 partial="Compiling crate 1/5..."
o: id=call_93 partial="Compiling crate 2/5..."
o: id=call_93 → [ok] Built in 34s
```

**Rule**: Inline `→ result` is sugar for immediate observation. Use `o:` when results are async, streamed, or arrive out-of-order.

### Step IDs and Timestamps

For joinability with dashboards and ATIF export, add `step=` and `ts=` to lines:

```
u: What's the stock price? step=1 ts=2025-12-18T03:21:08Z
a: I'll look that up. step=2 ts=2025-12-18T03:21:10Z
t:search id=call_1 ticker=GOOGL step=2 ts=2025-12-18T03:21:11Z → $185.35
```

**Elapsed time** (`# t=HH:MM:SS`) is a different axis - use both when needed:
- `ts=` for wall-clock (debugging, dashboards)
- `# t=` for session-relative (human scanning)

### Per-Step Metrics

Inline metrics on agent steps for cost debugging:

```
a: Here's what I found... step=3 # metrics prompt=520 completion=80 cached=200 cost=0.00045
```

Or as standalone comment after the step:
```
a: Here's what I found...
# metrics step=3 prompt_tokens=520 completion_tokens=80 cached_tokens=200 cost_usd=0.00045
```

For RL/SFT training data, use blob pointers for token IDs and logprobs:
```
# metrics step=3 completion_token_ids=@blob sha256=... logprobs=@blob sha256=...
```

Provider-specific fields go in `extra.*`:
```
# metrics step=3 ... extra.cache_creation_input_tokens=1234 extra.reasoning_tokens=12
```

### Subagent Trajectory References

Make subagent results linkable to external trajectory files:

```
x:explore id=sub_7 "find failing tests" → session_id=sess_sub_7 path=trajectories/sess_sub_7.bbox summary="Found 3 failures"
```

Fields:
- `session_id=` - The subagent's session ID
- `path=` - File path, S3 URL, or database reference
- `summary=` - Quick summary (optional if path provided)

### Notes Field

Explain weirdness, format discrepancies, or collapsed steps:

```
# notes: CI outage caused 2h gap; steps 45-47 were manual intervention
# notes: Token counts don't match because provider changed mid-session
```

In header: `notes: "Initial test trajectory, some tools mocked"`

### Extra Fields (Extensibility)

Escape hatch for custom metadata without forking the format:

```
# Header level
extra.eval_suite: terminalbench
extra.task_id: tb_042

# Line level
t:read path=... extra.cache=hit → [200 lines]
a: Done. extra.confidence=0.95 extra.model_version=sonnet-4.1
```

### Lifecycle Primitives (Long Sessions)

For sessions longer than a few minutes, use lifecycle primitives:

| Primitive | Purpose | Format |
|-----------|---------|--------|
| `@start` | Begin session | `@start id=sess_001 budget=$50 duration=12h` |
| `@checkpoint` | Progress marker | `@checkpoint hour=4 tokens=45000 cost=$12.30` |
| `@pause` | Halt for external | `@pause reason="waiting for CI"` |
| `@resume` | Continue after pause | `@resume` |
| `@end` | Complete session | `@end summary="..." prs=[201,202] issues=8` |
| `@assess` | Priority assessment | `@assess → P0: task, P1: task...` |
| `@notify` | Alert human | `@notify "PR #47 ready for review"` |
| `@wait` | Block for approval | `@wait-approval pr=47 timeout=2h → approved` |
| `@escalate` | Request help | `@escalate "Tests failing, need human"` |
| `@batch` | Group related | `@batch [issue-142, issue-156] reason="related"` |
| `@snapshot` | Named state | `@snapshot id=after_pr "PR created, tests pass"` |
| `@rotate` | Split log file | `@rotate file=sess_001_part2.bbox` |
| `@compact` | Context compaction | `@compact reason="context limit" tokens_before=180000` |
| `@phase` | Plan mode phase | `@phase explore`, `@phase design`, `@phase exit` |

### Plan Mode (Claude Code Style)

Plan mode is a structured workflow for complex tasks. Based on Claude Code's 5-phase approach.

#### Modes

```
m:chat                  # Default conversational mode
m:auto                  # Autonomous execution mode
m:plan                  # Plan mode (read-only except plan file)
```

#### Phase Transitions

Within plan mode, use `@phase` to mark workflow stages:

```
m:plan
@phase explore          # Phase 1: understand codebase
@phase design           # Phase 2: design approach
@phase review           # Phase 3: review and clarify
@phase final            # Phase 4: write final plan
@phase exit             # Phase 5: exit plan mode
```

#### Phase 1: Explore (Read-Only)

Launch up to 3 Explore subagents in parallel to understand the codebase:

```
m:plan
@phase explore

# Single explore agent for focused task
x:explore id=sub_1 "understand auth module" → [launched]
o: id=sub_1 → files=[src/auth.rs, src/middleware/auth.rs] patterns=["JWT validation", "session tokens"]

# Multiple parallel explores for complex task
x:explore id=sub_2 tid=2 "find existing implementations" → [launched]
x:explore id=sub_3 tid=3 "explore related components" → [launched]
x:explore id=sub_4 tid=4 "investigate test patterns" → [launched]
o: id=sub_2 tid=2 → summary="Found 3 auth implementations"
o: id=sub_3 tid=3 → summary="5 related services identified"
o: id=sub_4 tid=4 → summary="Integration tests in tests/auth/"
```

#### Phase 2: Design

Launch Plan subagents to design implementation approaches:

```
@phase design

# Single plan agent
x:plan id=sub_5 "design auth refactor" context="..." → [launched]
o: id=sub_5 → plan="1. Extract interface 2. Add adapter 3. Migrate"

# Multiple perspectives for complex decisions
x:plan id=sub_6 tid=2 perspective="simplicity" → [launched]
x:plan id=sub_7 tid=3 perspective="performance" → [launched]
x:plan id=sub_8 tid=4 perspective="maintainability" → [launched]
```

#### Phase 3: Review

Read critical files, ensure alignment with user intent:

```
@phase review

t:read id=call_20 src/auth.rs → [186 lines]
t:read id=call_21 src/services/user.rs → [234 lines]

# Clarify with user
q: id=q_1 "Which auth library should we use?" options=["JWT", "OAuth", "Session"] → [pending]
q: id=q_1 → [selected: OAuth]
```

#### Phase 4: Final Plan

Write the plan file (only writable file in plan mode):

```
@phase final

p:file path=~/.claude/plans/auth-refactor.md
p:write id=p1 "# Auth Refactor Plan\n\n## Steps\n1. ..." → [ok]
```

#### Phase 5: Exit Plan Mode

```
@phase exit approved=true

# Or with swarm launch
@phase exit approved=true swarm=true teammates=3

m:auto  # Now in execution mode
```

#### Questions and Clarifications

Use `q:` for structured user questions:

```
# Ask question with options
q: id=q_1 "Which database?" options=["Postgres", "SQLite", "Redis"] → [pending]

# User responds
q: id=q_1 → [selected: Postgres]

# Free-form question
q: id=q_2 "What's the expected load?" → [pending]
q: id=q_2 → "About 1000 req/s"

# Multi-select
q: id=q_3 "Which features?" options=["Auth", "Billing", "API"] multi=true → [pending]
q: id=q_3 → [selected: Auth, API]
```

#### Autonomous Mode (AFK)

When user is away, questions don't block. Agent decides autonomously and logs rationale:

```
m:auto afk=true

# Question that would normally block
q: id=q_4 "Which auth approach?" options=["JWT", "OAuth", "Session"] → [auto]

# Agent gathers context to decide
t:read id=call_30 src/auth/current.rs → [OAuth already in use]
t:grep id=call_31 "OAuth" type=rs → [12 matches]

# Agent makes autonomous decision with rationale
q: id=q_4 → [auto: OAuth, reason="existing OAuth infrastructure in 12 files"]

# Queued for human review when back
@notify "Auto-decided OAuth for auth. Review decision?"
```

Key patterns for AFK mode:
- `afk=true` on mode transition
- `[auto]` result on questions (not `[pending]`)
- Agent gathers more context before deciding
- `reason="..."` explains the autonomous choice
- `@notify` queues for human review later

#### Context Management

Multiple strategies for managing context as sessions grow. BlackBox logs what happened regardless of strategy.

**Option 1: Hard Compaction (Claude Code style)**
Single disruptive break, everything before summarized:
```
@compact reason="context limit" tokens_before=180000 tokens_after=8000
a: [COMPACT SUMMARY] ... [/COMPACT SUMMARY]
```

**Option 2: Progressive Condensation**
Older context progressively summarized, no hard break:
```
# condense range=50-100 detail=summary
# condense range=1-49 detail=outline
# Keeps recent steps in full detail, older steps condensed
```

**Option 3: Anchored Forgetting**
Drop low-value context, preserve high-value "anchors":
```
# anchor step=75 reason="key decision: chose OAuth"
# anchor step=92 reason="user feedback: prefer simpler approach"
# anchor step=108 reason="error fixed: type mismatch"
# Low-value steps (successful reads, expected results) can be dropped
```

**Option 4: Memory + Retrieval**
Store everything externally, retrieve on demand:
```
# memory store=all target=vectordb
r: "auth implementation decisions" → [3 matches from steps 45, 67, 89]
r: "user feedback on approach" → [2 matches]
# Context stays small, relevant history retrieved as needed
```

Key insight: BlackBox captures what happened. Context management strategy is an implementation choice—different agents may use different approaches.

#### Model Switching

Track model changes for cost analysis:

```
# model: haiku reason="topic detection"
# model: opus reason="plan mode"
# model: sonnet reason="code generation"
```

### Time Tracking

Use `# t=HH:MM:SS` comments to mark elapsed time:

```
# t=00:00:00
@start budget=$50
a: Starting autonomous session...

# t=00:05:23
t:read src/main.rs → [200 lines]

# t=04:00:00
@checkpoint hour=4 tokens=45000 cost=$12.30
```

### Budget Tracking

Track token usage and costs inline:

```
# budget: $50.00 remaining=$47.23 tokens=12847
```

### Concurrency (tid/span)

When multiple operations run in parallel (subagents, concurrent tool calls), add correlation fields:

| Field | Purpose |
|-------|---------|
| `tid=` | Thread/agent ID |
| `span=` | Operation ID |
| `parent=` | Parent span for nesting |

```
# Main agent spawns subagent
x:explore tid=2 span=x17 "scan auth failures" → [started]

# Subagent's tool calls reference its tid
t:rg tid=2 span=t91 "AuthError" path=crates → [12 matches]
t:read tid=2 span=t92 src/auth.rs → [186 lines]

# Subagent completes
x:explore tid=2 span=x17 → summary="Found 3 auth error patterns"

# Main agent continues (tid=1 implicit)
a: Based on subagent findings...
```

**Rule**: `tid=1` is implicit for main agent. Omit when single-threaded.

### Streaming Operations

Long-running tools use start/progress/complete markers:

| Marker | Meaning |
|--------|---------|
| `t!:` | Tool started |
| `t~:` | Incremental output |
| `t:` | Tool completed |

```
t!:test span=t200 cargo test → [running]
t~:test span=t200 "[12/128 passed]"
t~:test span=t200 "[64/128 passed]"
t:test span=t200 latency_ms=34200 → [ok] 128 tests

# Error with retry
t!:git span=t201 push → [running]
t:git span=t201 attempt=1 latency_ms=8430 → [err: permission denied]
t!:git span=t201 push → [retry 2/3]
t:git span=t201 attempt=2 latency_ms=9120 → [ok]
```

### Blob Pointers

Large outputs are stored externally and referenced by hash:

```
→ @blob sha256=ab12cd34... bytes=48219 mime=text/plain
```

Blobs live in `.bbox-blobs/` directory or object store.

```
t:read src/lib.rs → @blob sha256=ab12cd34 bytes=48219
t:git diff → @blob sha256=ef56gh78 bytes=12847 mime=text/x-diff

# Inline small, blob large
t:read README.md → [42 lines]
t:read src/huge.rs → @blob sha256=... bytes=128000
```

**Threshold**: Inline ≤1KB, blob >1KB (configurable).

### Outcome Signals

Add metadata for debugging and metrics:

| Field | Purpose |
|-------|---------|
| `latency_ms=` | Execution time |
| `attempt=N` or `attempt=N/M` | Retry tracking |
| `level=debug\|info\|warn\|error` | Severity |

```
t:cargo build latency_ms=12340 → [ok]
t:curl api.example.com attempt=2/3 latency_ms=1200 → [ok]
t:test level=warn → [ok] 3 tests skipped
```

### Secret Redaction

Tools must never emit raw secrets. Use redaction markers:

```
# Redacted form
t:read .env → API_KEY=[redacted:env_var] DB_URL=[redacted:url]

# Explicit type
t:shell echo $STRIPE_KEY → [redacted:stripe_key]

# In agent output
a: Configured auth with [redacted:github_token]
```

**Header**: Track redactions with `# redactions=N` at end of session.

### Syntax Rules

1. **Truncation**: Long content uses `...` or `[N lines]`
2. **Results**: `→` separates call from result
3. **Args**: `key=val` or positional, space-separated
4. **Quotes**: Only needed if value has spaces
5. **Multiline**: Indent continuation with 2 spaces

### Result Shorthands

| Shorthand | Meaning |
|-----------|---------|
| `[ok]` | Success, no notable output |
| `[N files]` | N files found/matched |
| `[N lines]` | N lines of content |
| `[err: msg]` | Error with message |
| `[skip]` | Skipped or cached |

---

## HEADER (required for replay)

YAML-style metadata block with version and execution context:

```
---
format: bbox/1
id: sess_abc123
mode: chat
model: sonnet-4

# Execution substrate (for deterministic replay)
repo: OpenAgentsInc/platform
repo_sha: 215db51
branch: main
dirty: false
sandbox_id: dtn_abc123
runner: daytona

# Capabilities
skills: [code-review, deploy-vercel]
mcp: [github, linear]
toolset: {git: 2.45, rg: 14.1, cargo: 1.82}

# Limits
budget: $50
duration: 12h

# Security
classification: internal

# Extensibility
notes: "Test trajectory for admin dashboard feature"
extra.eval_suite: internal-qa
extra.priority: P1
---
```

**Required fields**: `format`, `id`, `repo_sha`
**Recommended**: `runner`, `sandbox_id`, `branch`
**Optional**: `notes`, `extra.*`

---

## EXAMPLE: Ad Monetization Feature

User asks Autopilot to add ad monetization to billing tier.

```
---
format: bbox/1
id: sess_20250618_001
mode: chat
model: sonnet-4
repo: acme/billing-app
repo_sha: 7a3b2c1
branch: main
runner: daytona
sandbox_id: dtn_billing_001
skills: []
mcp: [github]
---

# User initiates task
u: Add ad monetization to the billing tier. Users on Pro plan should see no ads.

# Agent begins investigation
a: I'll analyze the billing system and plan this feature.

# Check codebase structure
t:grep "billing" type=rs → [4 files]
t:grep "tier\|plan" path=src/billing → [12 matches]

# Read core billing module
t:read src/billing/mod.rs → [186 lines]
  pub enum BillingTier { Free, Pro, Enterprise }
  pub struct Subscription { tier: BillingTier, ... }

# Check recent changes
t:git log -5 path=src/billing →
  a]2d3f4 "Add enterprise tier" (3 days ago)
  b92c1 "Fix proration bug" (1 week ago)

# Check for past context
r: "billing" "monetization" → [no matches]
r: "ads" "advertising" → [1 match]
  sess_20250610: discussed ad placement strategy

# Query GitHub for related issues
c:github.issues q="ad monetization" state=open → [2 issues]
  #142 "Add ad-supported free tier"
  #156 "Pro users complaining about ads"

# Discover relevant skill
s:marketplace search="billing" → [3 skills]
  stripe-integration ★★★★☆
  revenue-analytics ★★★☆☆
  pricing-tiers ★★★★★

# Activate skill
s:pricing-tiers install → [ok]
s:pricing-tiers activate →
  Loaded: tier design patterns, migration strategies

# Spawn exploration subagent for deeper analysis
x:explore "How are ads currently shown? Where to add tier check?" →
  Found: src/ads/display.rs shows ads unconditionally
  Found: No tier check exists in ad pipeline
  Suggest: Add should_show_ads(user) → bool

# Synthesize findings
a: I've analyzed the codebase. Here's what I found:
  - Billing has 3 tiers: Free, Pro, Enterprise
  - Ads display unconditionally in src/ads/display.rs
  - No tier-based ad logic exists
  - Related issues: #142, #156

# Create plan
p:create id=p1 "Add ad monetization tier logic" →
  1. Add should_show_ads() to billing module
  2. Check tier in ads/display.rs
  3. Add visual indicator for Pro users
  4. Update tests
  5. Close issues #142, #156

# User reviews
u: Looks good. Also add a banner showing "Upgrade to remove ads" for free users.

# Update plan
p:update id=p1 →
  + 6. Add upgrade banner component for Free tier

# User approves
u: Approved. Go ahead.

# Switch to execution
m: auto

# Agent executes plan
a: Starting implementation.

t:write src/billing/ads.rs →
  pub fn should_show_ads(user: &User) -> bool {
      matches!(user.subscription.tier, BillingTier::Free)
  }

t:edit src/ads/display.rs old="fn render_ad" →
  Added tier check before rendering

t:write src/components/upgrade_banner.rs → [42 lines]

t:test latency_ms=3420 path=src/billing → [ok] 12 tests passed
t:test latency_ms=2180 path=src/ads → [ok] 8 tests passed

t:git add -A → [ok]
t:git commit "Add ad monetization tier logic" → [ok]
t:git push latency_ms=8430 → [ok]

c:github.pr create title="Add ad monetization" body="..." →
  PR #203 created

c:github.issues close 142 156 →
  Closed #142, #156

# Complete
a: Done. PR #203 ready for review. Closes #142 and #156.

p:complete id=p1 → [ok]
m: chat

# Session summary
# tokens_in=2847 tokens_out=1923 tools=14 skills=1 duration=34s cost=$0.08 redactions=0
```

---

## COMPACT FORMAT

For logging/storage, strip whitespace and use single line:

```
u:Add ad monetization to billing tier
a:I'll analyze the billing system.
t:grep "billing" type=rs→[4]
t:read src/billing/mod.rs→[186]
t:git log -5 path=src/billing→[5]
r:billing monetization→[0]
c:github.issues q="ad monetization"→[2]
s:pricing-tiers activate→[ok]
x:explore "ad tier check"→[found]
p:create "Ad monetization"→[6 steps]
m:auto
t:write src/billing/ads.rs→[ok]
t:edit src/ads/display.rs→[ok]
t:test→[ok]
t:git commit→[ok]
c:github.pr create→#203
a:Done. PR #203.
```

---

## JSONL ALTERNATIVE

For programmatic use, each line is JSON:

```jsonl
{"t":"u","c":"Add ad monetization to billing tier"}
{"t":"a","c":"I'll analyze the billing system."}
{"t":"t","n":"grep","a":{"pattern":"billing","type":"rs"},"r":{"files":4}}
{"t":"t","n":"read","a":{"path":"src/billing/mod.rs"},"r":{"lines":186}}
{"t":"s","n":"pricing-tiers","a":"activate","r":"ok"}
{"t":"x","n":"explore","q":"ad tier check","r":"found in src/ads/display.rs"}
{"t":"p","a":"create","title":"Ad monetization","r":{"steps":6}}
{"t":"m","v":"auto"}
{"t":"t","n":"write","a":{"path":"src/billing/ads.rs"},"r":"ok"}
{"t":"c","s":"github","m":"pr.create","r":{"pr":203}}
{"t":"a","c":"Done. PR #203."}
```

Keys: `t`=type, `c`=content, `n`=name, `a`=args/action, `r`=result, `s`=server, `m`=method, `v`=value, `q`=query

---

## FUTURE: SKILL DISCOVERY

When marketplace is live, skill discovery flow:

```
# User needs capability we don't have
u: Deploy this to Vercel with preview URLs

# Agent checks installed skills
s:list → [code-review, pricing-tiers]

# No deploy skill, search marketplace
s:marketplace search="vercel deploy" →
  vercel-deploy ★★★★★ (2.3k installs)
    "Deploy to Vercel with preview URLs, env config"
  netlify-deploy ★★★★☆ (890 installs)

# Recommend to user
a: I don't have a Vercel deploy skill. Found "vercel-deploy" (★★★★★, 2.3k installs). Install it?

u: Yes

# Install and activate
s:vercel-deploy install → [ok]
s:vercel-deploy activate →
  Loaded: deployment workflow, env handling, preview URLs

# Now agent can deploy
a: Deploying to Vercel...
t:vercel-deploy preview → https://my-app-abc123.vercel.app
```

---

## FUTURE: AGENT-CREATED SKILLS

When Autopilot creates skills for future self:

```
# User does same thing repeatedly
u: Format the changelog like last time

# Agent notices pattern
a: I've done this 3 times. Creating a skill for future use.

s:create name="changelog-format" →
  Writing SKILL.md...
  Saving format template to assets/...
  [ok] Skill created at ~/.openagents/skills/changelog-format/

# Next session
s:changelog-format activate → [loaded from local]
a: Using saved changelog format.
```

---

## METRICS LINE

End session with metrics comment:

```
# tokens=4770 tools=14 skills=1 mcp=3 subagents=1 plans=1 duration=34s cost=$0.12
```

---

## PARSING

Regex patterns for parsing:

```
# Structure
Header:     ^---$
Meta:       ^# (.+)$

# Messages
User:       ^u: (.+)$
Agent:      ^a: (.+)$

# Tools (with streaming markers and ATIF fields)
ToolStart:  ^t!:(\w+)(?: id=(\w+))?(?: tid=(\w+))?(?: span=(\w+))? (.+?)→(.+)$
ToolProg:   ^t~:(\w+)(?: id=(\w+))?(?: span=(\w+))? (.+)$
Tool:       ^t:(\w+)(?: id=(\w+))?(?: tid=(\w+))?(?: span=(\w+))?(?: step=(\d+))?(?: ts=([^\s]+))?(?: attempt=(\d+(?:/\d+)?))?(?: latency_ms=(\d+))?(?: level=(\w+))? (.+?)(?:→(.+))?$
Observation: ^o:(?: id=(\w+))?(?: partial=)?(.+)?(?:→(.+))?$

# Other primitives
Skill:      ^s:(\S+) (\w+)(?:→(.+))?$
Plan:       ^p:(\w+)(?: id=(\w+))?(?: "(.+)")?(?:→(.+))?$
Mode:       ^m: ?(\w+)$
Recall:     ^r: ?(.+?)→(.+)$
Subagent:   ^x:(\w+)(?: id=(\w+))?(?: tid=(\w+))?(?: span=(\w+))? "(.+)"(?:→(.+))?$
MCP:        ^c:(\w+)\.(\w+)(?: id=(\w+))? ?(.+)?→(.+)$
Question:   ^q:(?: id=(\w+))? "(.+)"(?: options=\[(.+)\])?(?: multi=(\w+))?(?:→(.+))?$

# Lifecycle
Lifecycle:  ^@(\w+)(?:-(\w+))?(?: id=(\w+))? ?(.*)$
Phase:      ^@phase (\w+)(?: (.+))?$
Compact:    ^@compact reason="([^"]+)"(?: tokens_before=(\d+))?(?: tokens_after=(\d+))?$
Time:       ^# t=(\d{2}):(\d{2}):(\d{2})
Budget:     ^# budget: \$([0-9.]+) remaining=\$([0-9.]+)

# Blobs and special
Blob:       @blob sha256=([a-f0-9]+)(?: bytes=(\d+))?(?: mime=(\S+))?
Redacted:   \[redacted:(\w+)\]
Metrics:    ^# metrics(?: step=(\d+))? prompt_tokens=(\d+) completion_tokens=(\d+)
StepTs:     step=(\d+) ts=(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)
Notes:      ^# notes: (.+)$
Extra:      extra\.(\w+)=([^\s]+)
```

---

## RATIONALE

**Why BlackBox?**

Like aircraft flight recorders, captures everything that happened in a session for replay, debugging, and analysis.

**Why this format?**

1. **Readable**: Humans can scan quickly
2. **Compact**: ~50% smaller than full JSON
3. **Parseable**: Simple regex patterns
4. **Extensible**: Add new prefixes as needed
5. **Diff-friendly**: Line-based for git
6. **Replayable**: Can reconstruct session state

**Token efficiency vs JSON:**

| Format | Example line | Tokens (approx) |
|--------|--------------|-----------------|
| Full JSON | `{"type":"tool","name":"grep","args":{"pattern":"billing"},"result":{"files":4}}` | ~25 |
| BlackBox | `t:grep "billing"→[4]` | ~8 |
| Compact | `t:grep "billing"→[4]` | ~6 |

~3-4x more efficient than verbose JSON.

---

## ATIF INTEROPERABILITY

BlackBox is designed to be **losslessly convertible** to [ATIF (Agent Trajectory Interchange Format)](../rfcs/0001-trajectory-format.md) for interchange, training, and benchmarking.

### Mapping Table

| BlackBox | ATIF Field |
|----------|------------|
| Header `id:` | `session_id` |
| Header `format:` | `schema_version` |
| Header `model:` | `agent.model_name` |
| `u:` | `StepObject(source="user", message=...)` |
| `a:` | `StepObject(source="agent", message=...)` |
| `t:` with `id=` | `tool_calls[{tool_call_id, function_name, arguments}]` |
| `o:` with `id=` | `observation.results[{source_call_id, content}]` |
| `x:` with refs | `observation.results[{subagent_trajectory_ref}]` |
| `# metrics step=N` | `StepObject.metrics` |
| `@start/@end` | `system` steps or `final_metrics` |
| `@blob` | `extra.blobs[...]` or artifact refs |
| `notes:` | `notes` |
| `extra.*` | `extra` |

### Conversion Example

**BlackBox:**
```
t:search id=call_1 ticker=GOOGL step=2 ts=2025-12-18T03:21:11Z → $185.35
```

**ATIF:**
```json
{
  "step_id": 2,
  "timestamp": "2025-12-18T03:21:11Z",
  "source": "agent",
  "tool_calls": [{
    "tool_call_id": "call_1",
    "function_name": "search",
    "arguments": {"ticker": "GOOGL"}
  }],
  "observation": {
    "results": [{
      "source_call_id": "call_1",
      "content": "$185.35"
    }]
  }
}
```

### When to Use Which

| Use Case | Format |
|----------|--------|
| Debugging, human review | BlackBox (`.bbox`) |
| Training data, SFT/RL | ATIF (`.json`) |
| Benchmarking, interchange | ATIF (`.json`) |
| Version control, diffs | BlackBox (`.bbox`) |
| Long-running sessions | BlackBox (`.bbox`) |

**Design principle**: Debug from `.bbox` (fast, readable), export to ATIF for interchange/training (standard, portable).
