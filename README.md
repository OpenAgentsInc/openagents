# OpenAgents

Your agent command center. (wip)

## Tech stack

- [Effect](https://effect.website/)
- [Effuse](docs/effuse/README.md)

## MechaCoder

MechaCoder is our autonomous coding agent that picks up tasks, implements code, runs tests, and commits — learning patterns and conventions over time. It follows the Golden Loop v2 spec: select repo, pick ready task, understand, implement, test, commit & push, update task, repeat. See [docs/mechacoder/README.md](docs/mechacoder/README.md) for full documentation.

## Project FM Hill Climber

FM Hill Climber is our system for solving Terminal-Bench 2 tasks using Apple's on-device Foundation Model with MAP (Modular Agentic Planner) architecture, parallel sampling, and iterative verification. The goal is to achieve #1 on Terminal-Bench using only local inference. See [docs/fm-hillclimber.md](docs/fm-hillclimber.md) for full documentation. For current development status, see the [December 9 comprehensive summary](docs/logs/20251209/1119-comprehensive-daily-summary.md).
