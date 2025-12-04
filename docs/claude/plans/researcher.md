# Researcher Subagent Implementation Plan

## Overview

Automate the research paper discovery and ingestion workflow by creating a Researcher subagent that leverages Claude Code's built-in WebSearch and WebFetch capabilities.

**Current Manual Workflow:**
1. Agent reads reflection doc → requests papers to read
2. User manually downloads PDFs to `docs/local/`
3. User asks Claude Code to summarize → `docs/research/paper-summaries/`
4. Synthesis agents read summaries and update analysis
5. Cycle continues with new paper requests

**Automated Workflow:**
1. Researcher parses reflection docs for paper requests
2. Researcher invokes Claude Code with WebSearch/WebFetch to find/retrieve papers
3. Claude Code generates summaries following the established format
4. Researcher updates analysis docs with completion status

---

## Recommended Approach: Focused Implementation with State Tracking

Build a **focused implementation** (~400 lines) with:
- All CLI commands (search, url, from-reflection, auto)
- `papers.jsonl` registry for tracking processed papers
- Task creation in `.openagents/tasks.jsonl` for paper work

### Rationale

1. **Claude Code does the heavy lifting** - WebSearch, WebFetch, PDF reading, and summary writing are all Claude Code capabilities. We just need good prompts.
2. **State tracking enables batch workflows** - The papers.jsonl registry prevents duplicate work and enables status reporting.
3. **Task integration maintains traceability** - Creating tasks links research work to the standard workflow.

---

## Implementation Plan

### Files to Create

```
src/researcher/
├── index.ts          # runResearcher() + ResearchRequest/Result types
├── cli.ts            # CLI commands: search, url, from-reflection, auto, status
├── parser.ts         # Parse paper requests from reflection markdown
├── prompts.ts        # Research prompt templates
├── registry.ts       # papers.jsonl registry management
└── tasks.ts          # Task creation via TaskService
```

### Papers Registry Schema (`docs/research/papers.jsonl`)

One JSON object per line:
```typescript
interface PaperRecord {
  id: string;                       // e.g., "paper-a-mem-2025"
  title: string;
  authors?: string;
  year?: number;
  urls: string[];                   // arXiv, DOI, etc.
  priority: "HIGH" | "MEDIUM" | "LOW";
  status: "pending" | "processing" | "complete" | "failed";
  sourceDoc: string;                // Where the request came from
  summaryPath?: string;             // Path to generated summary
  taskId?: string;                  // Associated task ID
  createdAt: string;
  updatedAt: string;
  error?: string;                   // If failed, why
}
```

### Phase 1: Core Function (~2 hours)

**`src/researcher/index.ts`**

```typescript
export interface ResearchRequest {
  query: string;                    // Paper title, topic, or URL
  priority?: "high" | "medium" | "low";
  outputDir?: string;               // Default: docs/research/paper-summaries
  urls?: string[];                  // Direct URLs to analyze
}

export interface ResearchResult {
  success: boolean;
  summaryPath?: string;
  filesCreated: string[];
  error?: string;
}

export const runResearcher = async (
  request: ResearchRequest,
  options?: { onOutput?: (text: string) => void; cwd?: string }
): Promise<ResearchResult>
```

Implementation:
1. Build a `Subtask` with a detailed research prompt
2. Call `runClaudeCodeSubagent()` with WebSearch/WebFetch enabled
3. Parse the result for created files
4. Return `ResearchResult`

### Phase 2: CLI (~1 hour)

**`src/researcher/cli.ts`**

```bash
# Search and summarize a paper
bun run researcher:search "A-Mem: Agentic Memory for LLM Agents"

# Process a specific URL
bun run researcher:url "https://arxiv.org/abs/2502.12110"

# Parse reflection doc and process HIGH priority papers
bun run researcher:from-reflection docs/research/analysis/related-work-reflection.md

# Auto mode: read reflection, process up to N papers
bun run researcher:auto --max-papers 3
```

### Phase 3: Reflection Parser (~1 hour)

**`src/researcher/parser.ts`**

Parse paper request tables from markdown:
```markdown
| Paper | Year | URL | Priority |
|-------|------|-----|----------|
| A-Mem: Agentic Memory | 2025 | arxiv.org/abs/... | HIGH |
```

Also detect prose sections like "Papers to Request for Full Read" and "Please acquire PDFs for analysis".

### Phase 4: Registry and Task Integration (~1.5 hours)

**`src/researcher/registry.ts`**
- `loadRegistry()` / `saveRegistry()` - Read/write `docs/research/papers.jsonl`
- `findPaper(query)` - Check if paper already exists
- `addPaper(record)` / `updatePaper(id, updates)` - CRUD operations
- `getPendingPapers()` - Get papers not yet processed

**`src/researcher/tasks.ts`**
- `createResearchTask(paper)` - Create task in `.openagents/tasks.jsonl`
  - Type: `task`
  - Title: `Research: <paper title>`
  - Labels: `["research", "paper"]`
- `closeResearchTask(taskId, summaryPath)` - Close task when summary created

### Phase 5: Prompt Engineering

**`src/researcher/prompts.ts`**

The research prompt template instructs Claude Code to:
1. Use WebSearch to find the paper
2. Use WebFetch to retrieve content (arXiv HTML, conference page)
3. Write a structured summary following the `voyager-summary.md` format
4. Report what was created

Summary format sections:
- Paper title, authors, year, source
- Executive summary (2-3 sentences)
- Key findings (bullet points)
- Core architecture/methodology
- Results/benchmarks
- **Relevance to MechaCoder** (critical - how this applies to our work)
- BibTeX citation

---

## CLI Commands

| Command | Description |
|---------|-------------|
| `researcher:search <query>` | Search for a paper by title/topic and summarize |
| `researcher:url <url>` | Fetch and summarize a specific paper URL |
| `researcher:from-reflection <path>` | Parse reflection doc and register papers (creates tasks) |
| `researcher:auto` | Auto-process HIGH priority pending papers |
| `researcher:status` | Show registry status (pending, complete, failed counts) |
| `researcher:list` | List all papers in registry with status |

---

## Example Usage

```bash
# Process papers requested in the reflection doc
$ bun run researcher:auto --max-papers 3

[12:00:00] Parsing docs/research/analysis/related-work-reflection.md...
[12:00:01] Found 7 paper requests, 4 HIGH priority
[12:00:02] Processing: A-Mem: Agentic Memory for LLM Agents (HIGH)
[12:00:30] ✓ Created: docs/research/paper-summaries/a-mem-summary.md
[12:00:31] Processing: Reflexion: Language Agents with Verbal Reinforcement Learning (HIGH)
[12:01:00] ✓ Created: docs/research/paper-summaries/reflexion-summary.md
[12:01:01] Processing: Odyssey: Empowering Minecraft Agents (HIGH)
[12:01:30] ✓ Created: docs/research/paper-summaries/odyssey-summary.md

Done! Processed 3/4 HIGH priority papers.
```

---

## Future Enhancements (v2+)

If the initial implementation proves valuable, we can add:

1. **Full subagent pattern** - Policy, context, spells if orchestrator integration needed
2. **HUD integration** - Real-time progress visualization (`researcher_scan_start`, etc.)
3. **PDF storage** - Download PDFs to `docs/local/` for archival
4. **Orchestrator integration** - Auto-run researcher during overnight runs
5. **Analysis doc updates** - Automatically mark papers as processed in source docs

---

## Critical Files to Read Before Implementation

1. `src/agent/orchestrator/claude-code-subagent.ts` - `runClaudeCodeSubagent()` function
2. `src/healer/spells/typecheck.ts` - Pattern for building subtask prompts
3. `src/healer/cli.ts` - CLI pattern with parseArgs
4. `src/tasks/service.ts` - TaskService for creating/closing tasks
5. `docs/research/paper-summaries/voyager-summary.md` - Target summary format
6. `docs/research/analysis/related-work-reflection.md` - Paper request format to parse

---

## Estimated Implementation Time

| Phase | Hours |
|-------|-------|
| Phase 1: Core function | 2.0 |
| Phase 2: CLI | 1.0 |
| Phase 3: Reflection parser | 1.0 |
| Phase 4: Registry + tasks | 1.5 |
| Phase 5: Prompt engineering | 0.5 |
| **Total** | **~6 hours** |
