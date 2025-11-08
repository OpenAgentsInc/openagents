# Overnight Agents - Demo Script

**Total Duration**: ~11 minutes
**Recording Date**: TBD
**Presenter**: Christopher David
**Environment**: macOS 26.0+, iOS 16.0+

---

## Pre-Recording Checklist

### Environment Setup
- [ ] macOS laptop fully charged (or plugged in)
- [ ] iOS device charged and paired via bridge
- [ ] Clean repo state (`git status` shows no uncommitted changes)
- [ ] `gh` CLI authenticated (`gh auth status`)
- [ ] Foundation Models available (`SystemLanguageModel.default.availability == .available`)
- [ ] Bridge connected (iOS app shows green "Connected" indicator)
- [ ] Screen recording software ready (QuickTime or ScreenFlow)
- [ ] Good lighting and audio setup
- [ ] Close unnecessary apps (reduce distractions in screen recording)

### Test Manifest Ready
- [ ] `examples/nightly-refactor.json` validated and tested
- [ ] Upgrade executor can load and parse manifest
- [ ] Dry run completed successfully

### Backup Plan
- [ ] Have pre-recorded segments ready in case of live failures
- [ ] Know how to restart scheduler if it hangs
- [ ] Have example PR ready to show if creation fails live

---

## Part 1: Introduction & Setup (2 minutes)

### Opening Shot (0:00 - 0:30)
**Camera**: Screen recording starts with desktop visible

**Narration**:
> "What if your coding agents could work for you while you sleep? Today I'm going to show you overnight agent orchestration powered by Apple's on-device Foundation Models."

**Action**:
- Open terminal
- `cd ~/code/openagents`
- `git status` (show clean repo)

### Load Upgrade Manifest (0:30 - 1:00)
**Action**:
```bash
cat private/overnight/examples/nightly-refactor.json
```

**Narration** (while scrolling through JSON):
> "This is an upgrade manifest—a deterministic JSON file that describes what work to do, when to do it, and how to do it. This one runs every 30 minutes between 1am and 5am, but we'll compress the timeline for this demo."

**Highlight**:
- `schedule.expression`: `"*/30 1-5 * * *"`
- `constraints.plugged_in`: `true`
- `pipeline`: Array of operations

### Start Scheduler (1:00 - 2:00)
**Action**:
```bash
openagents scheduler start \
  --upgrade private/overnight/examples/nightly-refactor.json \
  --interval 60  # 1-min intervals for demo
```

**Terminal Output** (shown):
```
[2025-11-08 14:30:15] SchedulerService started
[2025-11-08 14:30:15] Loaded upgrade: nightly-refactor-v1
[2025-11-08 14:30:15] Next wake: 2025-11-08 14:31:15 (in 60s)
[2025-11-08 14:30:15] Checking constraints...
[2025-11-08 14:30:15]   ✓ Plugged in: true
[2025-11-08 14:30:15]   ✓ WiFi: true
[2025-11-08 14:30:15]   ✓ CPU usage: 23% (< 80%)
[2025-11-08 14:30:15] All constraints satisfied
[2025-11-08 14:30:15] Waiting for next wake time...
```

**Narration**:
> "The scheduler is now running. It checks constraints—power, network, CPU—and waits for the next scheduled time. On iOS, we can see this in real-time."

**Switch to iPhone**:
- Show iOS app
- Navigate to "Overnight Monitoring" tab
- Show status: "Scheduler Running, Next wake: 60s"

---

## Part 2: First Orchestration Cycle (3 minutes)

### Decision Phase (2:00 - 3:00)
**Action**: Wait for first wake-up at 14:31:15

**Terminal Output**:
```
[2025-11-08 14:31:15] Wake-up triggered
[2025-11-08 14:31:15] Executing pipeline...
[2025-11-08 14:31:15] [1/5] session.analyze
[2025-11-08 14:31:16]   Analyzing 20 recent sessions (Claude Code, Codex)
[2025-11-08 14:31:18]   Most touched: BridgeManager.swift (25x)
[2025-11-08 14:31:18]   User intents: refactor, error handling, concurrency
[2025-11-08 14:31:18] [2/5] repo.status
[2025-11-08 14:31:19]   Branch: main
[2025-11-08 14:31:19]   Modified files: 0
[2025-11-08 14:31:19]   Test coverage: 68%
[2025-11-08 14:31:19] [3/5] orchestrate.decide
[2025-11-08 14:31:19]   Calling Foundation Models...
[2025-11-08 14:31:20]   Streaming response...
```

**iOS App** (show in parallel):
- Task queue updates: "Pending: 1"
- DecisionCard appears with:
  - Task: "Refactor BridgeManager error handling with Swift Result types"
  - Agent: Claude Code
  - Rationale: "BridgeManager.swift touched 25 times with user frequently requesting error handling improvements"
  - Confidence: 0.87

**Narration**:
> "The Foundation Models orchestrator is analyzing session history and repository state. It sees BridgeManager has been touched 25 times recently, and the user cares about error handling. So it decides that's the highest-impact work right now."

### Agent Execution (3:00 - 4:30)
**Terminal Output**:
```
[2025-11-08 14:31:22] [4/5] agent.execute
[2025-11-08 14:31:22]   Agent: claude-code
[2025-11-08 14:31:22]   Task: Refactor BridgeManager error handling with Swift Result types
[2025-11-08 14:31:22]   Session: abc123-def456-789
[2025-11-08 14:31:23]   Starting Claude Code...
[2025-11-08 14:31:25]   [tool_call] read_file: BridgeManager.swift
[2025-11-08 14:31:27]   [tool_call] edit_file: BridgeManager.swift
[2025-11-08 14:31:30]   [tool_call] edit_file: BridgeManager.swift
[2025-11-08 14:31:33]   [tool_call] run_bash: xcodebuild test ...
[2025-11-08 14:31:55]   [tool_result] Tests passed ✓
[2025-11-08 14:31:56]   Session completed successfully
[2025-11-08 14:31:56]   Duration: 34s
[2025-11-08 14:31:56]   Tool calls: 12
```

**iOS App** (show in parallel):
- SessionCard updates in real-time:
  - "Claude Code - Running"
  - "12 tool calls"
  - "Elapsed: 34s"
  - Progress bar animating

**Narration**:
> "Claude Code is now running. You can see the tool calls streaming in real-time—reading files, making edits, running tests. All of this is happening autonomously while the iOS app monitors progress."

### PR Creation (4:30 - 5:00)
**Terminal Output**:
```
[2025-11-08 14:31:56] [5/5] pr.create
[2025-11-08 14:31:56]   Creating branch: agent/nightly-refactor/abc123
[2025-11-08 14:31:57]   Generating commit from 12 tool calls...
[2025-11-08 14:31:58]   Pushing to origin...
[2025-11-08 14:32:00]   Creating PR via gh CLI...
[2025-11-08 14:32:03]   ✓ PR #42 created
[2025-11-08 14:32:03]   URL: https://github.com/OpenAgentsInc/openagents/pull/42
[2025-11-08 14:32:03] Pipeline completed successfully
[2025-11-08 14:32:03] Next wake: 2025-11-08 14:33:03
```

**iOS App**:
- PRPreviewCard appears:
  - Title: "Refactor BridgeManager error handling with Swift Result types"
  - Body preview (scrollable)
  - Buttons: [Approve] [Edit] [Cancel]

**Narration**:
> "And just like that, we have a pull request. The agent created a branch, generated commits from its work, and pushed to GitHub. On iOS, I can preview the PR and approve or reject it."

**Action**: Tap [Approve] in iOS app

**iOS App**: Shows "✓ PR Approved"

---

## Part 3: GitHub PR Review (2 minutes)

### Show PR on GitHub (5:00 - 7:00)
**Action**:
- Open browser
- Navigate to `https://github.com/OpenAgentsInc/openagents/pull/42`

**Show**:
- PR title and body (generated from template)
- Files changed tab:
  - `BridgeManager.swift`: ~80 lines changed
  - Refactored optional returns to `Result<T, Error>`
  - Proper error propagation
- Commits tab:
  - Single commit with clear message
  - "Agent work: abc123-def456-789"
- Checks tab:
  - All tests passing ✓
  - Build succeeded ✓

**Narration**:
> "This is real code, written by Claude Code, ready for review. The changes make sense—it replaced optional returns with Swift's Result type, giving us proper error context. Tests are passing. This is production-quality work."

---

## Part 4: Second Orchestration Cycle (2 minutes)

### Wait for Next Wake (7:00 - 7:30)
**Terminal Output**:
```
[2025-11-08 14:33:03] Wake-up triggered
[2025-11-08 14:33:03] Executing pipeline...
[2025-11-08 14:33:05] [3/5] orchestrate.decide
[2025-11-08 14:33:07]   Task: Generate comprehensive tests for DesktopWebSocketServer
[2025-11-08 14:33:07]   Agent: codex
[2025-11-08 14:33:07]   Rationale: WebSocketServer has 0% test coverage, user frequently mentions "need more tests"
```

**Narration**:
> "Second cycle. This time the orchestrator picked a different task: test generation. And it chose Codex instead of Claude Code, because Codex excels at writing tests."

### Show Different Agent (7:30 - 9:00)
**Terminal Output** (abbreviated):
```
[2025-11-08 14:33:08] [4/5] agent.execute
[2025-11-08 14:33:08]   Agent: codex
[2025-11-08 14:33:10]   [tool_call] write_file: DesktopWebSocketServerTests.swift
[2025-11-08 14:33:15]   [tool_call] write_file: DesktopWebSocketServerTests.swift (updated)
[2025-11-08 14:33:20]   [tool_call] run_bash: xcodebuild test ...
[2025-11-08 14:33:45]   [tool_result] 15 tests passed ✓
[2025-11-08 14:33:46]   Session completed successfully
[2025-11-08 14:33:47] [5/5] pr.create
[2025-11-08 14:33:50]   ✓ PR #43 created
```

**iOS App**:
- Shows two active tasks now:
  - PR #42 (approved, merged)
  - PR #43 (pending review)

**Narration**:
> "Same orchestration loop, different agent, different task. Codex generated 15 comprehensive tests in under a minute. Another PR ready for review."

---

## Part 5: Results & Future Vision (2 minutes)

### Morning Summary View (9:00 - 10:00)
**Action**: Show mock "morning view" (simulated)

**Screen**: iOS app showing overnight summary
```
Overnight Run: Nov 8, 2025
Duration: 4 hours (1:00 AM - 5:00 AM)

Orchestration Cycles: 8
Tasks Completed: 6
Tasks Skipped: 2 (time budget exceeded)

PRs Created: 6
  - 4 × Refactoring (Claude Code)
  - 2 × Test Generation (Codex)

Total Autonomous Work Time: 3.2 hours
Agent Selection: 67% Claude, 33% Codex

User Action Required: Review 6 PRs
```

**Narration**:
> "Imagine waking up to this every morning. Six pull requests with quality work—refactoring, tests, documentation—all done while you slept. Your agents worked for you overnight, and all you need to do is review and merge."

### Show Upgrade JSON (10:00 - 10:30)
**Action**: Show `nightly-refactor.json` again

**Narration**:
> "And the best part? All this orchestration logic is just JSON. It's deterministic, shareable, and verifiable. You can send this manifest to a colleague, and they'll get the exact same behavior."

### Future Vision (10:30 - 11:00)
**Action**: Show mockup slides or diagrams

**Slide 1**: Nostr Marketplace
- Upgrades published as Nostr events (kind 30051)
- Discover upgrades from relays
- Ratings and reviews (kind 30054)

**Slide 2**: Bitcoin Payments
- Pay creators in Bitcoin (via Lightning/Spark)
- Revenue splits automatically distributed
- Reputation scores earn you more

**Slide 3**: Federated Agent Mesh
- P2P task delegation across devices
- Contribute compute to shared pools
- Earn Bitcoin for background availability

**Narration**:
> "This is the foundation of something bigger. In the near future, you'll be able to publish these upgrade manifests to the Nostr marketplace. Other users can discover, install, and run your upgrades. If your upgrade is good, you earn reputation points and Bitcoin revenue. We're building a decentralized compute marketplace where agents work around the clock, and everyone benefits."

---

## Closing (11:00)

**Action**: Return to terminal showing scheduler running

**Narration**:
> "That's overnight agent orchestration. Autonomous work while you sleep, powered by on-device Apple Intelligence, encapsulated in portable JSON manifests, and future-ready for the Nostr compute marketplace. Thanks for watching."

**Screen**: Fade to OpenAgents logo + GitHub link

---

## Post-Recording Checklist

- [ ] Review recording for audio/video quality
- [ ] Trim any dead air or mistakes
- [ ] Add captions/subtitles
- [ ] Add intro/outro graphics
- [ ] Export in 1080p or 4K
- [ ] Upload to YouTube with:
  - Title: "Overnight Agent Orchestration with Apple Intelligence"
  - Description: Link to GitHub, upgrade manifests, ADR
  - Tags: "ai agents", "foundation models", "automation", "swift"
  - Thumbnail: Compelling still from demo

---

## Backup Segments (Pre-recorded)

In case of live failures, have these pre-recorded:

1. **FM Decision Phase** (30s): Show Foundation Models streaming response
2. **Agent Tool Calls** (30s): Show Claude Code making edits
3. **PR on GitHub** (30s): Show completed PR with changes
4. **iOS Monitoring** (30s): Show real-time updates in iOS app

Use these as B-roll or fallback if live demo encounters issues.

---

## Notes for Presenter

- **Speak slowly and clearly**: Explain technical concepts for non-experts
- **Show, don't tell**: Let the code and UIs speak for themselves
- **Pause for effect**: Give viewers time to read terminal output
- **Be authentic**: If something fails, acknowledge it and move on
- **Emphasize key points**:
  - On-device privacy (no cloud)
  - Deterministic JSON (shareable, verifiable)
  - Real work (not a toy demo)
  - Future marketplace vision

---

## Technical Setup Details

### Terminal Window
- Font: SF Mono, 16pt
- Colors: Dark theme with high contrast
- Size: Half screen (left side)

### iOS App Window
- Screen mirroring or QuickTime screen recording
- Size: Half screen (right side) or picture-in-picture

### Browser
- GitHub PR view
- Pre-authenticated
- Incognito mode (no distracting notifications)

### Timing Notes
- Part 1 can be sped up in post if needed
- Parts 2-4 should be real-time (shows authenticity)
- Part 5 can use pre-recorded segments for smooth pacing
