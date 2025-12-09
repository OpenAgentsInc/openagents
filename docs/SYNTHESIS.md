# OpenAgents Desktop: Unified Product Synthesis

> **Vision:** A single desktop application that combines MechaCoder (autonomous coding agent), Agent Store (built-in marketplace), Terminal-Bench GYM (testing infrastructure), and FM Hill Climber (MAP architecture) into one cohesive product—all built with Bun, Effect, and Effuse.

**Last Updated:** 2025-12-09
**Status:** Synthesis Document (Planning Phase)

---

## Executive Summary

Over the past two years, OpenAgents has built multiple powerful systems in parallel:

1. **MechaCoder** — Autonomous coding agent following the Golden Loop v2 spec
2. **FM Hill Climber** — MAP architecture for solving Terminal-Bench tasks using Apple on-device FM
3. **Terminal-Bench GYM** — Testing and benchmarking infrastructure
4. **Agent Store** — Marketplace for AI agents (from video series)
5. **Effuse** — Effect-native UI framework for the desktop HUD

**The Synthesis:** These systems are not separate products—they are **components of a single unified desktop application** that serves as the complete agent development and deployment platform.

This document synthesizes all components into one coherent product vision and implementation roadmap.

---

## Table of Contents

1. [The Unified Product](#the-unified-product)
2. [Component Overview](#component-overview)
3. [Architecture Integration](#architecture-integration)
4. [User Experience Flow](#user-experience-flow)
5. [Technical Stack](#technical-stack)
6. [Implementation Roadmap](#implementation-roadmap)
7. [Strategic Implications](#strategic-implications)

---

## The Unified Product

### What We're Building

**OpenAgents Desktop** is a single application that provides:

1. **Agent Development Environment**
   - MechaCoder picks tasks, implements code, runs tests, commits
   - FM Hill Climber solves Terminal-Bench tasks using local Apple FM
   - Terminal-Bench GYM provides systematic testing and benchmarking

2. **Agent Marketplace**
   - Agent Store built-in for discovering, installing, and sharing agents
   - Agents can be trained in the GYM and published to the store
   - Users can run agents from the store in their own projects

3. **Unified UI (Effuse)**
   - Single HUD showing MechaCoder progress, GYM training runs, Agent Store listings
   - Real-time visualization of agent activity, Terminal-Bench scores, marketplace stats
   - All components share the same Effect-native architecture

### Why This Matters

**The Strategic Thesis:**

If MechaCoder + FM Hill Climber achieves **#1 on Terminal-Bench using only Apple on-device FM**, it proves:

- **Architecture beats raw model capability** — Local inference + better loops can outperform cloud models
- **OpenAgents becomes the agent runtime standard** — The platform that orchestrates the best agents
- **Enterprise adoption** — Agents running on employee MacBooks, no cloud dependency, no data leakage

The unified desktop app is the **delivery vehicle** for this thesis. It's not just a coding agent—it's the complete platform for agent development, testing, deployment, and commerce.

---

## Component Overview

### 1. MechaCoder (Autonomous Coding Agent)

**Purpose:** Pick tasks from `.openagents/tasks.jsonl`, implement code, run tests, commit & push.

**Key Features:**
- Golden Loop v2: Orient → select task → decompose → implement → verify → commit → update task
- Orchestrator + Coding Subagent architecture (minimal prompts, efficient execution)
- Preflight checklist (init.sh) for environment validation
- Session persistence and resumption
- HUD integration for real-time progress

**Documentation:**
- `docs/mechacoder/README.md` — Overview
- `docs/mechacoder/GOLDEN-LOOP-v2.md` — Complete spec
- `docs/mechacoder/MECHACODER-OPS.md` — Operations guide

**Current State:** ✅ Fully implemented, production-ready

---

### 2. FM Hill Climber (MAP Architecture)

**Purpose:** Solve Terminal-Bench 2 tasks using Apple on-device Foundation Model through sophisticated architecture.

**Key Features:**
- **MAP Architecture** — Modular Agentic Planner (decomposer, monitor, evaluator)
- **Test-Time Compute (TTC)** — Parallel sampling of N candidates, pick best
- **TestGen** — Dynamic generation of comprehensive test suites
- **Iterative Refinement** — Build solutions incrementally with verification feedback
- **Docker Verification** — Isolated pytest execution

**Key Achievement:** 89.5% (17/19 tests) on `regex-log` task using only local FM inference.

**The Three Curves (Validation Framework):**
1. **TestGen score vs evolution step** — Does meta-learning work?
2. **HillClimber pass rate vs TestGen config version** — Does epistemic quality transfer?
3. **TB2 performance vs internal metrics** — Is bootstrapping valid?

**Documentation:**
- `docs/fm-hillclimber.md` — Complete system documentation
- `docs/hillclimber/stakes.md` — Strategic implications of Terminal-Bench #1
- `docs/logs/20251209/1119-comprehensive-daily-summary.md` — Recent development status

**Current State:** ✅ Core architecture implemented, pushing to 100% on regex-log

---

### 3. Terminal-Bench GYM (Testing Infrastructure)

**Purpose:** Systematic evaluation and training of agents in safe, sandboxed environments.

**Key Features:**
- **Gym Environments** — Terminal-Bench, MechaBench, tool microbenchmarks, Healer scenarios
- **Agent Profiles** — Versioned agent configurations (prompts, models, tools)
- **Training Episodes** — Single runs of agent profiles in environments
- **Training Plans** — Structured evaluation suites with objectives
- **Evolution Strategies** — Generate improved agent profiles based on results

**Integration Points:**
- Uses `src/bench/terminal-bench.ts` for TB2 task execution
- Integrates with APM for metrics collection
- Uses worktrees for sandboxed training runs
- HUD shows training progress and results

**Documentation:**
- `docs/subagents/gym-trainer.md` — Trainer subagent & Gym spec
- `docs/tbench/README.md` — Terminal-Bench 2.0 evaluation guide
- `docs/terminal-bench.md` — Terminal-Bench integration overview

**Current State:** ⏳ Spec complete, implementation in progress

---

### 4. Agent Store (Built-in Marketplace)

**Purpose:** Discover, install, and share AI agents within the desktop app.

**Key Features:**
- **Agent Registry** — Browse available agents (coding, testing, deployment, etc.)
- **Installation** — One-click install agents into projects
- **Publishing** — Train agents in GYM, then publish to store
- **Ratings & Reviews** — Community feedback on agent performance
- **Versioning** — Agents have versions, users can upgrade/downgrade

**Integration Points:**
- Agents trained in GYM can be published to store
- MechaCoder can discover and use agents from store
- Store listings show Terminal-Bench scores and GYM metrics
- HUD shows installed agents and their status

**Documentation:**
- `docs/transcripts/` — Video series documenting Agent Store launch (Episodes 092-095)
- Agent Store was launched in November 2023 with first payouts

**Current State:** ⏳ Concept from video series, needs desktop integration

---

### 5. Effuse (UI Framework)

**Purpose:** Effect-native UI framework for the desktop HUD.

**Key Features:**
- **Widget System** — Effect-native components with typed state (`S`), events (`E`), services (`R`)
- **StateCell** — Reactive state primitive (Effect.Ref + Queue)
- **html``** — Tagged template with automatic XSS escaping
- **Services** — DomService, StateService, SocketService (all Effect Context.Tags)
- **Layers** — EffuseLive (production), makeTestLayer() (mock), makeHappyDomLayer() (real DOM)

**Current Widgets:**
- APM Widget — Actions per minute monitoring
- TB Controls — Terminal-Bench task controls
- MC Tasks — MechaCoder task list
- Three Background — Animated agent graph (to be replaced with SVG)

**Documentation:**
- `docs/effuse/README.md` — Quick intro, core concepts, examples
- `docs/effuse/ARCHITECTURE.md` — Deep dive (lifecycle, services, internals)
- `docs/effuse/TESTING.md` — Testing guide (three-layer pyramid)

**Current State:** ✅ Core framework implemented, widgets in progress

---

## Architecture Integration

### How Components Connect

```
┌─────────────────────────────────────────────────────────────────┐
│                    OpenAgents Desktop App                        │
│                    (Bun + Effect + Effuse)                      │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌──────────────────────┼──────────────────────┐
        │                      │                      │
        ▼                      ▼                      ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│  MechaCoder   │    │ FM Hill        │    │ Terminal-Bench │
│  (Orchestrator│    │ Climber        │    │ GYM            │
│   + Subagent) │    │ (MAP)          │    │ (Trainer)      │
└───────────────┘    └───────────────┘    └───────────────┘
        │                      │                      │
        │                      │                      │
        └──────────────────────┼──────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  Agent Store    │
                    │  (Marketplace)  │
                    └─────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │  Effuse HUD     │
                    │  (UI Framework) │
                    └─────────────────┘
```

### Shared Infrastructure

**1. Task System (`.openagents/tasks.jsonl`)**
- MechaCoder reads/writes tasks
- GYM creates training tasks
- Agent Store agents can create tasks
- All components use the same task schema

**2. Project Configuration (`.openagents/project.json`)**
- Defines test commands, default branch, allowPush
- Used by MechaCoder for verification
- Used by GYM for training environments
- Used by Agent Store for agent installation

**3. Effect Services**
- All components use Effect for async/error handling
- Shared services: FileSystem, Path, Process, etc.
- Effuse widgets consume services via Context.Tag

**4. HUD Event System**
- MechaCoder emits events (task_selected, subtask_complete, etc.)
- FM Hill Climber emits events (turn_start, progress_update, etc.)
- GYM emits events (episode_start, training_complete, etc.)
- All events flow to Effuse HUD for visualization

**5. Logging (`docs/logs/YYYYMMDD/`)**
- MechaCoder writes session logs
- FM Hill Climber writes run logs
- GYM writes training logs
- All use the same directory structure and naming convention

---

## User Experience Flow

### Scenario 1: Developer Uses MechaCoder

1. **Open Desktop App**
   - User sees home screen with project list
   - Selects a project (e.g., `~/code/myproject`)

2. **Start MechaCoder**
   - Clicks "Run one task" or "Run overnight"
   - HUD shows:
     - Current task ID/title
     - Live log stream
     - Files being modified
     - Test results

3. **Watch Progress**
   - MechaCoder picks task from `.openagents/tasks.jsonl`
   - Decomposes into subtasks
   - Implements code changes
   - Runs tests (from `project.json.testCommands`)
   - Commits & pushes (if tests pass)
   - Updates task status

4. **Review Results**
   - Git history shows new commits
   - Task marked as `closed` in `tasks.jsonl`
   - Log file in `docs/logs/YYYYMMDD/`

---

### Scenario 2: Developer Trains Agent in GYM

1. **Open GYM Tab**
   - User sees available environments (Terminal-Bench, MechaBench, etc.)
   - Sees agent profiles (baseline, evolved variants)

2. **Create Training Plan**
   - Select environment: "terminal-bench-v2"
   - Select agent profile: "coding-subagent-v3"
   - Select tasks: "regex-log, path-tracing, chess-best-move"
   - Set objectives: "maximize pass rate"

3. **Run Training**
   - GYM creates sandboxed worktrees
   - Runs each task with selected agent profile
   - Collects metrics (pass rate, turns, tokens, time)
   - HUD shows:
     - Current episode
     - Progress per task
     - Overall metrics

4. **Review Results**
   - Training summary shows:
     - Pass rate per task
     - Best performing profile
     - Suggested improvements
   - User can:
     - Publish best profile to Agent Store
     - Create evolved variant for next training run
     - Export results for analysis

---

### Scenario 3: Developer Uses FM Hill Climber

1. **Select Terminal-Bench Task**
   - User opens "Terminal-Bench" tab in HUD
   - Sees list of TB2 tasks
   - Selects "regex-log"

2. **Start Hill Climber**
   - Clicks "Run with FM Hill Climber"
   - System:
     - Generates comprehensive tests (TestGen)
     - Decomposes task into subtasks
     - Runs MAP orchestrator with Apple FM
     - Uses parallel sampling (TTC)
     - Verifies progress in Docker

3. **Watch Progress**
   - HUD shows:
     - Current turn and subtask
     - Progress percentage (e.g., "89.5% - 17/19 tests passing")
     - Monitor warnings
     - Best candidate selection
   - Real-time updates every 30 seconds

4. **Review Results**
   - Final result shows:
     - Pass rate (target: 100%)
     - Regex solution generated
     - Test suite used
     - Time taken
   - User can:
     - Export solution
     - Compare with cloud model results
     - Submit to Terminal-Bench leaderboard

---

### Scenario 4: Developer Browses Agent Store

1. **Open Agent Store Tab**
   - User sees:
     - Featured agents
     - Categories (coding, testing, deployment, etc.)
     - Search bar
     - Filters (by Terminal-Bench score, ratings, etc.)

2. **Browse Agents**
   - Each agent card shows:
     - Name and description
     - Terminal-Bench score (if applicable)
     - GYM metrics
     - User ratings
     - Installation count

3. **Install Agent**
   - User clicks "Install" on an agent
   - System:
     - Downloads agent profile
     - Installs to `.openagents/agents/`
     - Updates `project.json` with agent config
     - Agent appears in "Installed Agents" list

4. **Use Agent**
   - User can:
     - Run agent on a task (via MechaCoder integration)
     - Train agent in GYM
     - View agent metrics in HUD

---

### Scenario 5: Developer Publishes Agent to Store

1. **Train Agent in GYM**
   - User trains agent profile on Terminal-Bench tasks
   - Achieves high pass rate (e.g., 95%+)

2. **Publish to Store**
   - User clicks "Publish to Store" in GYM results
   - System:
     - Validates agent profile
     - Runs smoke tests
     - Creates store listing
     - Uploads to Agent Store

3. **Agent Available in Store**
   - Other users can discover and install
   - Ratings and reviews accumulate
   - Agent appears in search results

---

## Technical Stack

### Core Technologies

**Runtime:**
- **Bun** — JavaScript runtime, package manager, bundler
- **Effect** — Type-safe async/error handling, dependency injection
- **TypeScript** — Type safety, strict mode

**UI:**
- **Effuse** — Effect-native UI framework
- **HTML/CSS** — Standard web technologies
- **SVG** — For agent graph visualization (replacing Three.js)

**Agent Infrastructure:**
- **Apple Foundation Model** — On-device inference (via FM service)
- **Claude Code** — Cloud coding subagent (fallback)
- **Docker** — Sandboxed execution for Terminal-Bench verification

**Data Storage:**
- **SQLite** — For FM Hill Climber runs, GYM training data
- **JSONL** — For tasks (`.openagents/tasks.jsonl`)
- **JSON** — For project config, agent profiles, store listings

**Testing:**
- **Terminal-Bench 2.0** — External benchmark suite
- **pytest** — Test execution in Docker
- **Effect test utilities** — makeTestLayer(), makeHappyDomLayer()

---

## Implementation Roadmap

### Phase 1: Core Integration (Weeks 1-4)

**Goal:** Integrate existing components into unified desktop app.

**Tasks:**

1. **Unify HUD Event System**
   - Create shared event types for all components
   - Implement event router that forwards to Effuse widgets
   - Add event filtering (internal vs. user-visible)

2. **Integrate FM Hill Climber into HUD**
   - Create TB2 task list widget
   - Add "Run with FM Hill Climber" button
   - Show real-time progress (turns, subtasks, test results)
   - Display final results with solution

3. **Integrate GYM into HUD**
   - Create GYM tab with environment selector
   - Add training plan creation UI
   - Show training progress (episodes, metrics)
   - Display training summary with evolution suggestions

4. **Create Unified Project View**
   - Single screen showing:
     - MechaCoder status
     - Active GYM training runs
     - Installed agents from store
     - Terminal-Bench scores

**Deliverable:** Desktop app with all components visible in HUD

---

### Phase 2: Agent Store Integration (Weeks 5-8)

**Goal:** Build Agent Store as first-class component.

**Tasks:**

1. **Implement Agent Store Backend**
   - Agent registry (SQLite or JSON)
   - Agent profile schema (config, metadata, ratings)
   - Installation system (download, install to `.openagents/agents/`)
   - Versioning system

2. **Build Agent Store UI**
   - Browse/search interface
   - Agent detail pages
   - Installation flow
   - Ratings and reviews

3. **Integrate with GYM**
   - "Publish to Store" button in GYM results
   - Validation and smoke tests before publishing
   - Automatic metadata generation (Terminal-Bench scores, GYM metrics)

4. **Integrate with MechaCoder**
   - MechaCoder can discover agents from store
   - Agents can be used as subagents
   - Agent selection UI in MechaCoder config

**Deliverable:** Functional Agent Store with browse, install, publish

---

### Phase 3: Advanced Features (Weeks 9-12)

**Goal:** Polish and advanced capabilities.

**Tasks:**

1. **Agent Graph Visualization**
   - Replace Three.js with SVG-based graph (from Unit framework)
   - Show agent relationships, data flow
   - Interactive: hover, click, pan, zoom

2. **Unified Logging Dashboard**
   - Single view of all logs (MechaCoder, FM Hill Climber, GYM)
   - Search and filtering
   - Export capabilities

3. **Performance Monitoring**
   - APM integration across all components
   - Real-time metrics dashboard
   - Performance alerts

4. **Agent Composition**
   - Visual agent builder (drag-drop agents, connect pins)
   - Agent pipelines (agent A → agent B → agent C)
   - Shared state between agents

**Deliverable:** Polished desktop app with advanced features

---

### Phase 4: Terminal-Bench #1 Push (Weeks 13-16)

**Goal:** Achieve #1 on Terminal-Bench using only Apple FM.

**Tasks:**

1. **Complete FM Hill Climber**
   - Push regex-log to 100%
   - Scale to other TB2 tasks (path-tracing, chess-best-move, etc.)
   - Optimize turn budget and sampling strategy

2. **Validate Three Curves**
   - TestGen score vs evolution step (upward trend?)
   - HillClimber pass rate vs TestGen config (improvement?)
   - TB2 performance vs internal metrics (correlation?)

3. **Submit to Terminal-Bench Leaderboard**
   - Run full TB2 suite
   - Generate official submission
   - Submit to leaderboard

4. **Document Results**
   - Technical blog post
   - Press release
   - Investor narrative

**Deliverable:** #1 on Terminal-Bench leaderboard, validated thesis

---

## Strategic Implications

### If We Succeed (Terminal-Bench #1)

**1. Paradigm Shift Confirmed**
- Architecture beats raw model capability
- Local inference can outperform cloud models
- The "bigger model = better results" assumption is wrong

**2. OpenAgents Becomes the Standard**
- The agent runtime standard (like Node.js for web)
- The orchestrator for the best agents
- The platform enterprises choose

**3. Enterprise Adoption**
- Agents running on employee MacBooks
- No cloud dependency, no data leakage
- Cheaper than cloud AI bills
- Security/DevOps teams champion the solution

**4. Business Model Validation**
- Agent Store becomes the "App Store for skills"
- Marketplace revenue from agent sales
- Enterprise licensing for on-device agents

### The Unified Product Advantage

**Why One Desktop App Matters:**

1. **Developer Experience**
   - One tool for everything (development, testing, deployment, marketplace)
   - No context switching between separate tools
   - Consistent UI and workflows

2. **Integration Depth**
   - Agents trained in GYM can be published to store
   - MechaCoder can use agents from store
   - FM Hill Climber results inform GYM training
   - All components share the same task system

3. **Strategic Moat**
   - Not just a coding agent—it's the complete platform
   - Hard to replicate the integration depth
   - Network effects from Agent Store

4. **Enterprise Sales**
   - One product to sell, not five separate tools
   - Clear value proposition: "Complete agent platform"
   - Easier to price and package

---

## Key Documents Reference

### MechaCoder
- `docs/mechacoder/README.md` — Overview
- `docs/mechacoder/GOLDEN-LOOP-v2.md` — Complete spec
- `docs/mechacoder/MECHACODER-OPS.md` — Operations guide

### FM Hill Climber
- `docs/fm-hillclimber.md` — System documentation
- `docs/hillclimber/stakes.md` — Strategic implications
- `docs/logs/20251209/1119-comprehensive-daily-summary.md` — Recent status

### Terminal-Bench GYM
- `docs/subagents/gym-trainer.md` — Trainer & Gym spec
- `docs/tbench/README.md` — Terminal-Bench evaluation guide
- `docs/terminal-bench.md` — Integration overview

### Agent Store
- `docs/transcripts/README.md` — Video series transcripts (Episodes 092-095)

### Effuse
- `docs/effuse/README.md` — Framework intro
- `docs/effuse/ARCHITECTURE.md` — Deep dive
- `docs/effuse/TESTING.md` — Testing guide

### Planning
- `docs/claude/plans/unit-effuse.md` — Visual language improvements

---

## Next Steps

1. **Review this synthesis** with the team
2. **Prioritize Phase 1 tasks** in `.openagents/tasks.jsonl`
3. **Create implementation tickets** for each phase
4. **Start Phase 1 integration** work
5. **Set Terminal-Bench #1 target date**

---

**Status:** This is a living document. Update as components evolve and integration progresses.

**Questions?** See individual component documentation or create a task in `.openagents/tasks.jsonl`.
