# Terminal Bench User Stories

> Comprehensive user stories for Terminal Bench (TB) and Effuse HUD integration testing.
> These stories define all user flows for full management of Terminal Bench via the HUD.

> Log 2025-12-06: Added tests for US-4.1 (socket run start/complete and ignore mismatched completion) in src/effuse/widgets/tb-controls.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-5.2 (ignore output from other runs) in src/effuse/widgets/tb-output.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-4.1 (run status from socket start/complete) in src/effuse/widgets/tb-controls.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-4.5 (error icon) in src/effuse/widgets/category-tree.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-4.5 (failed icon) and US-14.6 (timeout icon) in src/effuse/widgets/category-tree.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-14.6 (timeout tasks icon) in src/effuse/widgets/category-tree.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-5.3 (verification output rendering) in src/effuse/widgets/tb-output.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-1.3 (suite metadata) and US-14.3/US-14.4 (suite/run errors) in src/effuse/widgets/tb-controls.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-4.5 (task status icons running→passed) in src/effuse/widgets/category-tree.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-3.3 (start single random task) in src/effuse/widgets/tb-controls.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-3.2 (start run for selected tasks) in src/effuse/widgets/tb-controls.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-5.1 (auto-scroll toggle) in src/effuse/widgets/tb-output.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-10.3 (assign task in sandbox) in src/effuse/widgets/mc-tasks.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-7.5 (clear/delete trajectory history) in src/effuse/widgets/trajectory-pane.test.ts; bun test passing.
> Log 2025-12-06: Added test for US-7.3 (unified TB/ATIF trajectories with type badges) in src/effuse/widgets/trajectory-pane.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-10.5 (view task type badges) and US-10.8 (refresh ready tasks) in src/effuse/widgets/mc-tasks.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-10.4 (view task priority) and US-10.6 (view task labels) in src/effuse/widgets/mc-tasks.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-10.1 (load ready tasks), US-10.2 (assign task), and US-10.7 (collapse widget) in src/effuse/widgets/mc-tasks.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-4.8 (APM live metrics updates and snapshot comparison) in src/effuse/widgets/apm-widget.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-7.1 (load trajectories), US-7.2 (select trajectory), US-7.6 (refresh list), and US-7.7 (collapse pane) in src/effuse/widgets/trajectory-pane.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-1.2 (invalid suite path error), US-2.8 (toggle individual task), and US-2.9 (select all/clear) in src/effuse/widgets/tb-controls.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-13.4 (container logs per task) and US-13.5 (sandbox/host label + exit status) in src/effuse/widgets/container-panes.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-2.2 (toggle category expand/collapse) in src/effuse/widgets/category-tree.test.ts and US-5.6 (copy output) in src/effuse/widgets/tb-output.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-5.1 (live output stream), US-5.2 (task/run context), and US-5.5 (clear output) in src/effuse/widgets/tb-output.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-2.1 (load suite into category tree), US-2.3 (expand all), and US-2.4 (collapse all) in src/effuse/widgets/category-tree.test.ts; bun test passing.
> Log 2025-12-06: Added tests for US-1.1 (load suite), US-3.1 (start run), and US-3.4 (stop run) in src/effuse/widgets/tb-controls.test.ts; bun test passing.

---

## Table of Contents

1. [Suite Loading & Configuration](#1-suite-loading--configuration)
2. [Task Selection & Filtering](#2-task-selection--filtering)
3. [Run Execution](#3-run-execution)
4. [Real-Time Monitoring](#4-real-time-monitoring)
5. [Output & Verification](#5-output--verification)
6. [Results & Metrics](#6-results--metrics)
7. [History & Trajectories](#7-history--trajectories)
8. [Comparison & Baselines](#8-comparison--baselines)
9. [Settings & Configuration](#9-settings--configuration)
10. [MechaCoder Integration](#10-mechacoder-integration)
11. [Overnight Iteration Runs](#11-overnight-iteration-runs)
12. [Dashboard & Reporting](#12-dashboard--reporting)
13. [Sandbox Mode](#13-sandbox-mode)
14. [Error Handling & Recovery](#14-error-handling--recovery)
15. [Navigation & UI](#15-navigation--ui)
16. [Keyboard Shortcuts](#16-keyboard-shortcuts)
17. [Widget Interactions](#17-widget-interactions)
18. [Socket & Connection](#18-socket--connection)

---

## 1. Suite Loading & Configuration

### US-1.1: Load Default Suite
**As a** TB operator
**I want to** load the default Terminal Bench suite
**So that** I can run the standard benchmark tasks

**Acceptance Criteria:**
- Click "Load Suite" button in TB Controls widget
- Default path `suites/terminal-bench-v1.json` is pre-populated
- Suite metadata (name, version, task count) displays after loading
- Tasks populate in the Category Tree widget

### US-1.2: Load Custom Suite Path
**As a** TB operator
**I want to** specify a custom suite file path
**So that** I can run custom benchmark configurations

**Acceptance Criteria:**
- Text input field accepts absolute or relative paths
- Clicking "Load" fetches and parses the suite file
- Invalid paths show clear error messages
- Suite file validation catches malformed JSON

### US-1.3: View Suite Metadata
**As a** TB operator
**I want to** see suite metadata after loading
**So that** I understand what benchmark I'm about to run

**Acceptance Criteria:**
- Suite name displays prominently
- Suite version shows (e.g., "v1.0.0")
- Total task count displays
- Category breakdown shows task distribution

### US-1.4: Reload Suite
**As a** TB operator
**I want to** reload the current suite
**So that** I can pick up changes to the suite file

**Acceptance Criteria:**
- "Refresh" button reloads from current path
- State resets (selected tasks cleared)
- Loading indicator shows during reload
- Success/failure feedback provided

### US-1.5: View Suite File Location
**As a** TB operator
**I want to** see the full path of the loaded suite
**So that** I can verify I'm using the correct file

**Acceptance Criteria:**
- Full path displays in suite info section
- Path is copyable to clipboard
- Relative paths resolve to absolute on display

---

## 2. Task Selection & Filtering

### US-2.1: View All Tasks in Category Tree
**As a** TB operator
**I want to** see all tasks organized by category
**So that** I can understand the benchmark scope

**Acceptance Criteria:**
- Tasks grouped under category headers
- Each category shows task count
- Uncategorized tasks appear in fallback group
- Scroll for long task lists

### US-2.2: Expand/Collapse Category
**As a** TB operator
**I want to** expand and collapse categories
**So that** I can focus on specific areas

**Acceptance Criteria:**
- Click category header toggles expand/collapse
- Collapse icon changes (▶/▼)
- Collapsed state persists during session
- Category stats visible when collapsed

### US-2.3: Expand All Categories
**As a** TB operator
**I want to** expand all categories at once
**So that** I can see the full task list

**Acceptance Criteria:**
- "Expand" button expands all categories
- All tasks become visible
- Button state reflects current expansion

### US-2.4: Collapse All Categories
**As a** TB operator
**I want to** collapse all categories
**So that** I can get a summary view

**Acceptance Criteria:**
- "Collapse" button collapses all categories
- Only category headers visible
- Per-category stats still visible

### US-2.5: Filter Tasks by Difficulty
**As a** TB operator
**I want to** filter tasks by difficulty level
**So that** I can run benchmarks of specific complexity

**Acceptance Criteria:**
- Difficulty filter dropdown (Easy/Medium/Hard)
- Filtered tasks update in real-time
- Task count updates to reflect filter
- Clear filter option available

### US-2.6: Filter Tasks by Category
**As a** TB operator
**I want to** filter to specific categories
**So that** I can benchmark specific skill areas

**Acceptance Criteria:**
- Category multiselect filter
- Categories: software-engineering, system-administration, security, etc.
- Combine with difficulty filter
- Show "X of Y tasks" after filtering

### US-2.7: Search Tasks by Name
**As a** TB operator
**I want to** search tasks by name or ID
**So that** I can find specific tasks quickly

**Acceptance Criteria:**
- Search input with debounced filtering
- Matches against task name and ID
- Case-insensitive matching
- Highlights matching text

### US-2.8: Select Individual Tasks
**As a** TB operator
**I want to** select specific tasks for a run
**So that** I can run a subset of the benchmark

**Acceptance Criteria:**
- Checkbox next to each task
- Click task row toggles selection
- Selection count displays in header
- "Clear Selection" button available

### US-2.9: Select All Tasks in Category
**As a** TB operator
**I want to** select all tasks in a category
**So that** I can run category-specific benchmarks

**Acceptance Criteria:**
- Category header checkbox selects all children
- Partial selection shows indeterminate state
- Deselecting category deselects all children

### US-2.10: View Task Details
**As a** TB operator
**I want to** see task details before running
**So that** I understand what will be tested

**Acceptance Criteria:**
- Click task shows detail popover/panel
- Details include: name, ID, difficulty, category
- Description/instructions if available
- Close detail view returns to tree

### US-2.11: Select Tasks by Difficulty Badge
**As a** TB operator
**I want to** quickly see difficulty via badges
**So that** I can scan task complexity at a glance

**Acceptance Criteria:**
- Difficulty badge (E/M/H) next to each task
- Color coding: green=easy, amber=medium, red=hard
- Badge clickable to filter to that difficulty

---

## 3. Run Execution

### US-3.1: Start Full Suite Run
**As a** TB operator
**I want to** start a run of all tasks
**So that** I can benchmark the full suite

**Acceptance Criteria:**
- "Start Run" button initiates run
- Run ID generated (format: tb-YYYYMMDDHHMMSS-xxxxx)
- Button disables during active run
- Run status shows "running"

### US-3.2: Start Selected Tasks Run
**As a** TB operator
**I want to** run only selected tasks
**So that** I can focus on specific areas

**Acceptance Criteria:**
- "Run Selected" button appears when tasks selected
- Only selected task IDs passed to runner
- Progress shows N of M selected tasks
- Can mix tasks from different categories

### US-3.3: Run Single Task
**As a** TB operator
**I want to** run a single task
**So that** I can test or debug specific cases

**Acceptance Criteria:**
- "Run" button on task row or detail view
- Single task run starts immediately
- Output focused on that task
- Results show for single task

### US-3.4: Stop Active Run
**As a** TB operator
**I want to** stop a running benchmark
**So that** I can abort if something is wrong

**Acceptance Criteria:**
- "Stop Run" button visible during run
- Clicking sends stop signal to subprocess
- Current task may complete or abort
- Results saved for completed tasks

### US-3.5: Set Run Timeout
**As a** TB operator
**I want to** configure per-task timeout
**So that** I can control how long tasks can run

**Acceptance Criteria:**
- Timeout input in seconds (default: 300)
- Applied to each task individually
- Tasks exceeding timeout marked "timeout"
- Timeout configurable before run start

### US-3.6: Set Max Turns
**As a** TB operator
**I want to** limit agent turns per task
**So that** I can control token usage

**Acceptance Criteria:**
- Max turns input (default: no limit)
- Agent stops after N turns if not complete
- "max_turns" outcome for exceeded limit
- Configurable before run start

### US-3.7: Configure Output Directory
**As a** TB operator
**I want to** specify where results are saved
**So that** I can organize benchmark outputs

**Acceptance Criteria:**
- Output directory input field
- Default: `results/tb-{runId}`
- Creates directory if doesn't exist
- Validates path is writable

### US-3.8: Resume Failed Run
**As a** TB operator
**I want to** resume a run that failed partway
**So that** I don't have to re-run completed tasks

**Acceptance Criteria:**
- "Resume" button on incomplete runs
- Skips already-completed tasks
- Continues from first incomplete
- Merges results with original run

### US-3.9: Re-run Failed Tasks Only
**As a** TB operator
**I want to** re-run only the failed tasks from a previous run
**So that** I can retry without full suite

**Acceptance Criteria:**
- "Retry Failed" button on completed runs
- Selects tasks with failed/timeout/error outcomes
- Creates new run with only those tasks
- Links to original run for comparison

### US-3.10: Run in Local Mode
**As a** TB operator
**I want to** run tasks without sandbox
**So that** I can test with direct system access

**Acceptance Criteria:**
- "Local Mode" toggle/checkbox
- Uses `tbench-local.ts` script
- Agent runs directly on host
- Faster but less isolated

### US-3.11: Run in Sandbox Mode
**As a** TB operator
**I want to** run tasks in sandbox containers
**So that** I get isolated, reproducible execution

**Acceptance Criteria:**
- "Sandbox Mode" toggle/checkbox
- Uses `tbench-sandbox.ts` script
- Each task runs in clean container
- Container backend configurable

---

## 4. Real-Time Monitoring

### US-4.1: View Current Task Progress
**As a** TB operator
**I want to** see which task is currently running
**So that** I can monitor progress

**Acceptance Criteria:**
- Current task highlighted in Category Tree
- Task row shows "running" indicator (▶)
- Pulsing/animated state for active task
- Task name shown in status bar

### US-4.2: View Run Progress Bar
**As a** TB operator
**I want to** see overall run progress
**So that** I know how much is complete

**Acceptance Criteria:**
- Progress bar showing X/Y tasks
- Percentage complete displayed
- Different colors for pass/fail/pending
- ETA based on average task time

### US-4.3: View Live Pass/Fail Counts
**As a** TB operator
**I want to** see pass/fail counts updating live
**So that** I can track success rate in real-time

**Acceptance Criteria:**
- Green count for passed tasks
- Red count for failed/timeout/error
- Updates immediately on task completion
- Pass rate percentage calculated

### US-4.4: View Category-Level Progress
**As a** TB operator
**I want to** see progress per category
**So that** I can identify problem areas

**Acceptance Criteria:**
- Category headers show pass/fail counts
- Color indicates category health
- Stats update as tasks complete
- Expandable to see task-level detail

### US-4.5: View Task Status Icons
**As a** TB operator
**I want to** see status icons for each task
**So that** I can quickly scan results

**Acceptance Criteria:**
- Icons: ○ pending, ▶ running, ✓ passed, ✗ failed, ⏱ timeout, ⚠ error
- Color-coded for quick scanning
- Icons update immediately on completion
- Consistent across all views

### US-4.6: View Run Duration Timer
**As a** TB operator
**I want to** see elapsed time for the run
**So that** I can track total runtime

**Acceptance Criteria:**
- Timer shows HH:MM:SS format
- Updates every second during run
- Stops when run completes
- Total duration saved to results

### US-4.7: View Token Usage Live
**As a** TB operator
**I want to** see token usage updating live
**So that** I can monitor costs

**Acceptance Criteria:**
- Input/output tokens shown separately
- Running total across all tasks
- Updates after each turn
- Per-task breakdown available

### US-4.8: View APM Widget Metrics
**As a** TB operator
**I want to** see real-time performance metrics
**So that** I can monitor system health

**Acceptance Criteria:**
- Actions Per Minute calculation
- Task completion rate
- Turn rate tracking
- Visual indicators for anomalies

---

## 5. Output & Verification

### US-5.1: View Live Agent Output
**As a** TB operator
**I want to** see agent output streaming live
**So that** I can watch the agent work

**Acceptance Criteria:**
- TB Output widget shows streaming text
- ANSI color codes rendered
- Auto-scrolls to bottom
- Scroll pause on user scroll-up

### US-5.2: View Task-Specific Output
**As a** TB operator
**I want to** see output for a specific task
**So that** I can review what happened

**Acceptance Criteria:**
- Click task shows its output
- Output includes all turns
- Tool calls visible
- Clear task header/separator

### US-5.3: View Verification Output
**As a** TB operator
**I want to** see the verification command output
**So that** I understand why a task passed/failed

**Acceptance Criteria:**
- Verification output shown after task
- Exit code displayed
- stdout/stderr separated
- Pass/fail determination explained

### US-5.4: Search Output
**As a** TB operator
**I want to** search within output text
**So that** I can find specific content

**Acceptance Criteria:**
- Search input in output widget
- Highlights matching text
- Navigate between matches
- Case-insensitive option

### US-5.5: Clear Output
**As a** TB operator
**I want to** clear the output display
**So that** I can start fresh

**Acceptance Criteria:**
- "Clear" button clears output
- Confirmation if run in progress
- Does not delete persisted logs
- Shortcut available (Cmd+K)

### US-5.6: Copy Output
**As a** TB operator
**I want to** copy output to clipboard
**So that** I can share or analyze externally

**Acceptance Criteria:**
- "Copy" button copies visible output
- "Copy All" copies full buffer
- Strips ANSI codes option
- Success notification shown

### US-5.7: Toggle Output Sources
**As a** TB operator
**I want to** filter output by source
**So that** I can focus on specific streams

**Acceptance Criteria:**
- Toggle: agent, verification, system
- Show/hide each source type
- Color-coding by source
- Source label on each line

### US-5.8: View Output Line Numbers
**As a** TB operator
**I want to** see line numbers in output
**So that** I can reference specific locations

**Acceptance Criteria:**
- Line numbers in gutter
- Click line number to select
- Copy includes line numbers option
- Toggle line numbers on/off

### US-5.9: Expand/Collapse Output Sections
**As a** TB operator
**I want to** collapse long output sections
**So that** I can focus on relevant parts

**Acceptance Criteria:**
- Collapsible sections for each task
- Collapse verification blocks
- Preserve expansion state
- Expand All / Collapse All

### US-5.10: View Container Output
**As a** TB operator
**I want to** see container stdout/stderr
**So that** I can debug sandbox issues

**Acceptance Criteria:**
- Container Panes widget shows container output
- Tabs for different containers
- Color-coded stdout/stderr
- Container lifecycle events

---

## 6. Results & Metrics

### US-6.1: View Run Summary
**As a** TB operator
**I want to** see a summary after run completes
**So that** I can understand overall performance

**Acceptance Criteria:**
- Pass rate percentage
- Total passed/failed/timeout/error counts
- Total duration
- Total tokens used

### US-6.2: View Per-Task Results
**As a** TB operator
**I want to** see results for each task
**So that** I can analyze individual performance

**Acceptance Criteria:**
- Table with task ID, name, outcome, duration, turns, tokens
- Sortable columns
- Filterable by outcome
- Click row for details

### US-6.3: View Category Statistics
**As a** TB operator
**I want to** see stats grouped by category
**So that** I can identify weak areas

**Acceptance Criteria:**
- Per-category pass rates
- Category comparison chart
- Sort by pass rate
- Drill down to tasks

### US-6.4: View Difficulty Statistics
**As a** TB operator
**I want to** see stats grouped by difficulty
**So that** I can assess capability levels

**Acceptance Criteria:**
- Pass rate by easy/medium/hard
- Expected vs actual comparison
- Difficulty distribution chart
- Trend analysis if multiple runs

### US-6.5: View Token Statistics
**As a** TB operator
**I want to** see detailed token usage
**So that** I can optimize costs

**Acceptance Criteria:**
- Total input/output tokens
- Average tokens per task
- Token distribution chart
- High-token task highlighting

### US-6.6: View Duration Statistics
**As a** TB operator
**I want to** see timing analysis
**So that** I can identify slow tasks

**Acceptance Criteria:**
- Average task duration
- Duration distribution
- Slowest tasks highlighted
- Duration vs difficulty correlation

### US-6.7: Export Results to JSON
**As a** TB operator
**I want to** export run results as JSON
**So that** I can process programmatically

**Acceptance Criteria:**
- "Export JSON" button
- Full run data included
- Pretty-printed option
- Download or copy to clipboard

### US-6.8: View Saved Run File
**As a** TB operator
**I want to** see where results are saved
**So that** I can access the raw data

**Acceptance Criteria:**
- File path displayed after run
- Path is copyable
- "Open in Finder" option (macOS)
- File follows TBRunFile schema

---

## 7. History & Trajectories

### US-7.1: View Recent Runs List
**As a** TB operator
**I want to** see my recent benchmark runs
**So that** I can review past results

**Acceptance Criteria:**
- Trajectory Pane shows recent runs
- Most recent first
- Shows: run ID, timestamp, pass rate
- Load 20+ runs by default

### US-7.2: Load Run Details
**As a** TB operator
**I want to** load full details of a past run
**So that** I can review what happened

**Acceptance Criteria:**
- Click run in list loads details
- Full task results load
- Output available if saved
- Run metadata displayed

### US-7.3: View Unified Trajectories
**As a** TB operator
**I want to** see TB runs and ATIF traces together
**So that** I can review all agent activity

**Acceptance Criteria:**
- Unified list of TB runs and ATIF trajectories
- Type indicator (TB/ATIF)
- Sorted by timestamp
- Filter by type

### US-7.4: View ATIF Trajectory Details
**As a** TB operator
**I want to** view ATIF trajectory details
**So that** I can analyze agent behavior

**Acceptance Criteria:**
- Select ATIF trajectory shows steps
- Step-by-step replay available
- Tool calls visible
- Agent state at each step

### US-7.5: Delete Run History
**As a** TB operator
**I want to** delete old runs
**So that** I can manage disk space

**Acceptance Criteria:**
- Delete button on run items
- Confirmation dialog
- Removes from filesystem
- Updates list after delete

### US-7.6: Refresh Trajectory List
**As a** TB operator
**I want to** refresh the trajectory list
**So that** I can see newly completed runs

**Acceptance Criteria:**
- "Refresh" button reloads list
- Auto-refresh option (5s interval)
- Loading indicator during refresh
- New items appear at top

### US-7.7: Collapse Trajectory Pane
**As a** TB operator
**I want to** collapse the trajectory pane
**So that** I can maximize working space

**Acceptance Criteria:**
- Toggle button collapses pane
- Minimized to narrow strip
- Click to expand again
- State persists in session

### US-7.8: Filter Trajectories by Date
**As a** TB operator
**I want to** filter trajectories by date range
**So that** I can find specific time periods

**Acceptance Criteria:**
- Date range picker
- Today/Week/Month presets
- Custom date range
- Filter indicator visible

---

## 8. Comparison & Baselines

### US-8.1: Compare Two Runs
**As a** TB operator
**I want to** compare two benchmark runs
**So that** I can see improvements/regressions

**Acceptance Criteria:**
- Select two runs for comparison
- Side-by-side metrics
- Task-level diff (pass→fail, fail→pass)
- Delta highlighting

### US-8.2: Set Baseline Run
**As a** TB operator
**I want to** mark a run as baseline
**So that** future runs compare against it

**Acceptance Criteria:**
- "Set as Baseline" option on run
- Baseline indicator in list
- New runs auto-compare to baseline
- Only one baseline at a time

### US-8.3: View Metric Deltas
**As a** TB operator
**I want to** see metric changes vs baseline
**So that** I can track progress

**Acceptance Criteria:**
- Pass rate delta (+/-%)
- Token delta (+/-N)
- Duration delta (+/-s)
- Color: green=improvement, red=regression

### US-8.4: View Task Outcome Changes
**As a** TB operator
**I want to** see which tasks changed outcomes
**So that** I can focus investigation

**Acceptance Criteria:**
- List of tasks with changed outcomes
- Previous vs current outcome
- Filter to regressions only
- Filter to improvements only

### US-8.5: Export Comparison Report
**As a** TB operator
**I want to** export a comparison report
**So that** I can share with team

**Acceptance Criteria:**
- Generate Markdown report
- Includes all deltas
- Task-level comparison table
- Copy or download option

---

## 9. Settings & Configuration

### US-9.1: Select Model Provider
**As a** TB operator
**I want to** choose which model to use
**So that** I can benchmark different models

**Acceptance Criteria:**
- Model dropdown: Claude Code, Ollama, etc.
- Claude Code is primary for TB
- Model persists across sessions
- Clear indication of active model

### US-9.2: Configure Claude Code Settings
**As a** TB operator
**I want to** configure Claude Code options
**So that** I can customize agent behavior

**Acceptance Criteria:**
- Max tokens setting
- Temperature (if applicable)
- System prompt override
- Tool restrictions

### US-9.3: Configure Sandbox Settings
**As a** TB operator
**I want to** configure sandbox options
**So that** I can customize isolation

**Acceptance Criteria:**
- Sandbox backend: docker, macos-container
- Sandbox image selection
- Resource limits (CPU, memory)
- Network access toggle

### US-9.4: View/Edit Default Timeouts
**As a** TB operator
**I want to** set default timeout values
**So that** I don't have to set them each run

**Acceptance Criteria:**
- Default timeout in settings
- Per-difficulty overrides
- Save settings persistently
- Reset to defaults option

### US-9.5: Configure Auto-Refresh
**As a** TB operator
**I want to** toggle auto-refresh for lists
**So that** I can see updates automatically

**Acceptance Criteria:**
- Auto-refresh toggle
- Refresh interval setting (seconds)
- Per-widget configuration
- Disable during active run

### US-9.6: Configure Theme/Appearance
**As a** TB operator
**I want to** customize the UI appearance
**So that** I can match my preferences

**Acceptance Criteria:**
- Dark theme (default)
- Light theme option
- Font size adjustment
- Color accent options

### US-9.7: Export/Import Settings
**As a** TB operator
**I want to** export and import settings
**So that** I can share configurations

**Acceptance Criteria:**
- Export settings to JSON
- Import settings from JSON
- Validate imported settings
- Merge vs replace option

---

## 10. MechaCoder Integration

### US-10.1: View Ready Tasks
**As a** TB operator
**I want to** see tasks ready for MechaCoder
**So that** I can assign work to the agent

**Acceptance Criteria:**
- MC Tasks widget shows ready tasks
- Tasks from `.openagents/tasks.jsonl`
- Priority-sorted display
- Refresh button to reload

### US-10.2: Assign Task to MechaCoder
**As a** TB operator
**I want to** assign a task to MechaCoder
**So that** the agent starts working on it

**Acceptance Criteria:**
- "Assign" button on each task row
- Spawns MechaCoder process
- Task removed from ready list
- Feedback on assignment success

### US-10.3: Assign Task with Sandbox
**As a** TB operator
**I want to** assign a task in sandbox mode
**So that** MechaCoder runs in isolation

**Acceptance Criteria:**
- Sandbox option on assignment
- Uses do-one-task.ts with --sandbox
- Container spawned for task
- Results persisted correctly

### US-10.4: View Task Priority
**As a** TB operator
**I want to** see task priorities (P0-P4)
**So that** I can prioritize critical work

**Acceptance Criteria:**
- Priority badge (P0/P1/P2/P3/P4)
- Color-coded by urgency
- Sorted by priority
- Filter by priority level

### US-10.5: View Task Type
**As a** TB operator
**I want to** see task types (bug/feature/task)
**So that** I understand the nature of work

**Acceptance Criteria:**
- Type indicator on each task
- Color: red=bug, green=feature, blue=task
- Filter by type
- Icon or badge display

### US-10.6: View Task Labels
**As a** TB operator
**I want to** see task labels
**So that** I can filter by topic

**Acceptance Criteria:**
- Labels displayed on task rows
- First 2-3 labels shown
- Overflow indicator
- Click label to filter

### US-10.7: Collapse MC Tasks Widget
**As a** TB operator
**I want to** collapse the MC Tasks widget
**So that** I can save screen space

**Acceptance Criteria:**
- Click header to collapse
- Shows task count when collapsed
- Expand to see full list
- State persists in session

### US-10.8: Refresh Ready Tasks
**As a** TB operator
**I want to** refresh the ready tasks list
**So that** I see newly available tasks

**Acceptance Criteria:**
- "Refresh" button reloads from JSONL
- Loading indicator during refresh
- Updates count in header
- Error handling for file issues

---

## 11. Overnight Iteration Runs

### US-11.1: Configure Overnight Run
**As a** TB operator
**I want to** configure an overnight iteration run
**So that** I can run many iterations unattended

**Acceptance Criteria:**
- Number of iterations input
- Output directory configuration
- Model selection for iterations
- Learning mode toggle

### US-11.2: Start Overnight Run
**As a** TB operator
**I want to** start an overnight run
**So that** the system benchmarks while I'm away

**Acceptance Criteria:**
- "Start Overnight" button
- Spawns tbench-iterate.ts process
- Runs N iterations sequentially
- Saves results after each

### US-11.3: View Iteration Progress
**As a** TB operator
**I want to** see progress across iterations
**So that** I can check status remotely

**Acceptance Criteria:**
- Current iteration N of M
- Cumulative pass rate trend
- Estimated time remaining
- Last iteration summary

### US-11.4: View Learning Patterns
**As a** TB operator
**I want to** see learning patterns from iterations
**So that** I can understand improvement

**Acceptance Criteria:**
- Pass rate trend chart
- Tasks that improved
- Tasks that regressed
- Stable vs volatile tasks

### US-11.5: Stop Overnight Run
**As a** TB operator
**I want to** stop an overnight run early
**So that** I can abort if needed

**Acceptance Criteria:**
- "Stop" button stops after current iteration
- Current iteration completes normally
- Results saved for completed iterations
- Partial run marked appropriately

### US-11.6: View Overnight Run History
**As a** TB operator
**I want to** see all overnight run results
**So that** I can compare iterations

**Acceptance Criteria:**
- List of iteration runs
- Each shows pass rate, duration
- Click to see individual iteration
- Compare iterations side-by-side

---

## 12. Dashboard & Reporting

### US-12.1: View HTML Dashboard
**As a** TB operator
**I want to** see results in an HTML dashboard
**So that** I can share rich reports

**Acceptance Criteria:**
- Dashboard link after run completes
- Opens in browser
- Charts and visualizations
- Shareable static HTML

### US-12.2: Generate Comparison Report
**As a** TB operator
**I want to** generate a comparison report
**So that** I can track progress over time

**Acceptance Criteria:**
- Select runs to compare
- Generate Markdown/HTML report
- Includes charts and tables
- Download or copy option

### US-12.3: View Task Trend Charts
**As a** TB operator
**I want to** see how tasks trend over time
**So that** I can identify patterns

**Acceptance Criteria:**
- Line chart of pass rate over runs
- Per-task trend lines
- Hover for details
- Date range filter

### US-12.4: View Token Cost Estimates
**As a** TB operator
**I want to** see estimated costs
**So that** I can budget appropriately

**Acceptance Criteria:**
- Token count × price estimate
- Per-run cost breakdown
- Cumulative cost tracking
- Model-specific pricing

---

## 13. Sandbox Mode

### US-13.1: Enable Sandbox Mode
**As a** TB operator
**I want to** enable sandbox mode for runs
**So that** tasks run in isolation

**Acceptance Criteria:**
- Sandbox toggle in TB Controls
- Uses tbench-sandbox.ts when enabled
- Containers spun up per task
- Cleaner isolation than local mode

### US-13.2: Select Sandbox Backend
**As a** TB operator
**I want to** choose the sandbox backend
**So that** I can use my preferred container technology

**Acceptance Criteria:**
- Backend dropdown: docker, macos-container
- Docker requires Docker Desktop
- macos-container uses native containers
- Backend validation on selection

### US-13.3: Configure Sandbox Image
**As a** TB operator
**I want to** specify the container image
**So that** I can customize the environment

**Acceptance Criteria:**
- Image input field
- Default image pre-populated
- Validates image exists locally
- Pull option if missing

### US-13.4: View Container Status
**As a** TB operator
**I want to** see container lifecycle status
**So that** I can debug sandbox issues

**Acceptance Criteria:**
- Container start/stop events
- Container ID displayed
- Exit codes shown
- Resource usage if available

### US-13.5: View Container Logs
**As a** TB operator
**I want to** see container stdout/stderr
**So that** I can debug execution issues

**Acceptance Criteria:**
- Container Panes widget shows logs
- Separate stdout/stderr
- Scrollable output
- Clear per-task separation

---

## 14. Error Handling & Recovery

### US-14.1: View Error Messages
**As a** TB operator
**I want to** see clear error messages
**So that** I can understand what went wrong

**Acceptance Criteria:**
- Errors displayed prominently
- Error type and message shown
- Stack trace available if relevant
- Actionable suggestions

### US-14.2: Handle Connection Errors
**As a** TB operator
**I want to** recover from WebSocket disconnects
**So that** the UI remains functional

**Acceptance Criteria:**
- Disconnect notification shown
- Automatic reconnection attempts
- Manual reconnect button
- State preserved during disconnect

### US-14.3: Handle Suite Load Errors
**As a** TB operator
**I want to** see errors when suite loading fails
**So that** I can fix the issue

**Acceptance Criteria:**
- Error message with reason
- File not found vs parse error
- Suggestions for fixes
- Retry option

### US-14.4: Handle Run Errors
**As a** TB operator
**I want to** see errors when runs fail
**So that** I can investigate issues

**Acceptance Criteria:**
- Error state in run status
- Task-level error details
- Subprocess exit codes
- Log output for debugging

### US-14.5: Recover from Crashed Run
**As a** TB operator
**I want to** recover from a crashed run
**So that** I don't lose progress

**Acceptance Criteria:**
- Partial results saved
- Resume from last checkpoint
- Identify crashed task
- Option to skip problematic task

### US-14.6: Handle Timeout Tasks
**As a** TB operator
**I want to** see timeout information
**So that** I can adjust settings

**Acceptance Criteria:**
- Timeout tasks clearly marked
- Timeout duration shown
- Suggestion to increase timeout
- Option to retry with longer timeout

---

## 15. Navigation & UI

### US-15.1: Navigate Between Views
**As a** TB operator
**I want to** switch between different views
**So that** I can focus on relevant information

**Acceptance Criteria:**
- View tabs/buttons for: Suite, Output, Results, History
- Active view highlighted
- Keyboard shortcuts for switching
- State preserved when switching

### US-15.2: Resize Widgets
**As a** TB operator
**I want to** resize widget panels
**So that** I can allocate screen space

**Acceptance Criteria:**
- Drag handles between panels
- Minimum sizes enforced
- Double-click to reset
- Sizes persist in session

### US-15.3: Full-Screen Widget
**As a** TB operator
**I want to** expand a widget to full screen
**So that** I can focus on details

**Acceptance Criteria:**
- Full-screen button on widgets
- Esc to exit full-screen
- Other widgets hidden
- Return to previous layout

### US-15.4: Pin Important Widgets
**As a** TB operator
**I want to** pin widgets to always be visible
**So that** I don't lose important information

**Acceptance Criteria:**
- Pin button on widget headers
- Pinned widgets stay visible
- Unpin to allow hiding
- Pinned state persists

### US-15.5: Scroll Long Lists
**As a** TB operator
**I want to** scroll through long lists
**So that** I can see all items

**Acceptance Criteria:**
- Smooth scrolling
- Scroll position preserved
- Scroll-to-top button
- Infinite scroll for history

### US-15.6: Zoom UI
**As a** TB operator
**I want to** zoom the UI in/out
**So that** I can adjust for my display

**Acceptance Criteria:**
- Cmd+/- zooms UI
- Zoom level indicator
- Reset to default option
- Persists across sessions

---

## 16. Keyboard Shortcuts

### US-16.1: Start/Stop Run
**As a** TB operator
**I want to** start/stop runs with keyboard
**So that** I can work efficiently

**Acceptance Criteria:**
- Cmd+Enter starts run
- Cmd+. stops run
- Shortcuts shown in tooltips
- Works from any widget

### US-16.2: Navigate Tasks
**As a** TB operator
**I want to** navigate tasks with arrow keys
**So that** I can browse quickly

**Acceptance Criteria:**
- Up/Down moves between tasks
- Enter selects/toggles task
- Space toggles checkbox
- Home/End for first/last

### US-16.3: Clear Output
**As a** TB operator
**I want to** clear output with keyboard
**So that** I can quickly reset view

**Acceptance Criteria:**
- Cmd+K clears output
- Works in output widget
- Confirmation if run active
- Shortcut shown in UI

### US-16.4: Toggle Panels
**As a** TB operator
**I want to** toggle panels with keyboard
**So that** I can manage layout quickly

**Acceptance Criteria:**
- Cmd+1/2/3 toggles panels
- Cmd+0 shows all
- Shortcuts in help
- Visual feedback on toggle

### US-16.5: Open Help
**As a** TB operator
**I want to** see keyboard shortcuts help
**So that** I can learn available shortcuts

**Acceptance Criteria:**
- Cmd+? opens shortcut panel
- All shortcuts listed
- Categorized by function
- Searchable

### US-16.6: Quick Search
**As a** TB operator
**I want to** search with Cmd+F
**So that** I can find content quickly

**Acceptance Criteria:**
- Cmd+F opens search
- Searches current widget
- Cmd+G for next match
- Esc closes search

---

## 17. Widget Interactions

### US-17.1: Drag and Drop Tasks
**As a** TB operator
**I want to** drag tasks to reorder
**So that** I can prioritize execution order

**Acceptance Criteria:**
- Drag handle on task rows
- Drop indicator shows position
- Order persists for run
- Reset order option

### US-17.2: Hover for Details
**As a** TB operator
**I want to** hover for more details
**So that** I can see information without clicking

**Acceptance Criteria:**
- Hover shows tooltip
- Delay before showing (300ms)
- Rich content in tooltip
- Pin tooltip option

### US-17.3: Right-Click Context Menu
**As a** TB operator
**I want to** right-click for actions
**So that** I can access common operations

**Acceptance Criteria:**
- Context menu on right-click
- Relevant actions for element
- Keyboard shortcuts shown
- Submenu for grouped actions

### US-17.4: Double-Click Actions
**As a** TB operator
**I want to** double-click for primary action
**So that** I can quickly perform common operations

**Acceptance Criteria:**
- Double-click task runs it
- Double-click run shows details
- Double-click trajectory loads it
- Consistent behavior

### US-17.5: Multi-Select Tasks
**As a** TB operator
**I want to** select multiple tasks at once
**So that** I can operate on groups

**Acceptance Criteria:**
- Shift+click for range select
- Cmd+click for add to selection
- Select all with Cmd+A
- Selection count shown

### US-17.6: Widget Tooltips
**As a** TB operator
**I want to** see helpful tooltips on buttons
**So that** I understand what each does

**Acceptance Criteria:**
- All buttons have tooltips
- Tooltip shows action + shortcut
- Delay before showing
- Consistent styling

---

## 18. Socket & Connection

### US-18.1: View Connection Status
**As a** TB operator
**I want to** see WebSocket connection status
**So that** I know if the UI is connected

**Acceptance Criteria:**
- Connection indicator in UI
- Green = connected, red = disconnected
- Shows reconnection attempts
- Click to manually reconnect

### US-18.2: Auto-Reconnect on Disconnect
**As a** TB operator
**I want to** auto-reconnect when disconnected
**So that** the UI recovers automatically

**Acceptance Criteria:**
- Automatic reconnection attempts
- Exponential backoff
- Max retry limit
- Notification of reconnection

### US-18.3: View Server Status
**As a** TB operator
**I want to** see if the desktop server is running
**So that** I can start it if needed

**Acceptance Criteria:**
- Server status indicator
- Instructions if server not running
- Quick start command shown
- Link to server logs

### US-18.4: Handle Network Errors
**As a** TB operator
**I want to** see network error details
**So that** I can troubleshoot issues

**Acceptance Criteria:**
- Error type displayed
- Connection refused vs timeout
- Retry button
- Help link for common issues

---

## Summary

| Category | Story Count |
|----------|-------------|
| Suite Loading & Configuration | 5 |
| Task Selection & Filtering | 11 |
| Run Execution | 11 |
| Real-Time Monitoring | 8 |
| Output & Verification | 10 |
| Results & Metrics | 8 |
| History & Trajectories | 8 |
| Comparison & Baselines | 5 |
| Settings & Configuration | 7 |
| MechaCoder Integration | 8 |
| Overnight Iteration Runs | 6 |
| Dashboard & Reporting | 4 |
| Sandbox Mode | 5 |
| Error Handling & Recovery | 6 |
| Navigation & UI | 6 |
| Keyboard Shortcuts | 6 |
| Widget Interactions | 6 |
| Socket & Connection | 4 |
| **Total** | **114** |

---

## Implementation Priority

### Phase 1: Core Functionality (Critical Path)
- US-1.1 through US-1.4 (Suite Loading)
- US-3.1, US-3.2, US-3.4 (Run Execution)
- US-4.1 through US-4.5 (Monitoring)
- US-5.1, US-5.2 (Output)
- US-6.1, US-6.2 (Results)

### Phase 2: Enhanced Selection & Filtering
- US-2.1 through US-2.11 (Task Selection)
- US-5.3 through US-5.7 (Output Features)

### Phase 3: History & Comparison
- US-7.1 through US-7.6 (History)
- US-8.1 through US-8.4 (Comparison)

### Phase 4: Advanced Features
- US-9.* (Settings)
- US-10.* (MechaCoder)
- US-11.* (Overnight Runs)
- US-13.* (Sandbox)

### Phase 5: Polish & Optimization
- US-15.* (Navigation)
- US-16.* (Keyboard)
- US-17.* (Interactions)
- US-14.* (Error Handling)
