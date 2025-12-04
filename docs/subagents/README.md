# Subagents

MechaCoder orchestrates work through specialized **subagents** - focused agents that handle specific aspects of the development lifecycle. Rather than one monolithic agent, we have a guild of specialists that the orchestrator invokes when their expertise is needed.

## Architecture

```
Orchestrator (MechaCoder)
    │
    ├── Coding Subagents (execute task work)
    │   ├── Claude Code (complex, multi-file changes)
    │   └── Minimal Subagent (simple, focused edits)
    │
    ├── Knowledge Subagents (acquire & organize information)
    │   └── Researcher (paper discovery & summarization)
    │
    └── System Subagents (maintain agent ecosystem)
        ├── Healer (recovery & self-repair)
        ├── Archivist (learning & memory)
        ├── Trainer (evaluation & improvement)
        ├── Advocate (preference & negotiation)
        └── ... (see others.md for more)
```

All subagent trajectories are captured via ATIF and can be linked via `subagent_trajectory_ref` for full traceability.

## Detailed Specs

### [Researcher](./researcher.md)

A knowledge acquisition subagent that automates academic paper discovery, retrieval, and summarization to feed the project's research-driven development process.

**Key features:**
- Parse analysis docs for paper requests (tables, prose sections)
- Search and retrieve papers via Claude Code's WebSearch/WebFetch
- Generate structured summaries following project conventions
- Track paper status in `papers.jsonl` registry
- Integrate with task system for traceability

**Triggers:** CLI commands (`researcher:search`, `researcher:auto`, etc.)

---

### [Healer](./healer.md)

A self-healing subagent that wakes up automatically when agent trajectories go off the rails, diagnoses what went wrong, and tries to repair or safely contain the damage.

**Key features:**
- Automatic recovery on init/verification/subtask failures
- Trajectory-aware diagnosis using ATIF data
- Controlled "spells" for safe operations (rewind, fix typecheck, mark blocked)
- Never makes things worse - respects Golden Loop invariants

**Triggers:** Init failure, verification failure, subtask failure, runtime errors, stuck detection.

---

### [Archivist](./archivist.md)

A reflective memory subagent that runs after important episodes and distills lessons into a structured Memory Bank. Inspired by the "Generative Agents" architecture.

**Key features:**
- Reflects on trajectories, sessions, logs, and APM metrics
- Distills lessons with importance scoring and tagging
- Stores memories scoped by project, tool, provider, or phase
- Feeds memories back into future prompts via retrieval

**Triggers:** End of orchestrator session, after Healer invocation, manual/ad-hoc reflection.

---

### [Trainer (Gym)](./gym-trainer.md)

A training subagent that can pull any agent into a safe Gym environment, run benchmark suites, and evolve them via prompt/config changes and learned policies.

**Key features:**
- Systematic agent improvement via structured evaluation
- Gym environments: Terminal-Bench, MechaBench, tool microbenchmarks
- Sandboxed worktrees/containers for safe experimentation
- Proposes versioned improvements: new prompts, configs, routing rules

**Triggers:** Manual CLI commands, or automatically on regression detection.

---

### [Advocate](./advocate.md)

A fiduciary subagent that represents user and project preferences in negotiations—within the MechaCoder ecosystem today, and eventually with external AI agents and systems at scale. Inspired by the vision of ["Coasean Bargaining at Scale"](../research/coasean-bargaining-at-scale.md).

**Key features:**
- Maintains structured preference profiles with conditional rules
- Negotiates resource allocation and tradeoffs internally
- Forms coalitions with other agents for collective bargaining (v2+)
- Enables externality pricing in multi-stakeholder environments (v3)
- Supports both economic (willingness-to-pay) and democratic (equal-weight) negotiation modes

**Triggers:** Model selection decisions, resource conflicts, budget allocation, external service negotiation.

**Horizons:**
| Version | Focus | Scope |
|---------|-------|-------|
| v1 | Internal preference management | Within MechaCoder |
| v2 | Multi-agent negotiation | External AI systems |
| v3 | Full Coasean advocacy | Ecosystem-wide coordination |

---

### [Others](./others.md)

A comprehensive catalog of 21 additional subagent ideas organized by function:

| Category | Subagents |
|----------|-----------|
| **Knowledge Acquisition** | [Researcher](./researcher.md) (full spec) |
| **Discovery & Understanding** | Scout, Cartographer, Librarian |
| **Planning & Strategy** | Strategist, Tactician |
| **Quality & Tests** | Testsmith, Refactorer, Stylist |
| **Safety & Policy** | Sentinel, Warden, Risk Officer |
| **Cost & Performance** | Quartermaster, Dispatcher, Optimizer |
| **Knowledge & Communication** | Scribe, Teacher |
| **Housekeeping** | Janitor, Steward |
| **Monitoring & Meta** | Watcher, Analyst, Alchemist |
| **Representation & Negotiation** | [Advocate](./advocate.md) (full spec) |

Each subagent has clear triggers, scopes, and integration points. See the document for detailed descriptions of each.

## Integration Points

All subagents integrate with:

- **ATIF** - Trajectories are captured with `agent.kind` identifying the subagent type
- **HUD** - Events broadcast for real-time visualization
- **Memory Bank** - Lessons stored and retrieved across sessions
- **ProjectConfig** - Per-project configuration in `.openagents/project.json`

## Future Work

- Implement remaining Phase 2+ specs
- Add subagent-specific CLI commands
- Build HUD panels for each subagent type
- Cross-session and cross-project learning
