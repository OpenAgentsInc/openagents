# Researcher Subagent Spec

> A knowledge acquisition subagent that automates academic paper discovery, retrieval, and summarization to feed the project's research-driven development process.

---

## 1. Goals & Non-Goals

### Goals

1. **Automated paper discovery and ingestion**

   * Parse analysis documents for paper requests (tables, prose sections)
   * Use Claude Code's WebSearch to find papers by title, author, or topic
   * Use Claude Code's WebFetch to retrieve content from arXiv, conference pages, etc.
   * Generate structured summaries following project conventions

2. **State tracking and deduplication**

   * Maintain a `papers.jsonl` registry to track all paper requests and their status
   * Prevent duplicate processing of already-summarized papers
   * Enable status reporting and batch management

3. **Task integration**

   * Create tasks in `.openagents/tasks.jsonl` for each paper request
   * Close tasks automatically when summaries are successfully created
   * Link research work to the standard MechaCoder workflow

4. **Consistent output format**

   * Generate summaries matching the `voyager-summary.md` template
   * Always include "Relevance to MechaCoder" section with actionable insights
   * Capture citations, source URLs, and related paper references

### Non-Goals (for v1)

* Researcher is **not** a PDF parser - it works with web-accessible content (arXiv HTML, abstracts)
* Researcher does **not** automatically trigger on analysis doc changes (manual/CLI invocation only)
* No full orchestrator integration or HUD visualization yet
* No cross-project research coordination

---

## 2. When Researcher Runs (Triggers)

Unlike Healer (which runs automatically on errors), Researcher is primarily **manually invoked** via CLI.

### 2.1 CLI triggers (Phase 1)

| Command | Description |
|---------|-------------|
| `researcher:search <query>` | Search for a paper by title/topic and summarize |
| `researcher:url <url>` | Fetch and summarize a specific paper URL |
| `researcher:from-reflection <path>` | Parse reflection doc and register papers (creates tasks) |
| `researcher:auto` | Auto-process HIGH priority pending papers |
| `researcher:status` | Show registry status (pending, complete, failed counts) |
| `researcher:list` | List all papers in registry with status |

### 2.2 Batch processing (Phase 1)

```bash
# Process papers requested in the reflection doc
$ bun run researcher:auto --max-papers 3

[12:00:00] Parsing docs/research/analysis/related-work-reflection.md...
[12:00:01] Found 7 paper requests, 4 HIGH priority
[12:00:02] Processing: A-Mem: Agentic Memory for LLM Agents (HIGH)
[12:00:30] Created: docs/research/paper-summaries/a-mem-summary.md
[12:00:31] Processing: Reflexion (HIGH)
[12:01:00] Created: docs/research/paper-summaries/reflexion-summary.md

Done! Processed 2/4 HIGH priority papers.
```

### 2.3 Future triggers (Phase 2+)

* **Analysis doc watcher** - Detect new paper requests and auto-process
* **Orchestrator integration** - Run researcher during overnight runs
* **Scheduled research cycles** - Periodic discovery of new related papers

---

## 3. Inputs Researcher Sees

For each invocation, Researcher receives a **ResearcherContext**:

```ts
interface ResearcherContext {
  projectRoot: string;
  query: string;                    // Paper title, topic, or search query
  priority: "HIGH" | "MEDIUM" | "LOW";
  urls?: string[];                  // Direct URLs if known
  authors?: string;                 // Author names (search hint)
  year?: number;                    // Publication year (search hint)
  sourceDoc?: string;               // Analysis doc that requested this paper
  outputDir: string;                // Default: docs/research/paper-summaries
  registry: PaperRecord[];          // Current state of papers.jsonl
  existingSummaries: string[];      // Already-created summary paths
}
```

Context is built from:
* CLI arguments or parsed reflection doc
* `docs/research/papers.jsonl` registry
* Existing files in `docs/research/paper-summaries/`

---

## 4. Researcher Operations

Instead of "spells" (Healer terminology), Researcher has **operations** - discrete research actions that Claude Code performs.

### 4.1 Operation catalog (v1)

1. **`search_paper`**

   * Goal: Find a paper using WebSearch
   * Input: Query string, optional author/year hints
   * Output: List of candidate URLs (arXiv, conference, GitHub)
   * Impl: Claude Code WebSearch with academic-focused queries

2. **`fetch_content`**

   * Goal: Retrieve paper content from a URL
   * Input: URL (arXiv abs page, conference page, etc.)
   * Output: Paper text, metadata
   * Impl: Claude Code WebFetch on HTML pages
   * Note: PDFs are not directly readable; use HTML abstracts

3. **`generate_summary`**

   * Goal: Create structured summary from paper content
   * Input: Paper content, metadata
   * Output: Markdown summary following template
   * Impl: Claude Code generates summary with required sections

4. **`write_summary`**

   * Goal: Save summary to filesystem
   * Input: Summary content, output path
   * Output: File path
   * Impl: Claude Code Write tool

5. **`update_registry`**

   * Goal: Track paper status in papers.jsonl
   * Input: Paper record updates
   * Output: Updated registry
   * Impl: Local JSONL manipulation

6. **`create_task`** / **`close_task`**

   * Goal: Integrate with task system
   * Input: Paper info / Task ID + summary path
   * Output: Task created/closed
   * Impl: TaskService from `src/tasks/service.ts`

---

## 5. Paper Registry Schema

Location: `docs/research/papers.jsonl`

```ts
interface PaperRecord {
  id: string;                       // e.g., "paper-a-mem-2025"
  title: string;
  authors?: string;
  year?: number;
  urls: string[];                   // arXiv, DOI, conference, etc.
  priority: "HIGH" | "MEDIUM" | "LOW";
  status: "pending" | "processing" | "complete" | "failed";
  sourceDoc: string;                // Analysis doc that requested this
  summaryPath?: string;             // Path to generated summary
  taskId?: string;                  // Associated task ID
  createdAt: string;                // ISO timestamp
  updatedAt: string;                // ISO timestamp
  error?: string;                   // If failed, why
}
```

### Registry operations

```ts
// Load registry
const registry = await loadRegistry("docs/research/papers.jsonl");

// Check if paper exists
const existing = findPaper(registry, "A-Mem");

// Add new paper request
const record = await addPaper(registry, {
  title: "A-Mem: Agentic Memory for LLM Agents",
  priority: "HIGH",
  sourceDoc: "docs/research/analysis/related-work-reflection.md",
});

// Update status
await updatePaper(registry, record.id, {
  status: "complete",
  summaryPath: "docs/research/paper-summaries/a-mem-summary.md"
});
```

---

## 6. Summary Template

All summaries must follow this structure (matching existing summaries like `voyager-summary.md`):

```markdown
# [Paper Title]

**Paper Summary**

- **Authors:** [Full author names]
- **Institutions:** [List of institutions]
- **Published:** [Venue, Year]
- **arXiv:** [arXiv ID if available]
- **Source:** [URL to official page]

---

## Executive Summary

[2-3 sentences summarizing the main contribution]

---

## Core Contribution

[1 paragraph explaining the key novelty]

---

## Architecture / Methodology

[Detailed explanation with ASCII diagrams if helpful]

---

## Key Results

| Metric | Result |
|--------|--------|
| ... | ... |

---

## Limitations and Future Work

[Bullet points]

---

## Relevance to MechaCoder

[CRITICAL SECTION - must be specific and actionable]

- **Applicable patterns:** [What can we use?]
- **Implementation considerations:** [How to adapt?]
- **Potential value:** [What problem does it solve?]

---

## Citation

```bibtex
@article{...}
```
```

---

## 7. Reflection Doc Parser

The parser extracts paper requests from analysis documents like `related-work-reflection.md`.

### 7.1 Table format

```markdown
| Paper | Year | URL | Priority |
|-------|------|-----|----------|
| A-Mem: Agentic Memory for LLM Agents | 2025 | arxiv.org/abs/2502.12110 | HIGH |
| Reflexion | 2023 | arxiv.org/abs/2303.11366 | HIGH |
```

### 7.2 Prose format

```markdown
## Papers to Request for Full Read

Please acquire PDFs for analysis:
- **A-Mem** (HIGH) - Memory architecture for LLM agents
- **Reflexion** (HIGH) - Verbal reinforcement learning
```

### 7.3 Parser output

```ts
interface ParsedPaperRequest {
  title: string;
  year?: number;
  url?: string;
  priority: "HIGH" | "MEDIUM" | "LOW";
  sourceLine?: number;
}

const requests = parseReflectionDoc("docs/research/analysis/related-work-reflection.md");
// Returns: ParsedPaperRequest[]
```

---

## 8. Integration Points

### 8.1 Task System

Each paper request becomes a task:

```ts
// Created task
{
  id: "oa-abc123",
  title: "Research: A-Mem: Agentic Memory for LLM Agents",
  type: "task",
  priority: 2,
  status: "open",
  labels: ["research", "paper"],
  description: "Find and summarize the A-Mem paper...",
}

// Closed when summary created
{
  status: "closed",
  closeReason: "Summary created at docs/research/paper-summaries/a-mem-summary.md",
  commits: ["abc123"],
}
```

### 8.2 Claude Code

Researcher uses Claude Code as its execution engine:

```ts
const result = await runClaudeCodeSubagent(subtask, {
  cwd: projectRoot,
  maxTurns: 100,
  permissionMode: "bypassPermissions",
  allowedTools: ["Read", "Write", "Edit", "Bash", "Glob", "Grep", "WebSearch", "WebFetch"],
});
```

### 8.3 Future: ATIF Integration

Researcher trajectories will be captured with:

```ts
{
  agent: {
    kind: "researcher",
    model: "claude-opus-4",
  },
  context: {
    paper_title: "A-Mem",
    priority: "HIGH",
    source_doc: "related-work-reflection.md",
  },
}
```

---

## 9. Configuration

### 9.1 ProjectConfig extension

```ts
interface ResearcherConfig {
  enabled?: boolean;                    // Default: true
  maxPapersPerSession?: number;         // Default: 5
  defaultPriority?: "HIGH" | "MEDIUM" | "LOW";
  analysisPatterns?: string[];          // Default: ["docs/research/analysis/*.md"]
  summaryOutputDir?: string;            // Default: "docs/research/paper-summaries"
  registryPath?: string;                // Default: "docs/research/papers.jsonl"
  autoProcessPriorities?: string[];     // Default: ["HIGH", "MEDIUM"]
}
```

### 9.2 Example project.json

```json
{
  "researcher": {
    "enabled": true,
    "maxPapersPerSession": 3,
    "autoProcessPriorities": ["HIGH"]
  }
}
```

---

## 10. Module Structure

```
src/researcher/
├── index.ts          # runResearcher() + types
├── cli.ts            # CLI commands
├── parser.ts         # Reflection doc parser
├── prompts.ts        # Research prompt templates
├── registry.ts       # papers.jsonl management
└── tasks.ts          # Task integration
```

---

## 11. Implementation Phases

### Phase R1 - Core (Priority 1)

1. `src/researcher/index.ts` - Core function wrapping Claude Code
2. `src/researcher/prompts.ts` - Research prompt templates
3. `src/researcher/cli.ts` - Basic CLI (search, url commands)
4. Package.json scripts

**Deliverable:** Can search and summarize individual papers via CLI

### Phase R2 - Registry & Parser (Priority 1)

5. `src/researcher/registry.ts` - papers.jsonl CRUD
6. `src/researcher/parser.ts` - Reflection doc parsing
7. CLI commands: from-reflection, status, list

**Deliverable:** Can parse reflection docs and track paper status

### Phase R3 - Task Integration (Priority 1)

8. `src/researcher/tasks.ts` - Task creation/closing
9. CLI command: auto

**Deliverable:** Full workflow with task tracking

### Phase R4 - Polish (Priority 2)

10. Tests
11. Documentation updates
12. Error handling improvements

---

## 12. Example Workflow

```bash
# 1. Parse the reflection doc to discover paper requests
$ bun run researcher:from-reflection docs/research/analysis/related-work-reflection.md
Found 7 papers, registered 5 new (2 already in registry)
Created 5 tasks

# 2. Check status
$ bun run researcher:status
Papers: 7 total, 5 pending, 0 processing, 2 complete, 0 failed

# 3. Process HIGH priority papers
$ bun run researcher:auto --max-papers 3
Processing A-Mem...
  Searching: "A-Mem Agentic Memory LLM Agents 2025"
  Found: arxiv.org/abs/2502.12110
  Fetching content...
  Generating summary...
  Written: docs/research/paper-summaries/a-mem-summary.md
  Task oa-abc123 closed

Processing Reflexion...
  ...

Done! 3/5 pending papers processed.

# 4. Check updated status
$ bun run researcher:status
Papers: 7 total, 2 pending, 0 processing, 5 complete, 0 failed
```

---

## 13. Relationship to Other Subagents

| Subagent | Relationship |
|----------|-------------|
| **Librarian** | Researcher produces summaries; Librarian indexes and retrieves them |
| **Scout** | Scout explores codebases; Researcher explores literature |
| **Archivist** | Archivist captures lessons; Researcher captures external knowledge |
| **Healer** | No direct relationship |

---

## 14. Future Enhancements (v2+)

1. **PDF reading** - When Claude Code gains native PDF support
2. **Citation graph traversal** - Follow references to discover related papers
3. **Auto-update analysis docs** - Mark papers as processed in source docs
4. **HUD integration** - Real-time research progress visualization
5. **Orchestrator integration** - Auto-run during overnight research cycles
6. **Semantic search** - Find papers related to current task context
7. **Cross-project research sharing** - Share summaries across workspaces
