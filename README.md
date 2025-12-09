# Commander

> **Build, Train, Deploy, Earn.**

Commander is a desktop application for building autonomous AI agents that earn bitcoin. Install with one command:

```bash
curl -fsSL https://openagents.com/install | sh
```

## The Vision

Commander transforms AI agent management into a game. You command **MechaCoder**, your autonomous coding agent, training it through the **GYM** to complete coding tasks and earn bitcoin. The better you train your agents, the more they earn. Publish your best agents to the **Agent Store** for passive income while you sleep.

**See the full product vision:** [docs/SYNTHESIS.md](docs/SYNTHESIS.md)

## Core Components

### MechaCoder - Your First Agent

MechaCoder is an autonomous coding agent that picks up tasks, implements code, runs tests, and commits — learning patterns and conventions over time. It follows the [Golden Loop v2 spec](docs/mechacoder/GOLDEN-LOOP-v2.md): select repo, pick ready task, understand, implement, test, commit & push, update task, repeat.

**Documentation:** [docs/mechacoder/](docs/mechacoder/)

### The GYM - Training Ground

Train MechaCoder to excel at specific skills through systematic benchmarking:

- **Terminal-Bench** - The gold-standard benchmark for agent capabilities
- **MechaBench** - Custom challenges for specific skills
- **Training Plans** - Structured evaluation suites with objectives
- **Evolution Strategies** - Generate improved agent profiles based on results

### FM Hill Climber - MAP Architecture

Our system for solving Terminal-Bench tasks using Apple's on-device Foundation Model with MAP (Modular Agentic Planner) architecture, parallel sampling, and iterative verification.

**Key Achievement:** 89.5% on Terminal-Bench `regex-log` using only local FM inference.

**Documentation:** [docs/fm-hillclimber.md](docs/fm-hillclimber.md)

### Agent Store - Marketplace

Train an agent. Publish it. Earn bitcoin every time someone uses it.

- Browse and install agents by category
- View Terminal-Bench scores and GYM metrics
- Revenue sharing paid daily in bitcoin

### The Three Curves

Our thesis — that architecture beats raw model capability — reduces to whether **three graphs slope upward**:

1. **TestGen score vs evolution step** — Does meta-learning work?
2. **HillClimber pass rate vs TestGen config version** — Does epistemic quality transfer?
3. **TB2 performance vs internal metrics** — Is bootstrapping valid?

If all three curves trend upward, we've proven that a well-trained local agent can outperform cloud giants. See [stakes.md](docs/hillclimber/stakes.md) for the full strategic implications.

## Gamification

Commander is gamified with:

- **APM Tracking** - Measure actions per minute (StarCraft-inspired)
- **Trust Tiers** - Bronze → Silver → Gold progression with XP
- **Skill Trees** - Unlockable agent capabilities
- **Achievements** - "First Blood", "Money Printer", "Leaderboard Legend"
- **Leaderboards** - Compete globally by earnings, APM, training completions

## Tech Stack

- **Runtime:** [Bun](https://bun.sh), [Effect](https://effect.website/), TypeScript
- **UI:** [Effuse](docs/effuse/README.md) (Effect-native framework)
- **Agents:** Apple Foundation Model, Claude Code, MCP
- **Payments:** Bitcoin Lightning, Spark

## Quick Start

```bash
# Install Commander
curl -fsSL https://openagents.com/install | sh

# Launch
commander

# Or run MechaCoder directly
bun run mechacoder
```

## Documentation

| Document | Description |
|----------|-------------|
| [docs/SYNTHESIS.md](docs/SYNTHESIS.md) | Full product vision |
| [docs/mechacoder/](docs/mechacoder/) | MechaCoder documentation |
| [docs/fm-hillclimber.md](docs/fm-hillclimber.md) | MAP architecture |
| [docs/effuse/](docs/effuse/) | UI framework |
| [docs/inspiration/starcraft.md](docs/inspiration/starcraft.md) | APM & hotkey design |
| [docs/inspiration/factorio.md](docs/inspiration/factorio.md) | Factory management design |

## Links

- **Website:** [openagents.com](https://openagents.com)
- **GitHub:** [github.com/openagents-inc/openagents](https://github.com/openagents-inc/openagents)
- **Discord:** [discord.openagents.com](https://discord.openagents.com)
