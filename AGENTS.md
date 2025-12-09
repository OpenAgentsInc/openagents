# OpenAgents: Commander

> Desktop app for selling spare compute and training AI agents. One command gives you a nostr identity and bitcoin wallet. Start earning.

## What We're Building

**Commander** is the desktop client for a swarm compute network. Users sell their device's idle compute for bitcoin and train agents that use the network.

### The Thesis

Hedge fund analysts say edge AI disrupts cloud AI capex in 3 years. We say it's NOW—because they model ONE device for ONE user. We're building for AGGREGATED compute across MILLIONS of devices with bitcoin incentives.

**Current proof:** 89.5% on Terminal-Bench `regex-log` using only Apple on-device Foundation Model. If we hit #1 with local inference, we prove architecture beats model size.

### Core Components

| Component | What It Does |
|-----------|-------------|
| **Swarm Compute** | Sell idle compute, earn bitcoin |
| **Bitcoin Wallet** | Built-in, self-custodial, Lightning/Spark |
| **Nostr Identity** | Keypair on first run, no signup |
| **MechaCoder** | Autonomous coding agent |
| **GYM** | Train agents on Terminal-Bench |
| **Agent Store** | Publish agents that use swarm compute |

### The Loop

```
SELL COMPUTE → EARN BITCOIN → TRAIN AGENTS → PUBLISH → AGENTS USE COMPUTE → REPEAT
```

---

## Key Documentation

Read these to understand the project:

### Product & Vision
| Doc | Description |
|-----|-------------|
| [docs/SYNTHESIS.md](docs/SYNTHESIS.md) | Full product vision, economics, architecture |
| [docs/commander/README.md](docs/commander/README.md) | Product decisions (distribution, identity, wallet, compute) |
| [docs/hillclimber/analysis-dec9.md](docs/hillclimber/analysis-dec9.md) | Edge AI thesis, why we're faster than analysts think |
| [docs/hillclimber/stakes.md](docs/hillclimber/stakes.md) | What winning Terminal-Bench #1 means |

### Technical
| Doc | Description |
|-----|-------------|
| [docs/fm-hillclimber.md](docs/fm-hillclimber.md) | MAP architecture, Three Curves validation framework |
| [docs/mechacoder/README.md](docs/mechacoder/) | Autonomous coding agent |
| [docs/mechacoder/GOLDEN-LOOP-v2.md](docs/mechacoder/GOLDEN-LOOP-v2.md) | Agent execution spec |
| [docs/effuse/README.md](docs/effuse/) | Effect-native UI framework |

### Design Inspiration
| Doc | Description |
|-----|-------------|
| [docs/inspiration/starcraft.md](docs/inspiration/starcraft.md) | APM tracking, hotkeys, control groups |
| [docs/inspiration/factorio.md](docs/inspiration/factorio.md) | Factory management, production stats |

### History
| Doc | Description |
|-----|-------------|
| [docs/transcripts/README.md](docs/transcripts/README.md) | 198 episodes of video series context |

---

## Current Focus: Terminal-Bench #1

We're pushing to 100% on Terminal-Bench using only local Apple FM inference.

**Status:** 89.5% (17/19 tests) on `regex-log`

**The Three Curves** (our validation framework):
1. TestGen Score vs Evolution Step — Does meta-learning work?
2. HillClimber Pass Rate vs TestGen Config — Does quality transfer?
3. TB2 Performance vs Internal Metrics — Is our proxy valid?

If all three slope upward, we've proven architecture beats model size. See [docs/fm-hillclimber.md](docs/fm-hillclimber.md).

---

## Agent Startup Checklist

Before making code changes:

1. **Read core docs:**
   - This file (AGENTS.md)
   - [docs/SYNTHESIS.md](docs/SYNTHESIS.md) — Product vision
   - [docs/mechacoder/README.md](docs/mechacoder/) — Agent overview
   - [docs/mechacoder/GOLDEN-LOOP-v2.md](docs/mechacoder/GOLDEN-LOOP-v2.md) — Execution spec

2. **If working on UI:**
   - [docs/effuse/README.md](docs/effuse/) — Required
   - [docs/effuse/ARCHITECTURE.md](docs/effuse/ARCHITECTURE.md) — If modifying framework

3. **Inspect task config:**
   - `.openagents/project.json` — Project settings
   - `.openagents/tasks.jsonl` — Task queue

---

## Task System

This repo uses `.openagents/` for task tracking:

```bash
.openagents/project.json   # Project config
.openagents/tasks.jsonl    # Task queue (NEVER edit manually)
```

### Task CLI

```bash
bun run tasks:list --json      # List all
bun run tasks:ready --json     # Ready tasks (no blockers)
bun run tasks:next --json      # Claim next task
bun run tasks:create --title "..." --type bug --priority 1 --json
bun run tasks:close --id oa-xxx --reason "Done" --json
```

### Workflow

1. Check ready work: `bun run tasks:ready --json`
2. Claim task: `bun run tasks:next --json`
3. Implement, test, document
4. Close: `bun run tasks:close --id <id> --reason "..."`
5. Commit `.openagents/tasks.jsonl` with code changes

---

## Git Conventions

**Safety:**
- NEVER `push --force` to main
- NEVER commit unless explicitly asked (exception: MechaCoder autonomous loop)
- NEVER use `-i` flag (interactive not supported)

**Commit format:**
```bash
git commit -m "$(cat <<'EOF'
Your message here.

🤖 Generated with [OpenAgents](https://openagents.com)

Co-Authored-By: MechaCoder <noreply@openagents.com>
EOF
)"
```

---

## Tech Stack

- **Runtime:** Bun + Effect + TypeScript
- **UI:** Effuse (Effect-native widgets)
- **Agents:** Apple FM (local), Claude Code (cloud fallback)
- **Payments:** Bitcoin Lightning, Spark
- **Data:** SQLite, JSONL, Nostr

---

## Rules

- ✅ Use `.openagents/tasks.jsonl` for task tracking
- ✅ Link discovered work with `discovered-from` deps
- ✅ Store planning docs in `history/` directory
- ❌ Do NOT manually edit `tasks.jsonl`
- ❌ Do NOT edit `tsconfig*.json` without asking
- ❌ Do NOT use `bd` or `.beads/` (deprecated)

---

## MechaCoder

Autonomous coding agent. Picks tasks, implements, tests, commits.

```bash
bun run mechacoder                    # Single agent
bun run mechacoder:parallel --max-agents 4 --cc-only  # Parallel overnight
```

See [docs/mechacoder/](docs/mechacoder/) for full docs.

---

## Effuse UI Framework

Effect-native widgets for Commander's HUD.

**Key concepts:**
- `Widget` — Component with typed state, events, service requirements
- `StateCell<A>` — Reactive state primitive
- `html\`\`` — Tagged template with XSS escaping

See [docs/effuse/](docs/effuse/) for full docs.

---

## Effect TypeScript Patterns

```typescript
// ✅ Provide layer to effect
Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)))

// ✅ Modern Effect.gen (no adapter)
Effect.gen(function* () {
  const fs = yield* FileSystem.FileSystem;
  return yield* fs.readFile(path);
})

// ✅ Map platform errors
const content = yield* fs.readFileString(path).pipe(
  Effect.mapError((e) => new ToolExecutionError("command_failed", e.message)),
);
```

Use `effect-solutions list` for more patterns.
