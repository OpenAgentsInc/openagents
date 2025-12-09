# StarCraft Inspiration: The Performance-Driven Power User Interface

## Executive Summary

OpenAgents can learn from **StarCraft's competitive mastery paradigm**: the idea that a powerful tool rewards skill, practice, and optimization. StarCraft players don't complain about complexity—they embrace it. They measure themselves in **APM (Actions Per Minute)**, memorize dozens of **hotkeys**, manage **multiple unit groups** simultaneously, and constantly optimize **build orders** to shave seconds off their timing.

**The Core Insight**: Managing AI agents isn't casual browsing—it's performance-driven work. Power users want **efficiency**, **control**, and **measurability**. They want to know: "How many jobs did I process? How fast? At what cost? Can I do it faster tomorrow?"

This document explores how StarCraft's UI/UX patterns—from the iconic **minimap** to **control groups** to **APM tracking**—can inspire a power-user interface for OpenAgents that rewards mastery and optimization.

---

## The Mental Model Shift

### From Casual Tool to Competitive Performance

```
CASUAL TOOL MENTALITY           STARCRAFT/POWER USER MENTALITY
━━━━━━━━━━━━━━━━━━━━            ━━━━━━━━━━━━━━━━━━━━━━━━━━━━
"I use AI when I need it"       "I maximize my agent throughput every day"
"Click buttons to get work done" "I use keyboard shortcuts for everything"
"One task at a time"            "I manage 12 parallel agent workflows"
"Success = task completed"      "Success = optimal cost/latency/quality ratio"
"Casual usage"                  "Performance measurement (jobs/hour, cost per job)"
```

**Why this matters**:

1. **Self-Selection**: Hardcore users self-select. StarCraft doesn't try to be easy—it's hard, and that's the point. OpenAgents can serve power users without dumbing down.

2. **Mastery Curve**: StarCraft players spend thousands of hours mastering mechanics. Power users will invest time learning OpenAgents if the ROI is clear (10x productivity, passive income).

3. **Measurement Culture**: StarCraft players obsess over metrics (APM, win rate, game length). OpenAgents users should obsess over efficiency metrics (jobs/hour, cost per job, uptime percentage).

4. **Community Competition**: StarCraft has leaderboards, tournaments, coaching. OpenAgents can have efficiency leaderboards, shared strategies, performance benchmarks.

5. **Iterative Optimization**: StarCraft players constantly refine build orders, unit compositions, micro techniques. OpenAgents users should constantly optimize workflows, model selection, prompts.

---

## StarCraft → OpenAgents: Core Parallels

### High-Level Mapping

| StarCraft Concept | OpenAgents Equivalent | Why It Works |
|------------------|----------------------|--------------|
| **APM (Actions Per Minute)** | **Jobs Per Hour / Efficiency Metrics** | Both measure productive output rate |
| **Hotkeys (keyboard shortcuts)** | **Agent Management Shortcuts** (Cmd+1-9, quick commands) | Both enable rapid execution without mouse |
| **Control Groups (1-9)** | **Agent Groups** (assign agents to groups, select with number keys) | Both manage multiple units/agents simultaneously |
| **Minimap** | **Dashboard Overview** (spatial or list view of all agents) | Both provide high-level awareness at a glance |
| **Resource Counters** (minerals, vespene, supply) | **Token Usage, Sats Balance, API Limits** | Both track constrained resources |
| **Unit Selection UI** | **Agent Selection Panel** (show selected agents, status, controls) | Both display multi-select state and actions |
| **Build Orders** | **Workflow Templates / Blueprints** | Both are optimized sequences to achieve goals |
| **Macro Management** | **Strategic Workflow Design** (overall architecture) | Both focus on high-level strategy |
| **Micro Management** | **Per-Agent Tuning** (prompts, model selection, parameters) | Both focus on tactical details |
| **Fog of War** | **Observability Gaps** (distributed agents, async jobs) | Both deal with incomplete information |
| **Supply Cap** | **Rate Limits / Token Quotas** | Both limit expansion/throughput |
| **Replay System** | **Job History / Performance Logs** | Both review past actions for analysis |
| **Match Statistics** | **Production Dashboard** (earnings, success rate, latency) | Both track performance over time |
| **Camera Hotkeys** (F1-F8 locations) | **Workspace Hotkeys** (jump to specific agent groups or views) | Both enable rapid navigation |
| **Shift-Queueing Commands** | **Job Queuing** (queue multiple tasks for an agent) | Both batch commands for efficiency |

### Key Insight: Speed and Control

**StarCraft rewards speed**: The player who can execute more actions per minute (with precision) wins.
**OpenAgents should reward speed**: The user who can process more jobs per hour (with quality) succeeds.

This means:
- **Keyboard-first UI**: Mouse is too slow for power users
- **Minimal clicks**: Every action should be <3 keystrokes
- **Batch operations**: Select multiple agents, apply changes to all
- **Macro support**: Automate repetitive workflows
- **Performance metrics**: Track and display efficiency

---

## Visual & Interaction Principles from StarCraft

### 1. The Minimap: Always-Visible Overview

**StarCraft**: The minimap is always in the lower-left corner. It shows the entire map, unit positions, and enemy activity at a glance. Clicking it jumps the camera.

**OpenAgents Application**:

```
┌────────────────────────────────────────────────────┐
│  [Main Content: Agent Details or Canvas]          │
│                                                     │
│                                                     │
│  ┌─────────────┐                                   │
│  │ [Minimap]   │  ← Lower-left corner (persistent)│
│  │ · · · ● · · │  ← Dots = agents, colored by status│
│  │ · ● · · ● · │  ← Click to jump to agent/group  │
│  │ · · ● · · · │                                   │
│  └─────────────┘                                   │
└────────────────────────────────────────────────────┘
```

**Features**:
- **Persistent**: Always visible (can't close it)
- **Color-coded**: Green dots (busy), blue (idle), red (error)
- **Clickable**: Click a dot to jump to that agent's detail view
- **Spatial**: If using canvas view, shows agent positions spatially
- **List View**: If not using canvas, shows vertical list of agent statuses

**Why It Works**: Glanceable awareness. Power users can monitor 20+ agents without leaving their current focus.

### 2. Resource Counters: Always-Visible Constraints

**StarCraft**: Top-right corner shows minerals, vespene gas, supply (e.g., `500 minerals, 300 gas, 50/200 supply`).

**OpenAgents Application**:

```
┌────────────────────────────────────────────────────┐
│  💰 1,234 sats    🔥 8.5k/10k tokens    ⚡ 47/hr  │  ← Top bar (always visible)
│  ──────────────────────────────────────────────────│
│  [Main Content]                                    │
└────────────────────────────────────────────────────┘
```

**Metrics Displayed**:
- **Sats Balance**: Current wallet balance (like minerals)
- **Token Usage**: Daily usage / daily limit (like supply cap)
- **Jobs/Hour**: Current throughput (performance metric)
- **Active Agents**: Number of busy agents / total agents

**Color Coding**:
- Green: Healthy (plenty of resources)
- Orange: Warning (approaching limit)
- Red: Critical (at or over limit)

**Why It Works**: Constant awareness of constraints. Users know when they're approaching limits before hitting errors.

### 3. Unit Selection Panel: Multi-Select Status

**StarCraft**: Bottom-center shows selected units—their health, energy, abilities, and count.

**OpenAgents Application**:

```
┌────────────────────────────────────────────────────┐
│  [Main Content]                                    │
│                                                     │
│  ──────────────────────────────────────────────────│
│  SELECTED: 3 agents                                │
│  ┌────────┐ ┌────────┐ ┌────────┐                 │
│  │CodeGen │ │ Test   │ │Review  │                 │
│  │ BUSY   │ │ IDLE   │ │ BUSY   │                 │
│  └────────┘ └────────┘ └────────┘                 │
│  [Start All] [Stop All] [Configure Group]          │
└────────────────────────────────────────────────────┘
```

**Features**:
- **Thumbnails**: Show each selected agent's name and status
- **Aggregate Actions**: Buttons to perform actions on all selected agents
- **Deselect**: Click outside or press Escape to deselect

**Why It Works**: Batch operations. Managing 10 agents individually is tedious; selecting them all and running "Start All" is fast.

### 4. Hotkeys: Keyboard-Driven Power

**StarCraft**: Nearly every action has a hotkey. Players rarely use the mouse for commands. Examples:
- `A` = Attack
- `S` = Stop
- `1-9` = Control groups (select specific units)
- `F1-F8` = Camera locations
- Shift+command = Queue command
- Ctrl+number = Assign to control group

**OpenAgents Application**:

```
GLOBAL HOTKEYS
Cmd+1-9      Select agent group 1-9
Cmd+Shift+1-9 Assign selected agents to group 1-9
Cmd+Space    Quick command palette (fuzzy search all actions)
Cmd+J        Jump to agent (type name to filter)
Cmd+K        Command runner (execute action on selected agents)
Cmd+M        Toggle minimap size (collapsed ↔ expanded)

AGENT CONTROL
S            Start selected agents
T            Stop selected agents (T for "Terminate")
R            Restart selected agents
L            View logs for selected agent
C            Configure selected agent(s)
D            Duplicate selected agent

VIEW NAVIGATION
Tab          Cycle through active agents (busy ones first)
Shift+Tab    Cycle backward
F            Focus on selected agent (jump to detail view)
Escape       Deselect all
```

**Why It Works**: Speed. A power user can navigate 20 agents, start 5, stop 3, and jump to logs—all in <10 keystrokes, <5 seconds.

### 5. APM Tracking: Measure Efficiency

**StarCraft**: The game tracks **APM (Actions Per Minute)**—a player with 300 APM is objectively faster than one with 150 APM.

**OpenAgents Application**:

```
┌────────────────────────────────────────────────────┐
│  YOUR PERFORMANCE STATS                            │
│  ──────────────────────────────────────────────────│
│  Jobs Processed (Today):     47                    │
│  Jobs Per Hour:              5.9 (avg this week)   │
│  Commands Per Session:       142 (efficiency: high)│
│  Keyboard Shortcut Usage:    89% (power user!)     │
│  Avg Response Time:          1.2s (reaction speed) │
│  Workflow Switches:          23 (multi-tasking)    │
│                                                     │
│  🏆 Rank: Top 5% of users (by jobs/hour)           │
└────────────────────────────────────────────────────┘
```

**Metrics to Track**:
- **Jobs Per Hour**: Primary productivity metric (like APM)
- **Keyboard Shortcut Usage**: % of actions via keyboard vs mouse (power user indicator)
- **Commands Per Session**: How many actions user executes (engagement)
- **Avg Response Time**: How quickly user reacts to job completions, errors (like APM for decision speed)
- **Workflow Efficiency**: Jobs completed per $ spent (cost optimization)

**Leaderboard**:
- Top users by jobs/hour
- Top users by efficiency (lowest cost per job)
- Top users by keyboard usage (true power users)

**Why It Works**: **What gets measured gets optimized**. Showing users their APM-equivalent metrics gamifies productivity.

### 6. Control Groups: Manage Agent Clusters

**StarCraft**: Players assign units to control groups (1-9). Press `1` to select army, `2` to select workers, `3` to select base defenses. This is essential for managing large armies.

**OpenAgents Application**:

```
WORKFLOW
1. User selects 5 code generation agents
2. User presses Cmd+Shift+1 (assign to group 1)
3. Later, user presses Cmd+1 → all 5 agents selected instantly
4. User presses S → start all 5 agents

USAGE EXAMPLE
Cmd+1        → Select "Code Gen Swarm" (5 agents)
Cmd+2        → Select "Test Runners" (3 agents)
Cmd+3        → Select "Reviewers" (2 agents)
Cmd+4        → Select "Documentation Writers" (4 agents)

Cmd+Shift+5  → Assign current selection to group 5 (create new group)
```

**UI Indicator**:
- Agent cards show small badge: `[1]` `[2]` `[3]` (which group they're in)
- Status bar shows: "Group 1: 5 agents selected" when Cmd+1 is pressed

**Why It Works**: Managing 20+ agents individually is impossible. Control groups let users think in terms of **roles** (code gen swarm, test runners) rather than individual agents.

### 7. Build Orders: Optimized Sequences

**StarCraft**: Competitive players memorize "build orders"—precise sequences of actions to maximize efficiency. Example:
```
12 supply depot
13 barracks
14 refinery
16 marine
17 orbital command
...
```

**OpenAgents Application**: **Workflow Templates as Build Orders**

```
TEMPLATE: "Code Review Pipeline v2"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Step 1: Spin up 2 × Code Gen agents (Sonnet 4.5)
Step 2: Connect to 3 × Test agents (Haiku 3.7, parallel)
Step 3: Connect all test outputs to 1 × Review agent (Opus 4.5)
Step 4: Configure timeout: 5 minutes
Step 5: Enable caching (7-day TTL)
Step 6: Set priority queue: High

Expected Cost: $0.28/job
Expected Time: 3m 15s
Success Rate (from 1000 runs): 97.2%
```

**Features**:
- Users can create templates from existing workflows
- Share templates in marketplace (like sharing build orders)
- Rate templates by efficiency (cost, time, success rate)
- "Speedrun mode": Track how fast user can execute a template from scratch

**Why It Works**: Optimization culture. StarCraft players share and refine build orders; OpenAgents users will do the same with workflow templates.

### 8. Macro vs Micro: Strategic vs Tactical

**StarCraft**:
- **Macro**: Economy, base expansion, unit production (big picture)
- **Micro**: Individual unit control, combat tactics (details)

Good players balance both. Great players excel at both simultaneously.

**OpenAgents Application**:

```
MACRO (Strategic Workflow Management)
- Design overall workflow architecture (which agents, how they connect)
- Monitor production metrics (jobs/hour, costs, success rates)
- Optimize resource allocation (which agents get premium models?)
- Plan for scale (add more agents to bottlenecks)

MICRO (Per-Agent Optimization)
- Tune individual agent prompts (refine instructions)
- Select optimal model for each agent (Haiku vs Sonnet vs Opus)
- Configure retry logic, timeouts, caching
- Debug specific job failures (inspect logs, fix edge cases)
```

**UI Support**:
- **Macro View**: Dashboard with aggregate metrics, workflow visualizations
- **Micro View**: Agent detail panel with logs, config, performance

**Hotkeys to Switch**:
- `M` = Macro view (dashboard)
- `D` = Micro view (agent details)

**Why It Works**: Power users need both perspectives. Toggling between them quickly (one keystroke) enables fluid workflow.

---

## The User Journey: From Casual to Competitive

### Phase 1: Casual Use (Mouse-Driven)

**Profile**: New user, exploring features
**Behavior**:
- Uses mouse for everything
- Clicks buttons to start/stop agents
- Views one agent at a time
- Doesn't know hotkeys exist

**Metrics**:
- Jobs/hour: 2-3 (slow)
- Keyboard usage: <10%

### Phase 2: Intermediate (Learning Shortcuts)

**Profile**: Regular user, starting to optimize
**Behavior**:
- Learns basic hotkeys (Cmd+Space for command palette)
- Uses Tab to cycle through agents
- Discovers control groups (Cmd+1-9)
- Still uses mouse for detail work

**Metrics**:
- Jobs/hour: 8-12 (improving)
- Keyboard usage: 40-60%

### Phase 3: Power User (Keyboard-First)

**Profile**: Daily user, optimizing workflows
**Behavior**:
- Uses hotkeys for 90% of actions
- Has memorized control groups (Cmd+1 = code swarm, Cmd+2 = tests)
- Uses quick command palette (Cmd+Space) constantly
- Batch operations (select multiple agents, apply changes)
- Monitors performance metrics (dashboard always visible)

**Metrics**:
- Jobs/hour: 30-50 (high throughput)
- Keyboard usage: 85-95%

### Phase 4: Competitive (Optimizing for Leaderboard)

**Profile**: Hardcore user, treating it like a competitive game
**Behavior**:
- Measures everything (tracks personal APM-equivalent metrics)
- Optimizes workflows obsessively (shaves seconds off timing)
- Shares strategies in community (Discord, forums)
- Competes on leaderboards (top jobs/hour, efficiency)
- Creates video tutorials ("How I hit 100 jobs/hour")

**Metrics**:
- Jobs/hour: 80-150+ (top 1%)
- Keyboard usage: 98%+
- Workflow efficiency: Lowest cost per job

**Engagement**:
- Daily usage (checks dashboard multiple times/day)
- Active in community (shares tips, templates)
- Evangelical (brings in other power users)

---

## Key Features Inspired by StarCraft

### 1. Persistent Minimap (Lower-Left Corner)

**Implementation**:
```tsx
// Always-visible minimap component
<div className="minimap-container fixed bottom-4 left-4 w-40 h-32
                bg-black/80 border border-white/20 backdrop-blur">
  <MinimapGrid
    agents={allAgents}
    onAgentClick={(id) => jumpToAgent(id)}
    colorByStatus={true}
  />
</div>
```

**Features**:
- Shows all agents as colored dots
- Click to jump to agent detail
- Can't be closed (persistent awareness)
- Hover shows agent name + status

### 2. Resource Counter Bar (Top-Right)

**Implementation**:
```tsx
<div className="resource-bar fixed top-0 right-0 flex gap-6 px-6 py-2
                bg-black/80 border-b border-white/20">
  <ResourceCounter icon="💰" label="Sats" value={balance} max={null} />
  <ResourceCounter icon="🔥" label="Tokens" value={tokensUsed} max={tokenLimit} color="warning" />
  <ResourceCounter icon="⚡" label="Jobs/hr" value={jobRate} trend="+12%" />
</div>
```

**Color Logic**:
- Green: Value < 70% of limit
- Orange: Value 70-90% of limit
- Red: Value > 90% of limit

### 3. Control Groups (Number Keys 1-9)

**Implementation**:
```tsx
// Assign agents to group
const assignToGroup = (groupNum: number, agentIds: string[]) => {
  controlGroups.set(groupNum, agentIds);
  toast.success(`Assigned ${agentIds.length} agents to group ${groupNum}`);
};

// Select group
const selectGroup = (groupNum: number) => {
  const agentIds = controlGroups.get(groupNum) || [];
  setSelectedAgents(agentIds);
  toast.info(`Selected group ${groupNum}: ${agentIds.length} agents`);
};

// Keyboard listener
useEffect(() => {
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.metaKey && e.key >= '1' && e.key <= '9') {
      const groupNum = parseInt(e.key);
      if (e.shiftKey) {
        // Cmd+Shift+Number: Assign to group
        assignToGroup(groupNum, selectedAgents);
      } else {
        // Cmd+Number: Select group
        selectGroup(groupNum);
      }
    }
  };
  window.addEventListener('keydown', handleKeyDown);
  return () => window.removeEventListener('keydown', handleKeyDown);
}, [selectedAgents]);
```

### 4. APM-Style Performance Tracking

**Implementation**:
```tsx
interface PerformanceMetrics {
  jobsPerHour: number;           // Primary productivity metric
  commandsPerSession: number;    // How many actions user performed
  keyboardUsagePercent: number;  // % of actions via keyboard vs mouse
  avgResponseTime: number;       // How fast user reacts (in seconds)
  workflowSwitches: number;      // How often user switches contexts
  efficiencyScore: number;       // Composite score (weighted average)
}

// Track user actions
const trackAction = (type: 'keyboard' | 'mouse', command: string) => {
  actions.push({ type, command, timestamp: Date.now() });

  // Calculate metrics
  const keyboardActions = actions.filter(a => a.type === 'keyboard').length;
  const totalActions = actions.length;
  metrics.keyboardUsagePercent = (keyboardActions / totalActions) * 100;

  // Jobs per hour (from last 1 hour of activity)
  const lastHour = actions.filter(a => a.timestamp > Date.now() - 3600000);
  metrics.jobsPerHour = lastHour.filter(a => a.command === 'job:complete').length;
};
```

**Dashboard Display**:
```tsx
<MetricCard
  label="Jobs/Hour"
  value={metrics.jobsPerHour}
  rank="Top 5%"
  icon="⚡"
/>
<MetricCard
  label="Keyboard Usage"
  value={`${metrics.keyboardUsagePercent.toFixed(0)}%`}
  badge={metrics.keyboardUsagePercent > 80 ? "Power User" : null}
  icon="⌨️"
/>
```

### 5. Quick Command Palette (Cmd+Space)

**Implementation** (inspired by Cmd+K in Notion, but also StarCraft's command card):

```tsx
<CommandPalette
  trigger="Cmd+Space"
  commands={[
    { id: 'start-all', label: 'Start All Selected Agents', hotkey: 'S' },
    { id: 'stop-all', label: 'Stop All Selected Agents', hotkey: 'T' },
    { id: 'jump-to', label: 'Jump to Agent...', hotkey: 'Cmd+J' },
    { id: 'configure', label: 'Configure Selected', hotkey: 'C' },
    { id: 'view-logs', label: 'View Logs', hotkey: 'L' },
    { id: 'duplicate', label: 'Duplicate Selected', hotkey: 'D' },
    // ... more commands
  ]}
  onExecute={(commandId) => executeCommand(commandId, selectedAgents)}
/>
```

**Features**:
- Fuzzy search (type "sta" → finds "Start All Selected Agents")
- Shows hotkey next to each command (learn as you go)
- Recent commands at top
- Can execute on current selection or prompt for target

### 6. Shift-Queue Actions (Batch Commands)

**StarCraft**: Hold Shift while issuing commands to queue them. Example: Shift+click 5 locations → unit moves to all 5 in sequence.

**OpenAgents Application**:

```tsx
// Queue multiple jobs for an agent
const queueJobs = (agentId: string, jobs: Job[]) => {
  jobs.forEach((job, index) => {
    agentQueues.get(agentId)?.push({
      ...job,
      queuePosition: index,
      status: 'queued'
    });
  });
  toast.info(`Queued ${jobs.length} jobs for agent ${agentId}`);
};

// UI: Hold Shift while clicking "Run Job" button
<Button
  onClick={(e) => {
    if (e.shiftKey) {
      // Add to queue (don't run immediately)
      queueJobs(agentId, [currentJob]);
    } else {
      // Run immediately
      runJob(agentId, currentJob);
    }
  }}
>
  Run Job {shiftHeld && "(+Queue)"}
</Button>
```

**Visual Feedback**:
- Agent card shows queue depth: `Queue: [3]`
- Hover shows queued jobs: "1. Generate tests, 2. Review code, 3. Update docs"

### 7. Replay System (Performance Analysis)

**StarCraft**: After each match, players can watch a replay to analyze their decisions and improve.

**OpenAgents Application**: **Job History with Replay/Analysis**

```tsx
<JobHistoryPanel>
  <JobEntry>
    <JobInfo>
      Job #482: Code Review Pipeline
      Duration: 3m 15s
      Cost: $0.28
      Status: ✅ Success
    </JobInfo>
    <Actions>
      <Button onClick={() => replayJob(482)}>🔄 Replay</Button>
      <Button onClick={() => analyzeJob(482)}>📊 Analyze</Button>
    </Actions>
  </JobEntry>
</JobHistoryPanel>

// Replay view: Step-through of job execution
<ReplayTimeline jobId={482}>
  <TimelineEvent time="0:00" agent="CodeGen" event="Started" />
  <TimelineEvent time="0:45" agent="CodeGen" event="Completed" output="code.ts" />
  <TimelineEvent time="0:46" agent="Test" event="Started" />
  <TimelineEvent time="2:15" agent="Test" event="Completed" output="3 tests passed" />
  <TimelineEvent time="2:16" agent="Review" event="Started" />
  <TimelineEvent time="3:15" agent="Review" event="Completed" output="2 suggestions" />
</ReplayTimeline>
```

**Features**:
- **Replay**: Step through job execution (see what each agent did, when)
- **Analysis**: Identify bottlenecks (which agent was slowest?)
- **Compare**: Compare two runs (why was run #482 faster than #481?)
- **Learn**: Export as "case study" to share with community

---

## Visual Design Language: Command & Control

### Color Palette: Military/Technical HUD

**Inspired by StarCraft's Terran UI** (blue, steel, industrial):

```
Primary Blue:   #4a9eff  ████  (unit selection, highlights)
Steel Gray:     #999999  ████  (structure, borders)
Energy Green:   #00ff00  ████  (health, shields, success)
Alert Red:      #ff0000  ████  (damage, errors, warnings)
Background:     #0a0a0a  ████  (dark, almost black)
Glass Overlay:  #1a1a1aCC ████  (translucent panels)
```

**Usage**:
- Minimap background: Dark gray with blue grid
- Selected agents: Blue border glow
- Resource counters: Green (healthy), orange (warning), red (critical)
- Active elements: Blue highlights (like StarCraft's selected units)

### Typography: Military Monospace

```
Primary Font:   Rajdhani (geometric, bold, military feel)
Monospace:      JetBrains Mono (for data, metrics, code)
UI Labels:      All caps, tight letter-spacing (RESOURCE COUNTERS, CONTROL GROUPS)
```

**Example**:
```
┌────────────────────────────────────┐
│ AGENT STATUS: ACTIVE               │  ← All caps labels
│ ────────────────────────────────── │
│ Jobs:       47/hr                  │  ← Monospace data
│ Uptime:     14h 23m                │
│ Success:    98.4%                  │
└────────────────────────────────────┘
```

### UI Layout: Always-Visible HUD Elements

**StarCraft Layout**:
```
┌──────────────────────────────────────────┐
│ [Main Game View: Units and Map]         │  ← Center focus
│                                          │
│  ┌────────┐                              │
│  │Minimap │  ← Lower-left (persistent)   │
│  └────────┘                              │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │ [Unit Selection Panel]             │ │  ← Bottom-center
│  │ [Command Card: Abilities]          │ │
│  └────────────────────────────────────┘ │
│                                          │
│  [Resources: 500m 300g 50/200] ← Top-right│
└──────────────────────────────────────────┘
```

**OpenAgents Adaptation**:
```
┌──────────────────────────────────────────┐
│ 💰 1,234 sats  🔥 8.5k/10k   ⚡ 47/hr   │  ← Top bar (resources)
│ ──────────────────────────────────────── │
│ [Main Content: Agent Details or Canvas] │  ← Center focus
│                                          │
│  ┌────────┐                              │
│  │Minimap │  ← Lower-left (agents overview)
│  └────────┘                              │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │ SELECTED: 3 agents                 │ │  ← Bottom panel
│  │ [Start All] [Stop All] [Configure] │ │
│  └────────────────────────────────────┘ │
└──────────────────────────────────────────┘
```

**Key Principles**:
- **HUD is persistent**: Minimap, resource counters, selection panel always visible
- **Center = work area**: Where user focuses (agent details, canvas, logs)
- **Edges = awareness**: Glanceable status without shifting focus

---

## Implementation Roadmap: 4 Phases

### Phase 1: HUD Foundation (Weeks 1-4)

**Goal**: Add persistent HUD elements (minimap, resource counters, selection panel)

**Deliverables**:
- Minimap component (lower-left, always visible)
- Resource counter bar (top-right: sats, tokens, jobs/hr)
- Multi-select UI (bottom panel showing selected agents)
- Basic keyboard shortcuts (S=start, T=stop, Tab=cycle)

**Success Metric**: User can monitor 10 agents without leaving current view

### Phase 2: Hotkeys & Control Groups (Weeks 5-8)

**Goal**: Implement full keyboard-driven workflow

**Deliverables**:
- Control groups (Cmd+1-9 to select, Cmd+Shift+1-9 to assign)
- Quick command palette (Cmd+Space fuzzy search)
- Jump-to-agent (Cmd+J, type name)
- Batch actions on selection (start all, stop all, configure group)
- Hotkey hints UI (press ? to show all shortcuts)

**Success Metric**: Power users execute 90% of actions via keyboard

### Phase 3: Performance Metrics (Weeks 9-12)

**Goal**: Add APM-style efficiency tracking

**Deliverables**:
- Jobs/hour tracking (real-time + historical)
- Keyboard usage percentage (mouse vs keyboard)
- Commands per session counter
- Efficiency leaderboard (top users by jobs/hr)
- Personal performance dashboard

**Success Metric**: Users check performance metrics daily, optimize to improve rank

### Phase 4: Replay & Analysis (Weeks 13-16)

**Goal**: Enable post-mortem analysis of job workflows

**Deliverables**:
- Job history with replay (step-through execution)
- Performance analysis (identify bottlenecks)
- Compare runs (why was job A faster than job B?)
- Export analysis as report (share with team/community)

**Success Metric**: Users analyze past jobs to optimize future workflows

---

## Why This Works: Competitive Psychology

### 1. Measurement → Improvement

**StarCraft**: Players track APM religiously. Seeing "I went from 120 APM to 180 APM" is motivating.
**OpenAgents**: Tracking jobs/hour creates the same drive. "I went from 12 jobs/hr to 47 jobs/hr by optimizing my workflow."

### 2. Skill Ceiling → Long-Term Engagement

**StarCraft**: The skill ceiling is extremely high (pros have 300+ APM, perfect macro/micro). This keeps players engaged for years.
**OpenAgents**: If the UI rewards mastery (keyboard shortcuts, control groups, workflow optimization), power users will invest time to get better.

### 3. Community Competition → Virality

**StarCraft**: Leaderboards, tournaments, Twitch streams create competitive community.
**OpenAgents**: Efficiency leaderboards ("Top 10 users by jobs/hour this week") + sharing strategies creates similar dynamics.

### 4. Identity → Retention

**StarCraft**: Players identify as "Platinum league Zerg main" or "Diamond Terran."
**OpenAgents**: Users will identify as "Top 1% power user" or "Keyboard-only speedrunner."

This identity creates **stickiness**. Users don't want to lose their rank/status.

---

## Design Challenges & Solutions

### Challenge 1: "Casual users will be overwhelmed"

**Solution**: Progressive disclosure
- **Default**: Simple mode (mouse-driven, basic UI)
- **Hint system**: Show hotkey hints (e.g., "Press S to start" tooltip)
- **Power user toggle**: Settings → "Enable power user mode" (unlocks HUD, hotkeys)
- **Onboarding**: Tutorial teaches basics first, advanced features later

### Challenge 2: "Keyboard shortcuts are hard to learn"

**Solution**: Contextual hints + practice mode
- **? Key**: Press `?` anywhere to show all available shortcuts
- **Tooltip hints**: Hover button → shows hotkey (e.g., "Start (S)")
- **Command palette**: Cmd+Space shows commands + their hotkeys (learn as you search)
- **Practice mode**: Gamified tutorial ("Complete 10 actions using only keyboard")

### Challenge 3: "Not everyone wants to compete"

**Solution**: Make it opt-in
- **Leaderboards**: Opt-in only (users choose to display on leaderboard)
- **Metrics**: Can be hidden (Settings → "Hide performance metrics")
- **Pressure-free**: Default mode doesn't show rankings, just personal stats

### Challenge 4: "Mobile users can't use hotkeys"

**Solution**: Desktop-first, mobile-adapted
- **Desktop**: Full hotkey experience (target audience)
- **Mobile**: Simplified UI (touch-optimized, no hotkeys expected)
- **Tablet**: Hybrid (external keyboard support for hotkeys)

---

## Success Metrics

### Engagement
- [ ] Power users (keyboard usage >80%): 20% of active users
- [ ] Average session time for power users: 45min+ (vs 10min for casual)
- [ ] Hotkey usage: 50%+ of actions via keyboard (across all users)
- [ ] Control group usage: 30% of users assign agents to groups

### Performance
- [ ] Top 10% of users: 50+ jobs/hour (10x casual users)
- [ ] Keyboard-only users: 3x faster than mouse-only users
- [ ] Leaderboard participation: 40% of power users opt-in

### Community
- [ ] User-created "speedrun" videos (YouTube, Twitter)
- [ ] Shared "build order" templates (workflow optimization guides)
- [ ] Community challenges ("Process 100 jobs in 1 hour")
- [ ] "APM challenge" events (who can hit highest jobs/hour?)

---

## Conclusion: Why StarCraft?

**StarCraft isn't just an RTS—it's a masterclass in competitive UI/UX for power users.**

What makes StarCraft's UI compelling:
1. **Efficiency is rewarded** (faster players win)
2. **Measurement is built-in** (APM tracking, match statistics)
3. **Mastery curve is steep** (thousands of hours to reach pro level)
4. **Community is competitive** (leaderboards, tournaments, coaching)
5. **Keyboard > Mouse** (power users learn hotkeys)
6. **Awareness at a glance** (minimap, resource counters, selection panel)

**These same principles make for an excellent power-user interface for OpenAgents.**

The vision: Users don't casually "use AI"—they **compete to maximize their agent throughput**, optimize workflows obsessively, track performance metrics religiously, and share strategies with the community.

**StarCraft players don't complain about complexity—they embrace it. OpenAgents power users will too.**

---

**Last Updated**: 2025-12-08
**Status**: 🚧 Design Vision (Pre-Implementation)
**Next Steps**: Prototype Phase 1 (HUD elements: minimap, resource counters, selection panel)

**References**:
- StarCraft II: https://starcraft2.com
- APM & Performance: https://liquipedia.net/starcraft2/Actions_per_minute
- Hotkey optimization guides (community knowledge)
- External inspiration: VS Code (Cmd+P), Notion (Cmd+K), Figma (hotkey-driven)
