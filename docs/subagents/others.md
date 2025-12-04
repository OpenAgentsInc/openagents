Love this question. Let’s turn MechaCoder into a little guild of specialists. I’ll use “fantasy ops team” names like Healer/Archivist, but you can rename any of these.

I’ll group them loosely, but all are meant to be **subagents** that the orchestrator can call with clear triggers + scopes.

---

## Discovery & Understanding

**1. Scout (Repo Explorer)**
Scout is the “first-contact” subagent for a repo or new project. It runs a quick but structured exploration: identifies main entrypoints, test harnesses, build scripts, frameworks in use, and risky areas (`infra/`, `migrations/`, `payments/` etc.). It writes a concise `SCOUT.md` or structured JSON summary (and ATIF trajectory) that other agents can use to skip the “where am I?” phase. It’s especially useful at the start of an overnight run or when a new repo is added to .openagents. Over time, Scout can learn to recognize common repo archetypes and tag them for Archivist.

**2. Cartographer (Flow & Topology Mapper)**
Cartographer builds and maintains **structural maps**: code dependency graphs, test dependency graphs, and “golden paths” for common operations (build → test → deploy). It integrates directly with the HUD/flow engine you already have, turning static maps into live overlays (e.g., “this subtask touched these nodes”). Cartographer might run periodically or after large refactors to update the internal topology so other agents (Planner, Healer, Testsmith) reason with accurate structural context.

**3. Librarian (Code & Docs Indexer)**
Librarian is in charge of **searchability**. It maintains indexes (and later embeddings) over code, docs, ADRs, run logs, and AgentMemory entries. When other subagents ask “where do we define X?” or “show me all flaky test tickets,” Librarian surfaces the right snippets and files. It can also periodically detect “orphaned knowledge” (e.g., README that doesn’t match code) and hand off follow-up tasks. In v1, it can just be smart grep + structured tags; later you can back it with vector search.

---

## Planning & Strategy

**4. Strategist (Project-Level Planner)**
Strategist works above individual tasks: given the current task backlog, project config, and Memory Bank, it proposes **medium-term plans** (“for the next 1–3 days of MechaCoder runs, prioritize X, sequence Y, avoid Z until infra is fixed”). It doesn’t execute code; instead it writes planning artifacts: roadmaps, dependency chains, recommended `priority` changes on tasks. Orchestrator can consult Strategist before picking tasks for overnight runs, especially when many tasks are ready.

**5. Tactician (Deep Subtask Decomposer)**
You already have basic subtask decomposition; Tactician is the “max difficulty” version for gnarly tasks. When a task is too large or ambiguous, Tactician reads the task, Scout’s repo summary, relevant memories, and ATIF histories, then produces a very detailed subtask DAG with risk annotations (“this subtask is likely to touch complex test infra; schedule it in sandbox, use minimal subagent”). Tactician could be used only when `priority=0` or tasks tagged as “epic”.

---

## Quality, Tests & Refactors

**6. Testsmith (Test Designer & Stabilizer)**
Testsmith is obsessed with tests. When verification fails or coverage is low, Testsmith designs **better tests**, hunts flaky tests, and proposes refactors to make tests more reliable and faster. It might create/modify test files, add smoke tests, or split a huge slow test into targeted ones. It uses ATIF + APM (slowest commands, most frequent failures) and contributes memories like “tests under `foo/bar` are flaky when run in parallel; run them serially”.

**7. Refactorer (Code Health Specialist)**
Refactorer focuses on long-term maintainability rather than feature work. It periodically scans for code smells (duplication, giant files, tangled dependencies) and proposes/refactors small, safe chunks that future agents benefit from. It might have stricter constraints than normal coding agents: no behavior changes allowed, only structural improvements validated by tests. Archivist learns which refactors lead to fewer future Healer invocations.

**8. Stylist (Lints, Style, Conventions)**
Stylist enforces **style and conventions**: lint rules, formatting, naming, best practices per repo. It runs when lint tools fail, when new frameworks are introduced, or on a schedule to keep drift down. Stylist can also synthesize “Style Guides” from repeated patterns and add them as high-importance memories: “In this repo, prefer X over Y; here’s why.” That then feeds back into MechaCoder’s prompts so future edits match the house style.

---

## Safety, Policy & Risk

**9. Sentinel (Safety & Security Guard)**
Sentinel’s job is to keep agents from doing dangerous or non-compliant things. It watches trajectories for high-risk actions: destructive bash commands, secrets exposure, editing critical infra files without tests, etc. When it detects a risk, it can block execution, inject warnings, or route to Healer/Archivist to document the incident. Sentinel also enforces project-level policies (“never touch `prod/`, never call external URLs in this repo”), and can maintain a memory bank of previously dangerous patterns.

**10. Warden (Sandbox & Environment Protector)**
Warden manages containers/worktrees/sandboxes. It ensures that potentially dangerous work only happens in isolation: verifying container health, cleaning up broken worktrees, and validating that `project.sandbox.enabled` is respected. If sandbox startup fails, Warden can either heal the environment or mark tasks blocked with clear instructions. It’s particularly important now that you have container + worktree infrastructure; Warden can be the dedicated brain for that subsystem.

**11. Risk Officer (Policy & Compliance Advisor)**
Risk Officer is more “legal/ops flavored”: it interprets project-defined policies (e.g., HIPAA / PII rules for TIARA, or internal data handling constraints) and surfaces them at planning time. Before certain tasks or tools are used, Risk Officer retrieves relevant policy memories and adds constraints: “Do not include real user data in test fixtures” or “avoid sending proprietary content to provider X”. It might also generate periodic risk reports based on ATIF logs.

---

## Cost, Performance & Scheduling

**12. Quartermaster (Cost & Token Budget Manager)**
Quartermaster uses APM + usage metrics to control **spend and efficiency**. It decides when to use expensive vs cheap models, when to throttle parallelism, and when to cut off a run that’s burning tokens with low productivity. It also writes “budget memories”: “This project routinely exceeds budget when running full test suite; prefer `bun test --filter` unless releasing.” Orchestrator queries Quartermaster before launching big operations or multiple parallel agents.

**13. Dispatcher (Task & Worktree Scheduler)**
Dispatcher sits between the task system and the parallel worktree runner. It decides **which tasks go to which agent/worktree**, balancing complexity, size, and risk. It can use memories like “Agent 3 previously had issues with this part of the repo” or “tests in this folder are slow, schedule them early”. Without touching code, Dispatcher optimizes throughput by smart assignment and sequencing.

**14. Optimizer (Performance Tuner)**
Optimizer focuses on runtime performance of the code itself (not just tests). When APM + episodes show repeated slow commands or long-running operations, Optimizer gets subtasks like “speed up this integration test suite” or “optimize this hot path”. It reads profiling output (or simple timing logs), tries small targeted optimizations, and records what works. Over time, Optimizer + Archivist can build a playbook of “performance recipes” for common frameworks.

---

## Knowledge, Explanation & Communication

**15. Scribe (Documentation & Narrative)**
Scribe owns human-facing explanations. After a run, Scribe turns technical ATIF data into readable change logs, PR descriptions, ADRs, and status updates. It can also summarize complex trajectories for you (“Here’s what MechaCoder did last night, in English”). Scribe is the natural consumer of Archivist’s memories: it turns “lessons” into explicit docs, and vice versa.

**16. Teacher (Local Tutor / Onboarding Agent)**
Teacher’s focus is explaining the repo and the agents **to humans**. It uses Scout/Cartographer/Librarian outputs to answer “Why did the agent do X?”, “How does this module work?”, or “What’s the best way to add feature Y here?”. It might maintain guided tutorials or “playbooks” for common operations, updated over time from experiences. Helpful both for you and for new collaborators who want to understand what MechaCoder already learned.

---

## Housekeeping & Lifecycle

**17. Janitor (Cleanup & Hygiene)**
Janitor keeps the ecosystem tidy: rotates run logs, archives old ATIF files, compacts tasks and memories, cleans up stale worktrees/branches, and prunes obsolete configs. It runs on explicit CLI commands or on safe schedules. Janitor ensures that the data surfaces Archivist, Librarian, and others rely on don’t grow indefinitely or get corrupted, tying into your tasks:archive/compact plans.

**18. Steward (Backlog Curator)**
Steward is a project-management flavored subagent that **grooms the task system**. It identifies stale tasks, merges duplicates, proposes priority changes, adds missing dependencies, and creates follow-up tasks based on Healer/Archivist findings. Steward doesn’t fix code; it keeps the task universe coherent so orchestrator + Dispatcher can operate on a clean backlog.

---

## Monitoring, Meta & Cross-Cutting

**19. Watcher (Stuck-Run & Anomaly Monitor)**
Watcher is your “ops SRE” for the agent system itself. It monitors runs for anomalies: stuck runs, unusual error rates, sudden drops in completion success, or weird cost patterns. When something looks off, Watcher can trigger Healer, Archivist, or Sentinel, and open tasks documenting the anomaly. Think of it as the first responder for **meta-level** problems in the MechaCoder ecosystem.

**20. Analyst (Metrics & Insight Generator)**
Analyst sits on top of APM + ATIF + Memory Bank and answers: *“What’s actually working?”* It builds comparative reports like “MechaCoder vs direct Claude Code efficiency,” “success rate by provider/model,” or “which tool causes the most failures”. Analyst doesn’t change behavior directly but informs Strategist/Quartermaster/Steward. It might periodically publish dashboard-style summaries into docs or the HUD.

**21. Alchemist (Experiment Runner)**
Alchemist is an R&D subagent that runs controlled experiments: A/B testing prompts, tools, or provider configs. It generates experiment plans (“Try minimal vs CC on these 10 tasks”), runs them, and reports results to Archivist/Analyst as new memories. You could gate Alchemist behind explicit flags so it only runs when you want to explore new strategies without affecting production flows.

---

If any of these feel especially juicy (my guesses: **Librarian, Strategist, Quartermaster, Sentinel, Steward, Watcher**), tell me which ones and I’ll do full specs like Healer/Archivist—including triggers, data surfaces, config, and suggested .openagents tasks.
