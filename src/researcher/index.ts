/**
 * Researcher Subagent
 *
 * Automates research paper discovery and ingestion using Claude Code's
 * WebSearch and WebFetch capabilities.
 *
 * Usage:
 *   import { runResearcher } from "./researcher/index.js";
 *   const result = await runResearcher({ query: "A-Mem: Agentic Memory" });
 */
import { runClaudeCodeSubagent } from "../agent/orchestrator/claude-code-subagent.js";
import type { Subtask } from "../agent/orchestrator/types.js";
import { buildResearchPrompt } from "./prompts.js";

// ============================================================================
// Types
// ============================================================================

export type PaperPriority = "HIGH" | "MEDIUM" | "LOW";
export type PaperStatus = "pending" | "processing" | "complete" | "failed";

/**
 * Request to research a paper.
 */
export interface ResearchRequest {
  /** Paper title, topic, or search query */
  query: string;
  /** Priority level for batch processing */
  priority?: PaperPriority;
  /** Output directory for summary (default: docs/research/paper-summaries) */
  outputDir?: string;
  /** Direct URLs to analyze (arXiv, DOI, etc.) */
  urls?: string[];
  /** Authors (optional, helps with search) */
  authors?: string;
  /** Publication year (optional, helps with search) */
  year?: number;
}

/**
 * Result of a research operation.
 */
export interface ResearchResult {
  /** Whether the research was successful */
  success: boolean;
  /** Path to the generated summary file */
  summaryPath?: string;
  /** All files created during research */
  filesCreated: string[];
  /** Error message if failed */
  error?: string;
  /** Claude Code session ID for debugging */
  sessionId?: string;
}

/**
 * Options for running the researcher.
 */
export interface ResearcherOptions {
  /** Callback for streaming output */
  onOutput?: (text: string) => void;
  /** Working directory */
  cwd?: string;
  /** Maximum turns for Claude Code */
  maxTurns?: number;
  /** Abort signal for cancellation */
  signal?: AbortSignal;
}

// ============================================================================
// Core Function
// ============================================================================

/**
 * Run the researcher to find and summarize a paper.
 *
 * This function builds a research prompt and invokes Claude Code with
 * WebSearch and WebFetch capabilities to find, retrieve, and summarize
 * academic papers.
 */
export const runResearcher = async (
  request: ResearchRequest,
  options: ResearcherOptions = {}
): Promise<ResearchResult> => {
  const cwd = options.cwd ?? process.cwd();
  const outputDir = request.outputDir ?? "docs/research/paper-summaries";

  // Build the research prompt
  const prompt = buildResearchPrompt(request, outputDir);

  // Create a subtask for Claude Code
  const subtask: Subtask = {
    id: `research-${Date.now()}`,
    description: prompt,
    status: "in_progress",
    startedAt: new Date().toISOString(),
    failureCount: 0,
  };

  try {
    // Build options for Claude Code subagent
    const subagentOptions: Parameters<typeof runClaudeCodeSubagent>[1] = {
      cwd,
      maxTurns: options.maxTurns ?? 100,
      permissionMode: "bypassPermissions",
      // Allow WebSearch and WebFetch for research
      allowedTools: [
        "Read",
        "Write",
        "Edit",
        "Bash",
        "Glob",
        "Grep",
        "WebSearch",
        "WebFetch",
      ],
    };
    // Only add optional properties if defined
    if (options.onOutput) subagentOptions.onOutput = options.onOutput;
    if (options.signal) subagentOptions.signal = options.signal;

    // Run Claude Code with research capabilities
    const result = await runClaudeCodeSubagent(subtask, subagentOptions);

    // Extract created files from the result
    const filesCreated = result.filesModified || [];
    const summaryPath = filesCreated.find((f) => f.endsWith("-summary.md"));

    // Build result with only defined properties
    const researchResult: ResearchResult = {
      success: result.success,
      filesCreated,
    };
    if (summaryPath) researchResult.summaryPath = summaryPath;
    if (result.error) researchResult.error = result.error;
    if (result.claudeCodeSessionId) researchResult.sessionId = result.claudeCodeSessionId;

    return researchResult;
  } catch (error) {
    return {
      success: false,
      filesCreated: [],
      error: error instanceof Error ? error.message : String(error),
    };
  }
};

// ============================================================================
// Exports
// ============================================================================

export { buildResearchPrompt } from "./prompts.js";
