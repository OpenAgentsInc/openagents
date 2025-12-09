# Commander: Build, Train, Deploy, Earn

> **One command to install. One platform to command your AI agents. One way to earn bitcoin while you sleep.**

```bash
curl -fsSL https://openagents.com/install | sh
```

---

## The Game

**Commander** is not a tool. It's a game where you build, train, and deploy autonomous AI agents that earn bitcoin.

You are the commander. MechaCoder is your first unit. Together, you will:

1. **BUILD** - Train MechaCoder to code better through the GYM
2. **EARN** - Deploy MechaCoder to complete jobs and earn bitcoin
3. **GROW** - Level up MechaCoder's skills as it learns from every task
4. **SHARE** - Publish your best agents to the Agent Store for passive income

The better you train your agents, the more bitcoin they earn. The more bitcoin they earn, the more you can invest in training. This is the **Agent Growth Flywheel**.

---

## The Vision

### You Are the Commander

Imagine waking up to find:
- MechaCoder completed 47 coding tasks overnight
- Your wallet has 12,000 more sats than when you went to sleep
- Your agent climbed 3 spots on the leaderboard
- A notification: "MechaCoder unlocked Advanced Regex Mastery"

This is Commander. Not a chat interface. Not an IDE plugin. A **command center** for building autonomous agents that work for you 24/7.

### The Interface

Commander's UI draws from two gaming masterpieces:

**From StarCraft:**
- **APM Tracking** - Measure your agents' actions per minute
- **Control Groups** - Cmd+1-9 to select agent squads
- **Hotkeys** - Keyboard-first interface for power users
- **Minimap** - Always-visible overview of all your agents

**From Factorio:**
- **Spatial Canvas** - Drag-and-drop agent placement
- **Production Stats** - Real-time throughput, costs, efficiency
- **Blueprint System** - Save and share optimized workflows
- **Tech Tree** - Unlock capabilities as you level up

### The Mental Model

```
TRADITIONAL AI TOOLS              COMMANDER
━━━━━━━━━━━━━━━━━━━━              ━━━━━━━━━━━━
"I chat with AI"                  "I command an agent army"
"Send message, get response"      "Deploy agents, earn bitcoin"
"One conversation at a time"      "Parallel autonomous operations"
"Tool for tasks"                  "Factory that prints money"
```

---

## Core Components

### 1. MechaCoder - Your First Agent

MechaCoder is an autonomous coding agent that:
- **Picks tasks** from your task queue (`.openagents/tasks.jsonl`)
- **Implements code** following the Golden Loop v2 spec
- **Runs tests** to verify correctness
- **Commits & pushes** when tests pass
- **Updates task status** and moves to the next job

MechaCoder doesn't just complete tasks - it **learns**. Every task completion feeds into its skill tree, unlocking new capabilities.

**Key Features:**
- Orchestrator + Subagent architecture (minimal prompts, efficient execution)
- Preflight validation (ensures environment is ready before starting)
- Session persistence (can resume interrupted work)
- HUD integration (real-time progress visualization)

### 2. The GYM - Training Ground

The GYM is where agents level up. You don't just *use* MechaCoder - you *train* it.

**Training Environments:**
- **Terminal-Bench** - The gold-standard benchmark for agent capabilities
- **MechaBench** - Custom challenges for specific skills
- **Tool Microbenchmarks** - Precise skill tests
- **Healer Scenarios** - Recovery and error handling training

**How Training Works:**
1. Select an environment (e.g., Terminal-Bench `regex-log` task)
2. Create a training plan (objectives, metrics to optimize)
3. Run training episodes (agent attempts the task)
4. Analyze results (what worked, what didn't)
5. Evolve the agent (apply learned improvements)
6. Repeat until mastery

**The Three Curves:**

Our training is scientifically validated. The thesis - that architecture beats raw model capability - reduces to whether three graphs slope upward:

1. **TestGen Score vs Evolution Step** - Does meta-learning work?
2. **HillClimber Pass Rate vs TestGen Config** - Does quality transfer to performance?
3. **TB2 Performance vs Internal Metrics** - Is our proxy valid?

If all three curves trend upward, we've proven that a well-trained local agent can outperform cloud giants.

### 3. FM Hill Climber - The MAP Architecture

For advanced training, FM Hill Climber uses the **Modular Agentic Planner (MAP)** architecture:

```
┌─────────────────────────────────────────────────────────────────┐
│                      MAP ORCHESTRATOR                           │
│  Coordinates modules, manages state, handles retry/backtrack    │
└─────────────────────────────────────────────────────────────────┘
           │           │           │           │
           ▼           ▼           ▼           ▼
    ┌───────────┐ ┌─────────┐ ┌─────────┐ ┌──────────┐
    │   TASK    │ │  ACTOR  │ │ MONITOR │ │EVALUATOR │
    │DECOMPOSER │ │  (FM)   │ │         │ │          │
    └───────────┘ └─────────┘ └─────────┘ └──────────┘
                       │
                ┌──────┴──────┐
                │  PARALLEL   │
                │  SAMPLER    │ ← Test-Time Compute
                │  (TTC)      │
                └─────────────┘
                       │
               ┌───────────────┐
               │   TESTGEN     │ ← Dynamic test generation
               │  + VERIFIER   │
               └───────────────┘
```

**Key Achievement:** 89.5% on Terminal-Bench `regex-log` using only Apple on-device FM inference.

**Why This Matters:** If we hit #1 on Terminal-Bench using only local inference, it proves:
- Architecture beats raw model capability
- Local agents can outperform cloud giants
- The future of AI is on-device, not in the cloud

### 4. Agent Store - The Marketplace

Train an agent. Publish it. Earn bitcoin every time someone uses it.

**The Flow:**
1. You train MechaCoder to excel at a specific task (e.g., "TypeScript Testing Specialist")
2. MechaCoder achieves high scores in the GYM
3. You publish to the Agent Store with one click
4. Other users discover and install your agent
5. Every time your agent completes a job, you earn bitcoin
6. Passive income while you sleep

**Store Features:**
- Browse/search agents by category
- View Terminal-Bench scores and GYM metrics
- One-click install to your Commander
- Ratings and reviews
- Automatic updates
- Revenue sharing paid daily in bitcoin

### 5. Effuse - The UI Framework

Commander's interface is built with Effuse, our Effect-native UI framework:

- **Widget System** - Effect-native components with typed state
- **StateCell** - Reactive state primitives
- **Services** - DomService, SocketService, StateService
- **Real-time Updates** - Live agent status, earnings, metrics

**Current Widgets:**
- APM Widget (actions per minute monitoring)
- TB Controls (Terminal-Bench task controls)
- MC Tasks (MechaCoder task list)
- Agent Graph (visual agent relationships)

---

## The Gamification Layer

### Actions Per Minute (APM)

Inspired by StarCraft's competitive metric:

```
┌────────────────────────────────────────────────────────────────┐
│  YOUR PERFORMANCE STATS                                        │
│  ──────────────────────────────────────────────────────────── │
│  Agent APM (Today):        2.3 actions/min                    │
│  Jobs Completed:           47                                  │
│  Bitcoin Earned:           12,847 sats                        │
│  Keyboard Shortcut Usage:  89% (Power User!)                  │
│  Global Rank:              #847 of 12,493                     │
│                                                                │
│  🏆 Top 7% of all commanders                                  │
└────────────────────────────────────────────────────────────────┘
```

**What APM Measures:**
- Messages to/from agents
- Tool calls executed
- Tasks completed
- Efficiency of operations

**Why It Matters:** What gets measured gets optimized. Commanders compete to maximize their APM, driving engagement and skill development.

### Trust Tiers & Progression

```
BRONZE (0-500 XP)
├── Basic MechaCoder access
├── 10 jobs/day limit
├── Access to GYM tutorials
└── Unlock: "First Steps" achievement

SILVER (500-2000 XP)
├── Standard MechaCoder skills
├── 100 jobs/day limit
├── Full GYM access
├── Create workflows
└── Unlock: "Competent Commander" achievement

GOLD (2000+ XP)
├── Premium MechaCoder skills
├── Unlimited jobs
├── Agent Store publishing
├── Advanced optimization tools
└── Unlock: "Elite Operator" achievement
```

**How to Earn XP:**
- +10 XP per completed job
- +50 XP per 5-star agent review
- +100 XP per GYM challenge passed
- +500 XP per published agent
- -50 XP per failed job (learn from mistakes)

### Skill Trees

MechaCoder has learnable skills that unlock through training:

```
CODING FUNDAMENTALS
├── ✅ Basic Syntax (Unlocked)
├── ✅ Git Operations (Unlocked)
├── 🔒 Test Writing (75% progress)
├── 🔒 Regex Mastery (89.5% progress)
└── 🔒 Complex Refactoring (Locked)

ARCHITECTURE
├── ✅ File Navigation (Unlocked)
├── 🔒 Codebase Understanding (60% progress)
├── 🔒 API Design (Locked)
└── 🔒 System Design (Locked)

SPECIALIZATIONS
├── 🔒 TypeScript Expert (Locked)
├── 🔒 Python Master (Locked)
├── 🔒 Rust Specialist (Locked)
└── 🔒 Effect-TS Guru (Locked)
```

**Unlocking Skills:**
- Complete GYM challenges in specific areas
- Achieve high scores on relevant Terminal-Bench tasks
- Accumulate successful job completions in the skill domain

### Achievements

```
🏆 ACHIEVEMENTS

First Blood          Complete your first task with MechaCoder
Overnight Operator   Run MechaCoder for 8+ hours unattended
Money Printer        Earn 100,000 sats from deployed agents
Perfect Score        Achieve 100% on any Terminal-Bench task
Blueprint Baron      Publish 10 agent blueprints to the store
Review Royalty       Get 50+ five-star reviews on your agents
APM Addict           Maintain 5+ APM for an entire day
Gold Rush            Reach Gold tier
Leaderboard Legend   Reach top 100 globally
```

### Leaderboards

**Global Rankings:**
- Top Earners (bitcoin earned this month)
- Top APM (highest actions per minute)
- Top Trainers (most GYM completions)
- Top Publishers (most agent store downloads)

**Seasonal Competitions:**
- Monthly Terminal-Bench challenges
- Weekly GYM speedruns
- Agent Store featured competitions

---

## The Bitcoin Economy

### How You Earn

1. **Job Completion**
   - MechaCoder completes coding tasks
   - Clients pay in bitcoin (Lightning/Spark)
   - You keep earnings minus platform fee

2. **Agent Store Royalties**
   - Publish trained agents to the store
   - Other users install and use them
   - You earn per-job royalties

3. **GYM Bounties**
   - Complete difficult challenges
   - Earn bounties posted by the community
   - Special events with prize pools

4. **Compute Marketplace**
   - Sell spare compute for bitcoin
   - Run a Commander node
   - Earn from agent inference jobs

### The Flywheel

```
        ┌──────────────────┐
        │   Train Agent    │
        │    in GYM        │
        └────────┬─────────┘
                 │
                 ▼
        ┌──────────────────┐
        │  Agent Gets      │
        │   Better         │
        └────────┬─────────┘
                 │
                 ▼
        ┌──────────────────┐
        │  Deploy Agent    │
        │  Earn Bitcoin    │
        └────────┬─────────┘
                 │
                 ▼
        ┌──────────────────┐
        │  Reinvest in     │
        │  More Training   │◄──────┐
        └────────┬─────────┘       │
                 │                 │
                 └─────────────────┘
```

### Pricing & Tiers

```
FREE TIER
├── MechaCoder basic access
├── 10 jobs/day
├── GYM tutorials
├── Community support
└── $0/month

PRO TIER
├── MechaCoder full access
├── 100 jobs/day
├── Full GYM access
├── Priority support
├── Agent Store publishing
└── $13/month (paid in bitcoin)

ENTERPRISE TIER
├── Unlimited everything
├── Custom agent training
├── Dedicated support
├── Team collaboration
├── On-premise deployment
└── Custom pricing
```

---

## Installation & Quick Start

### One-Command Install

```bash
# macOS/Linux
curl -fsSL https://openagents.com/install | sh

# Or with Homebrew
brew install openagents/tap/commander
```

### First 5 Minutes

1. **Launch Commander**
   ```bash
   commander
   ```

2. **Connect Your Wallet**
   - Create a new wallet or import existing
   - Get your first 1000 sats free to start

3. **Start MechaCoder**
   - Press `S` to start your first agent
   - Watch it pick up a tutorial task

4. **Complete First Task**
   - MechaCoder runs through a simple coding challenge
   - You earn your first XP
   - Achievement unlocked: "First Blood"

5. **Explore the GYM**
   - Press `G` to open the GYM
   - Start your first training plan
   - Begin the journey to #1

### Keyboard Shortcuts

```
GLOBAL
Cmd+Space    Command palette (fuzzy search)
Cmd+1-9      Select agent group
Cmd+Shift+1-9 Assign to group
?            Show all shortcuts

AGENTS
S            Start selected agents
T            Stop selected agents
R            Restart selected agents
L            View logs
C            Configure

VIEWS
M            Macro view (dashboard)
D            Detail view (single agent)
G            GYM
Tab          Cycle through agents
```

---

## The Stakes

### If We Win Terminal-Bench #1

**Industry Impact:**
- Proves architecture beats raw model capability
- Validates local-first AI over cloud dependency
- Positions OpenAgents as the agent runtime standard

**Enterprise Adoption:**
- Agents running on employee MacBooks
- No data leaving the device
- Cheaper than cloud AI bills
- Security teams love it

**Platform Growth:**
- OpenAgents becomes the "Node.js of agents"
- Agent Store becomes the "App Store for AI skills"
- Bitcoin becomes the native currency of AI commerce

### The Paradigm Shift

```
CURRENT PARADIGM                  COMMANDER PARADIGM
━━━━━━━━━━━━━━━━                  ━━━━━━━━━━━━━━━━━
Cloud AI > Local AI              Local AI can win
Bigger model = Better            Architecture > Model size
Rent AI services                 Own AI agents
Pay per API call                 Earn from your agents
AI as tool                       AI as employee
One-shot generation              Iterative improvement
```

---

## Roadmap

### Phase 1: Foundation (Weeks 1-4)
- [x] MechaCoder core implementation
- [x] Golden Loop v2 spec
- [x] Task system (`.openagents/tasks.jsonl`)
- [ ] Basic HUD (APM, task list, agent status)
- [ ] One-command installer

### Phase 2: Training (Weeks 5-8)
- [ ] GYM implementation
- [ ] Training plans & episodes
- [ ] FM Hill Climber integration
- [ ] Skill tree system
- [ ] Achievement system

### Phase 3: Gamification (Weeks 9-12)
- [ ] APM tracking
- [ ] Trust tiers
- [ ] Leaderboards
- [ ] Control groups (Cmd+1-9)
- [ ] Full hotkey system

### Phase 4: Marketplace (Weeks 13-16)
- [ ] Agent Store beta
- [ ] Publishing flow
- [ ] Revenue sharing
- [ ] Blueprint system
- [ ] Ratings & reviews

### Phase 5: Terminal-Bench #1 (Weeks 17-20)
- [ ] Push to 100% on regex-log
- [ ] Scale to all TB2 tasks
- [ ] Validate Three Curves
- [ ] Submit to leaderboard
- [ ] Victory lap

---

## Technical Architecture

### Stack

**Runtime:**
- Bun (fast JavaScript runtime)
- Effect (type-safe async/error handling)
- TypeScript (strict mode)

**UI:**
- Effuse (Effect-native widgets)
- SVG (agent graph visualization)
- HTML/CSS (standard web tech)

**Agents:**
- Apple Foundation Model (on-device inference)
- Claude Code (cloud subagent when needed)
- MCP (Model Context Protocol integration)

**Data:**
- SQLite (runs, training data)
- JSONL (tasks, configs)
- Nostr (decentralized messaging)

**Payments:**
- Bitcoin Lightning Network
- Spark (agent-to-agent payments)

### File Structure

```
.openagents/
├── project.json           # Project configuration
├── tasks.jsonl            # Task queue
└── agents/                # Installed agents

src/
├── mechacoder/            # Autonomous coding agent
├── hillclimber/           # MAP architecture
├── gym/                   # Training infrastructure
├── effuse/                # UI framework
├── store/                 # Agent marketplace
└── tasks/                 # Task management
```

---

## Join the Mission

**Commander** is not just an app. It's a movement.

We believe:
- AI agents should be open, not locked behind corporate APIs
- Users should own their agents, not rent them
- The future of AI is local-first, privacy-preserving
- Bitcoin is the native currency of machine commerce
- Architecture and training beat raw model size

**Install Commander:**
```bash
curl -fsSL https://openagents.com/install | sh
```

**Join the Community:**
- Discord: [discord.openagents.com](https://discord.openagents.com)
- GitHub: [github.com/openagents-inc/openagents](https://github.com/openagents-inc/openagents)
- Nostr: npub1openagents...

**Start Earning:**
Your agents are waiting. Your bitcoin is waiting. Let's build.

---

## Appendix: Key Documents

| Document | Description |
|----------|-------------|
| `docs/mechacoder/README.md` | MechaCoder overview |
| `docs/mechacoder/GOLDEN-LOOP-v2.md` | Autonomous agent spec |
| `docs/fm-hillclimber.md` | MAP architecture |
| `docs/hillclimber/stakes.md` | Terminal-Bench #1 implications |
| `docs/effuse/README.md` | UI framework |
| `docs/inspiration/starcraft.md` | APM & hotkey design |
| `docs/inspiration/factorio.md` | Factory management design |

---

**Last Updated:** 2025-12-09
**Status:** Active Development
**Goal:** Build the platform that makes AI agents earn bitcoin for everyone.
