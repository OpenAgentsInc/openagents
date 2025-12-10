# OpenAgents: Commander

> Desktop app for selling spare compute and training AI agents. One command gives you a nostr identity and bitcoin wallet. Start earning.

## What We're Building

**Commander** is the desktop client for a swarm compute network. Users sell their device's idle compute for bitcoin and train agents that use the network.

### The Thesis

Hedge fund analysts say edge AI disrupts cloud AI capex in 3 years. We say it's NOW—because they model ONE device for ONE user. We're building for AGGREGATED compute across MILLIONS of devices with bitcoin incentives.

**Current focus:** Pushing toward 100% on Terminal-Bench `regex-log` using only Apple on-device Foundation Model. If we hit #1 with local inference, we prove architecture beats model size.

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

**Status:** Active development, pushing toward 100% on `regex-log`

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

---

## Git Conventions

**Safety:**
- NEVER `push --force` to main
- NEVER commit unless explicitly asked (exception: MechaCoder autonomous loop)
- NEVER use `-i` flag (interactive not supported)
- NEVER `git stash` or `git checkout` to revert/discard changes you don't understand
- If you see modified files causing build errors, another agent may be working on them - work around it or ask the user

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
- **Rust crates:** All crates in `crates/` use edition 2024

---

## Rust Crates

All Rust crates must use `edition = "2024"` in their `Cargo.toml`.

Key crates:
- `crates/fm-bridge` — HTTP client for Apple Foundation Model
- `crates/testgen` — Test generation and evolution
- `crates/hillclimber` — MAP-based overnight optimization

---

## Rules

- ✅ Store planning docs in `history/` directory
- ❌ Do NOT edit `tsconfig*.json` without asking

---

## CRITICAL: Terminal-Bench Anti-Cheating Policy

**READ THIS BEFORE ANY HILLCLIMBER/TESTGEN WORK**

See `docs/logs/20251208/1219-benchmark-gaming-analysis.md` and `docs/logs/20251209/1454-decomposer-cleanup-no-cheating.md` for full context.

### The Spectrum

```
LEGITIMATE                                                      CHEATING
    |                                                               |
    |   Domain     Process    Test         Expected    Hardcoded   |
    |   Knowledge  Knowledge  Feedback     Output      Solutions   |
    |   (regex     (TDD       (pass/fail)  Leakage     (if/else    |
    |   syntax)    approach)               ("got X,    task==X)    |
    |                                       expected               |
    |                                       Y")                    |
    +---------------------------------------------------------------+
```

### NEVER DO THESE (Cheating)

1. **NEVER hardcode task IDs**: No `if task_id == "regex-log"` or `match task_id { "regex-log" => ... }`
2. **NEVER hardcode solutions**: No "EXAMPLE REGEX (copy this exactly)" or known-working patterns
3. **NEVER parse TB2 test files**: No extracting expected outputs from `test_outputs.py`
4. **NEVER leak specific test cases**: No "TEST CASES: '192.168.1.1 2024-01-15' → captures '2024-01-15'"
5. **NEVER hardcode TB2 paths**: No `/app/regex.txt` defaults based on knowing TB2 structure

### ALWAYS DO THESE (Legitimate)

1. **Data-driven detection is OK**: Detect task type from test DATA keywords, not task IDs
2. **TestGen discovers patterns**: FM learns from TestGen-generated tests, not injected hints
3. **Skills libraries are separate**: If domain knowledge is needed, it comes from skills, not hardcoded hints

### NO INJECTED HINTS

Do NOT inject domain knowledge hints like:
- "Use lookahead (?=...) for conditions"
- "Word boundary \b prevents partial matches"
- "Greedy .* matches as much as possible"

These hints should come from:
1. **TestGen** - generates tests from task description
2. **Skills libraries** - if we have general domain skills
3. **NOT** from hardcoded decomposer hints or task-specific prompts

The FM must DISCOVER techniques through iteration against TestGen tests.

### The Philosophy

The HillClimber architecture proves "architecture beats model size" by:

1. **TestGen** generates tests from task DESCRIPTION (not TB2 tests)
2. **FM** DISCOVERS solutions through iteration against TestGen tests
3. **If TestGen is good**, discovered solutions pass TB2 too
4. **Giving FM the answer defeats the entire purpose**

### Why This Matters

If we hardcode TB2 knowledge:
- We're not proving architecture beats model size
- We're just proving "hardcoding answers passes tests"
- Results won't generalize to TB3 or novel tasks
- The entire thesis is invalidated

**The real test**: Would this system perform equally well on Terminal-Bench 3 with completely different tasks?

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

### CRITICAL: Event Handling Pattern

**NEVER use raw `addEventListener()` for click handlers in Effuse components.** Use `ctx.dom.delegate()` instead.

```typescript
// ❌ WRONG - Events will break after re-render
setupEvents: (ctx) =>
  Effect.gen(function* () {
    const handleClick = (e: Event) => { ... }
    ctx.container.addEventListener("click", handleClick)  // BROKEN!
  })

// ✅ CORRECT - Events survive re-renders
setupEvents: (ctx) =>
  Effect.gen(function* () {
    yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
      const action = (target as HTMLElement).dataset.action
      Effect.runFork(ctx.emit({ type: action }))
    })
  })
```

**Why:** Effuse uses `innerHTML` replacement on re-render. Raw listeners are attached to elements that get destroyed. `ctx.dom.delegate()` attaches to the container (which survives) and uses event bubbling + `closest()` to find targets.

**Data attributes pattern:**
- Use `data-action="actionName"` for click targets
- Use `data-input="inputName"` for form inputs
- Use `data-*` attributes to pass IDs/values (e.g., `data-session-id="${id}"`)

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
