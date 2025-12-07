Nice, this is super clear. I’ll break it into two buckets:

1. **Components implicitly or explicitly present in your plan / code**
2. **Additional components we *should* extract / create for v1**

I’ll treat “component” loosely in Effuse terms: anything that has a clear responsibility and can be its own widget/template.

---

## 1. Components already identified in the plan/code

### Top-level widgets / pages

These are explicitly in the plan and/or implemented:

1. **`TBCCShellWidget`** (`tbcc-shell.ts`)

   * Shell, sidebar, tab nav, status bar, tab content containers.

2. **`TBCCDashboardWidget`** (`tbcc-dashboard.ts`)

   * Dashboard page: KPIs, quick actions, recent runs, current run status.

3. **`TBCCTaskBrowserWidget`** (`tbcc-task-browser.ts`, planned)

   * Task browser page: task list + detail, filters, run buttons.

4. **`TBCCRunBrowserWidget`** (`tbcc-run-browser.ts`, planned)

   * Run browser page: unified HF + local runs, list + run detail, trajectory display.

5. **`TBCCSettingsWidget`** (`tbcc-settings.ts`, planned)

   * Settings page: execution settings, logging settings, persistence.

6. **`hf-trajectory-list`** (existing)

   * To be reused/adapted as the **Run list** in the Runs tab.

7. **`hf-trajectory-detail`** (existing)

   * To be reused/adapted as the **Run detail / step accordion**.

8. **`tb-controls`** (existing)

   * To be reused in **Task browser** for suite loading + run control logic.

9. **`tb-output`** (existing)

   * To be reused as **terminal output panel** in run detail.

---

### Logical subcomponents inside `TBCCShellWidget`

These are in the shell code already (could stay inline or be refactored):

10. **Sidebar / Navigation**

    * Renders app title, “Command Center” label, and tab buttons.
11. **Tab Navigation Item** (each button in `TABS`)

    * Individual clickable tab entry.
12. **Sidebar Collapse Toggle**

    * The `← Collapse` / `→` button.
13. **Connection / Run Status Bar**

    * The status dot + “Ready / Disconnected / current task name” text.
14. **Main Tab Containers**

    * `#tbcc-tab-dashboard`, `#tbcc-tab-tasks`, `#tbcc-tab-runs`, `#tbcc-tab-settings`.

---

### Logical subcomponents inside `TBCCDashboardWidget`

The dashboard widget already has these logical pieces:

15. **Current Run Status Card**

    * The blue card with “Task is running, step X/Y, attempt A/B”, and Stop button.

16. **KPI Grid**

    * The `grid` rendering success rate, avg steps, avg duration, total runs.

17. **KPI Card** (per metric)

    * The individual “Success Rate / Avg Steps / Avg Duration / Total Runs” boxes.

18. **Quick Actions Row**

    * Buttons: “Run Full Benchmark”, “Random Task”, “Refresh”.

19. **Recent Runs Table**

    * Header row (“Task / Outcome / Steps / Duration / Date / View”)
    * 0–10 recent runs as table rows.

20. **Recent Run Row** (per run)

    * Task name, outcome badge, steps, duration, date, “View →” button.

21. **Outcome Badge**

    * Uses `OUTCOME_COLORS` mapping to style success/failure/running etc.

22. **Error Banner**

    * Red box with error text and “Retry” button.

23. **Loading Skeletons**

    * Animated placeholder KPI cards and “Loading recent runs…” state.

---

### State types / domain structures (supporting components)

24. **`TBCCShellState`, `TBCCShellEvent`**
25. **`TBCCDashboardState`, `TBCCDashboardEvent`**
26. **Domain types like `DashboardStats`, `TBRunSummary`, `CurrentRunInfo`**, etc. (in `types.ts`)

These aren’t visual components but they drive all the UI components above.

---

## 2. Components not in the code yet but worth creating

Now, here’s a list of **additional components** that are not explicitly written yet but will make the TB Command Center much cleaner, more modular, and easier to evolve:

### A. Task Browser components

We’ll want to split `tbcc-task-browser.ts` into:

1. **`TaskBrowserLayout`**

   * The left-right split container (`task-list` | `task-detail`).

2. **`TaskListPanel`**

   * Wraps filters + list.
   * Handles scroll, empty/loading states.

3. **`TaskFilterBar`**

   * Search box.
   * Difficulty filter (all/easy/medium/hard).
   * Status filter (passed/failed/untried).

4. **`TaskListItem`**

   * Single task row: name, difficulty pill, last status dot/badge.

5. **`TaskDetailPanel`**

   * Right-hand side: task name, difficulty, description, metadata.
   * Hooks to `tb-controls` for run logic.

6. **`TaskRunControls`**

   * Buttons: “Run single”, “Run 5 attempts”, maybe “Run all variants”.
   * Indicates pending/running state per task.

7. **`TaskMetadataSection`**

   * Container, time limit, tags, #runs, etc.
   * Keeps `TaskDetailPanel` focused.

8. **`TaskEmptyState`**

   * Shown when no tasks loaded or search returns nothing.

---

### B. Run Browser components

Similarly for `tbcc-run-browser.ts`:

9. **`RunBrowserLayout`**

   * Left: run list; Right: run detail.

10. **`RunListPanel`**

    * Unified HF + local runs list.
    * Source toggle (“All / HF only / Local only”).

11. **`RunSourceFilter`**

    * The “dataSource: all/hf/local” control.

12. **`RunListItem`**

    * One run row: taskName, source (HF/local), outcome badge, date.

13. **`RunDetailPanel`**

    * Uses `hf-trajectory-detail` patterns.
    * Contains summary + step accordion + terminal output.

14. **`RunSummaryHeader`**

    * Task name, outcome badge, pass rate, run duration, attempts.

15. **`RunStepAccordion`**

    * Vertical accordion of steps (like `hf-trajectory-detail`).
    * Step header/row + toggle open state.

16. **`RunStepHeader`**

    * Step index, tool/action, short reason, success/failure dot.

17. **`RunStepBody`**

    * Command text, arguments, stdout/stderr snippet, tests triggered.

18. **`TerminalOutputPanel`**

    * Reusable terminal view (backed by `tb-output` logic).
    * Scroll lock toggle + jump-to-step integration.

19. **`RunEmptyState`**

    * When there are no runs found for the current filter.

20. **`RunLoadingState`**

    * Skeleton for run list or detail while loading from HF / disk.

---

### C. Settings components

For `tbcc-settings.ts`:

21. **`SettingsLayout`**

    * Page container with Execution + Logging sections.

22. **`ExecutionSettingsCard`**

    * Group for `maxAttempts`, `maxSteps`, `timeout`, deep compute toggles.

23. **`LoggingSettingsCard`**

    * Group for `saveTrajectories`, `saveTerminalOutput`, `autoPrune`.

24. **`SettingsField` / `SettingsRow`**

    * Label + control + helper text.
    * Reused in both cards.

25. **`SettingsSectionHeader`**

    * Title + small description.

---

### D. Shared UI primitives (worth extracting once)

These don’t have to be Effuse widgets; they’re reusable HTML helpers/partials:

26. **`PageContainer`**

    * Standard layout: padding, scroll, header.

27. **`Card`**

    * Generic dark card with border and rounded corners; used everywhere.

28. **`Badge` / `Pill`**

    * Styled label for outcome, difficulty, source.
    * e.g. `DifficultyPill`, `OutcomeBadge`, `SourceBadge`.

29. **`StatusDot`**

    * Tiny colored indicator: success/failure/running/idle.

30. **`Button` / `IconButton`**

    * Your Effuse buttons with consistent Tailwind classes.

31. **`Table` & `TableRow`** (or at least helpers)

    * Standard table wrapper used by dashboard + run list + maybe tasks.

32. **`Skeleton`**

    * Generic loading skeleton bar/box used in KPI grid + lists.

33. **`ErrorBanner` (generic)**

    * Re-usable version of the red error banner in dashboard.

34. **`EmptyState` (generic)**

    * For “no tasks”, “no runs”, etc.

---

### E. Domain-specific helpers

These are UI fragments that encode your domain logic:

35. **`OutcomeBadge`** (already half-implemented via `OUTCOME_COLORS`)

    * Wraps the span with the right class set.

36. **`DifficultyBadge`**

    * Color-coded difficulty; used in Task list + detail.

37. **`RunDurationLabel`**

    * Uses `formatDuration` logic to present durations consistently.

38. **`DateLabel`**

    * Uses `formatDate` consistently for all tables.

39. **`RunningRunChip`**

    * Little chip with current run + pulsing dot, for the shell status bar and maybe dashboard.

---

If you want, next I can:

* Turn these into a **concrete checklist** (with file names like `tbcc-task-browser-task-list.ts`)
* Or go deeper on one tab (e.g. Run Browser) and sketch the full component tree with explicit props for each.
