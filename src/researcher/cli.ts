#!/usr/bin/env bun
/**
 * Researcher CLI
 *
 * Commands for discovering and summarizing academic papers.
 *
 * Usage:
 *   bun run researcher:search <query>           - Search for a paper and summarize
 *   bun run researcher:url <url>                - Fetch and summarize a specific URL
 *   bun run researcher:from-reflection <path>   - Parse reflection doc for papers
 *   bun run researcher:auto [options]           - Auto-process pending papers
 *   bun run researcher:status                   - Show registry status
 *   bun run researcher:list                     - List all papers in registry
 */
import { parseArgs } from "util";
import { runResearcher, type ResearchRequest, type PaperPriority } from "./index.js";
import {
  loadRegistry,
  addPaper,
  updatePaper,
  getPendingPapers,
  findPaperByTitle,
  getRegistryStats,
  type PaperRecord,
} from "./registry.js";
import { parseReflectionDoc } from "./parser.js";
import { createResearchTask, closeResearchTask } from "./tasks.js";

// ============================================================================
// CLI Argument Parsing
// ============================================================================

const { values, positionals } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    help: { type: "boolean", short: "h" },
    json: { type: "boolean", short: "j" },
    verbose: { type: "boolean", short: "v" },
    "max-papers": { type: "string", default: "5" },
    priority: { type: "string", default: "HIGH" },
    "output-dir": { type: "string" },
    "registry-path": { type: "string", default: "docs/research/papers.jsonl" },
    "no-task": { type: "boolean" },
  },
  allowPositionals: true,
});

const command = positionals[0] ?? "help";
const commandArg = positionals[1];

// ============================================================================
// Help
// ============================================================================

const printHelp = () => {
  console.log(`
Researcher CLI - Academic paper discovery and summarization

USAGE:
  bun run researcher:search <query>           Search for a paper and summarize
  bun run researcher:url <url>                Fetch and summarize a specific URL
  bun run researcher:from-reflection <path>   Parse reflection doc for papers
  bun run researcher:auto [options]           Auto-process pending papers
  bun run researcher:status                   Show registry status
  bun run researcher:list                     List all papers in registry

OPTIONS:
  --max-papers <N>      Max papers to process in auto mode (default: 5)
  --priority <P>        Priority filter: HIGH, MEDIUM, LOW (default: HIGH)
  --output-dir <dir>    Output directory for summaries
  --registry-path <p>   Path to papers.jsonl (default: docs/research/papers.jsonl)
  --no-task             Don't create tasks for paper requests
  --json, -j            Output as JSON
  --verbose, -v         Show detailed output
  --help, -h            Show this help

EXAMPLES:
  bun run researcher:search "A-Mem: Agentic Memory for LLM Agents"
  bun run researcher:url "https://arxiv.org/abs/2502.12110"
  bun run researcher:from-reflection docs/research/analysis/related-work-reflection.md
  bun run researcher:auto --max-papers 3 --priority HIGH
  bun run researcher:status
`);
};

// ============================================================================
// Utilities
// ============================================================================

const log = (msg: string) => {
  if (!values.json) {
    console.log(`[${new Date().toISOString().slice(11, 19)}] ${msg}`);
  }
};

const outputJson = (data: unknown) => {
  console.log(JSON.stringify(data, null, 2));
};

// ============================================================================
// Search Command
// ============================================================================

const runSearch = async () => {
  if (!commandArg) {
    console.error("Error: Missing search query");
    console.error("Usage: bun run researcher:search <query>");
    process.exit(1);
  }

  const query = commandArg;
  log(`Searching for: ${query}`);

  const request: ResearchRequest = {
    query,
    priority: (values.priority?.toUpperCase() as PaperPriority) ?? "MEDIUM",
  };
  if (values["output-dir"]) request.outputDir = values["output-dir"];

  const researcherOptions: Parameters<typeof runResearcher>[1] = {
    cwd: process.cwd(),
  };
  if (values.verbose) researcherOptions.onOutput = (text) => process.stdout.write(text);

  const result = await runResearcher(request, researcherOptions);

  if (values.json) {
    outputJson(result);
  } else if (result.success) {
    log(`Success! Summary created at: ${result.summaryPath ?? "unknown"}`);
  } else {
    log(`Failed: ${result.error ?? "Unknown error"}`);
    process.exit(1);
  }
};

// ============================================================================
// URL Command
// ============================================================================

const runUrl = async () => {
  if (!commandArg) {
    console.error("Error: Missing URL");
    console.error("Usage: bun run researcher:url <url>");
    process.exit(1);
  }

  const url = commandArg;
  log(`Fetching: ${url}`);

  // Extract paper title from URL for the query
  const query = url.includes("arxiv")
    ? `Paper at ${url}`
    : url;

  const request: ResearchRequest = {
    query,
    urls: [url],
    priority: (values.priority?.toUpperCase() as PaperPriority) ?? "MEDIUM",
  };
  if (values["output-dir"]) request.outputDir = values["output-dir"];

  const researcherOptions: Parameters<typeof runResearcher>[1] = {
    cwd: process.cwd(),
  };
  if (values.verbose) researcherOptions.onOutput = (text) => process.stdout.write(text);

  const result = await runResearcher(request, researcherOptions);

  if (values.json) {
    outputJson(result);
  } else if (result.success) {
    log(`Success! Summary created at: ${result.summaryPath ?? "unknown"}`);
  } else {
    log(`Failed: ${result.error ?? "Unknown error"}`);
    process.exit(1);
  }
};

// ============================================================================
// From-Reflection Command
// ============================================================================

const runFromReflection = async () => {
  if (!commandArg) {
    console.error("Error: Missing reflection document path");
    console.error("Usage: bun run researcher:from-reflection <path>");
    process.exit(1);
  }

  const docPath = commandArg;
  const registryPath = values["registry-path"] ?? "docs/research/papers.jsonl";
  const createTasks = !values["no-task"];

  log(`Parsing: ${docPath}`);

  // Parse the reflection doc
  const requests = await parseReflectionDoc(docPath);
  log(`Found ${requests.length} paper requests`);

  if (requests.length === 0) {
    if (values.json) {
      outputJson({ parsed: 0, registered: 0, skipped: 0 });
    }
    return;
  }

  // Load existing registry
  const registry = await loadRegistry(registryPath);
  let registered = 0;
  let skipped = 0;
  const newRecords: PaperRecord[] = [];

  for (const req of requests) {
    // Check if already in registry
    const existing = findPaperByTitle(registry, req.title);
    if (existing) {
      skipped++;
      continue;
    }

    // Add to registry
    const paperCreate: Parameters<typeof addPaper>[1] = {
      title: req.title,
      urls: req.url ? [req.url] : [],
      priority: req.priority,
      sourceDoc: docPath,
    };
    if (req.year) paperCreate.year = req.year;
    const record = await addPaper(registryPath, paperCreate);

    // Create task if enabled
    if (createTasks) {
      const taskId = await createResearchTask(record);
      if (taskId) {
        await updatePaper(registryPath, record.id, { taskId });
        record.taskId = taskId;
      }
    }

    newRecords.push(record);
    registered++;
  }

  if (values.json) {
    outputJson({
      parsed: requests.length,
      registered,
      skipped,
      records: newRecords,
    });
  } else {
    log(`Registered ${registered} new papers (${skipped} already in registry)`);
    if (createTasks && registered > 0) {
      log(`Created ${registered} tasks`);
    }
  }
};

// ============================================================================
// Auto Command
// ============================================================================

const runAuto = async () => {
  const registryPath = values["registry-path"] ?? "docs/research/papers.jsonl";
  const maxPapers = parseInt(values["max-papers"] ?? "5", 10);
  const priorityFilter = (values.priority?.toUpperCase() as PaperPriority) ?? "HIGH";

  log(`Auto-processing up to ${maxPapers} ${priorityFilter} priority papers`);

  // Get pending papers
  const registry = await loadRegistry(registryPath);
  let pending = getPendingPapers(registry);

  // Filter by priority
  if (priorityFilter !== "LOW") {
    const priorities: PaperPriority[] =
      priorityFilter === "HIGH" ? ["HIGH"] : ["HIGH", "MEDIUM"];
    pending = pending.filter((p) => priorities.includes(p.priority));
  }

  if (pending.length === 0) {
    log("No pending papers to process");
    if (values.json) {
      outputJson({ processed: 0, success: 0, failed: 0 });
    }
    return;
  }

  log(`Found ${pending.length} pending papers`);

  const toProcess = pending.slice(0, maxPapers);
  let successCount = 0;
  let failCount = 0;
  const results: Array<{ paper: string; success: boolean; path?: string; error?: string }> = [];

  for (const paper of toProcess) {
    log(`Processing: ${paper.title} (${paper.priority})`);

    // Mark as processing
    await updatePaper(registryPath, paper.id, { status: "processing" });

    const request: ResearchRequest = {
      query: paper.title,
      urls: paper.urls,
      priority: paper.priority,
    };
    if (paper.authors) request.authors = paper.authors;
    if (paper.year) request.year = paper.year;
    if (values["output-dir"]) request.outputDir = values["output-dir"];

    const researcherOptions: Parameters<typeof runResearcher>[1] = {
      cwd: process.cwd(),
    };
    if (values.verbose) researcherOptions.onOutput = (text) => process.stdout.write(text);

    const result = await runResearcher(request, researcherOptions);

    if (result.success && result.summaryPath) {
      successCount++;
      await updatePaper(registryPath, paper.id, {
        status: "complete",
        summaryPath: result.summaryPath,
      });

      // Close task if exists
      if (paper.taskId) {
        await closeResearchTask(paper.taskId, result.summaryPath);
      }

      log(`Created: ${result.summaryPath}`);
      results.push({ paper: paper.title, success: true, path: result.summaryPath });
    } else {
      failCount++;
      const updateData: Parameters<typeof updatePaper>[2] = { status: "failed" };
      if (result.error) updateData.error = result.error;
      await updatePaper(registryPath, paper.id, updateData);
      log(`Failed: ${result.error ?? "Unknown error"}`);
      const failResult: (typeof results)[number] = { paper: paper.title, success: false };
      if (result.error) failResult.error = result.error;
      results.push(failResult);
    }
  }

  if (values.json) {
    outputJson({ processed: toProcess.length, success: successCount, failed: failCount, results });
  } else {
    log(`Done! ${successCount}/${toProcess.length} papers processed successfully`);
  }
};

// ============================================================================
// Status Command
// ============================================================================

const runStatus = async () => {
  const registryPath = values["registry-path"] ?? "docs/research/papers.jsonl";
  const stats = await getRegistryStats(registryPath);

  if (values.json) {
    outputJson(stats);
  } else {
    console.log("Paper Registry Status");
    console.log("=====================");
    console.log(`Total:      ${stats.total}`);
    console.log(`Pending:    ${stats.pending}`);
    console.log(`Processing: ${stats.processing}`);
    console.log(`Complete:   ${stats.complete}`);
    console.log(`Failed:     ${stats.failed}`);
    console.log("");
    console.log("By Priority:");
    console.log(`  HIGH:   ${stats.byPriority.HIGH ?? 0}`);
    console.log(`  MEDIUM: ${stats.byPriority.MEDIUM ?? 0}`);
    console.log(`  LOW:    ${stats.byPriority.LOW ?? 0}`);
  }
};

// ============================================================================
// List Command
// ============================================================================

const runList = async () => {
  const registryPath = values["registry-path"] ?? "docs/research/papers.jsonl";
  const registry = await loadRegistry(registryPath);

  if (values.json) {
    outputJson(registry);
  } else {
    if (registry.length === 0) {
      console.log("No papers in registry");
      return;
    }

    console.log("Papers in Registry");
    console.log("==================");
    for (const paper of registry) {
      const status = paper.status.toUpperCase().padEnd(10);
      const priority = paper.priority.padEnd(6);
      console.log(`[${status}] [${priority}] ${paper.title}`);
      if (paper.summaryPath) {
        console.log(`           -> ${paper.summaryPath}`);
      }
      if (paper.error) {
        console.log(`           !! ${paper.error}`);
      }
    }
  }
};

// ============================================================================
// Main
// ============================================================================

const main = async () => {
  if (values.help) {
    printHelp();
    process.exit(0);
  }

  switch (command) {
    case "search":
      await runSearch();
      break;

    case "url":
      await runUrl();
      break;

    case "from-reflection":
      await runFromReflection();
      break;

    case "auto":
      await runAuto();
      break;

    case "status":
      await runStatus();
      break;

    case "list":
      await runList();
      break;

    case "help":
    default:
      printHelp();
      break;
  }
};

main().catch((error) => {
  console.error("Error:", error.message);
  if (values.verbose) {
    console.error(error.stack);
  }
  process.exit(1);
});
