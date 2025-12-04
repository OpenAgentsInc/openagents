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
// Helpers
// ============================================================================

/**
 * Get list of existing files in output directory.
 */
const getExistingFiles = async (cwd: string, outputDir: string): Promise<string[]> => {
  const fullPath = outputDir.startsWith("/") ? outputDir : `${cwd}/${outputDir}`;
  try {
    const entries = await Array.fromAsync(new Bun.Glob("*.md").scan({ cwd: fullPath }));
    return entries.map((f) => `${outputDir}/${f}`);
  } catch {
    // Directory doesn't exist or can't be scanned
    return [];
  }
};

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
    // Get files before running to detect new ones
    const filesBefore = new Set(await getExistingFiles(cwd, outputDir));

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
    let filesCreated = result.filesModified || [];
    let summaryPath = filesCreated.find((f) => f.endsWith("-summary.md"));

    // If no files tracked but operation succeeded, scan for new files
    if (result.success && filesCreated.length === 0) {
      const filesAfter = await getExistingFiles(cwd, outputDir);
      const newFiles = filesAfter.filter((f) => !filesBefore.has(f));
      if (newFiles.length > 0) {
        filesCreated = newFiles;
        summaryPath = newFiles.find((f) => f.endsWith("-summary.md"));
      }
    }

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
// Synthesis Types
// ============================================================================

/**
 * Request to synthesize summaries into analysis doc.
 */
export interface SynthesisRequest {
  /** Paths to paper summary files */
  summaryPaths: string[];
  /** Path to the analysis document to update */
  analysisPath: string;
}

/**
 * Result of a synthesis operation.
 */
export interface SynthesisResult {
  /** Whether the synthesis was successful */
  success: boolean;
  /** Path to the updated analysis document */
  analysisPath?: string;
  /** Summary of changes made */
  changesSummary?: string;
  /** Error message if failed */
  error?: string;
  /** Claude Code session ID for debugging */
  sessionId?: string;
}

// ============================================================================
// Synthesis Function
// ============================================================================

import { buildSynthesisPrompt } from "./prompts.js";

/**
 * Run the synthesizer to update analysis doc with paper summaries.
 */
export const runSynthesizer = async (
  request: SynthesisRequest,
  options: ResearcherOptions = {}
): Promise<SynthesisResult> => {
  const cwd = options.cwd ?? process.cwd();

  // Build the synthesis prompt
  const prompt = buildSynthesisPrompt(request.summaryPaths, request.analysisPath);

  // Create a subtask for Claude Code
  const subtask: Subtask = {
    id: `synthesis-${Date.now()}`,
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
      allowedTools: ["Read", "Write", "Edit", "Glob", "Grep"],
    };
    if (options.onOutput) subagentOptions.onOutput = options.onOutput;
    if (options.signal) subagentOptions.signal = options.signal;

    // Run Claude Code with synthesis task
    const result = await runClaudeCodeSubagent(subtask, subagentOptions);

    // Build result
    const synthesisResult: SynthesisResult = {
      success: result.success,
      analysisPath: request.analysisPath,
    };
    if (result.error) synthesisResult.error = result.error;
    if (result.claudeCodeSessionId) synthesisResult.sessionId = result.claudeCodeSessionId;

    return synthesisResult;
  } catch (error) {
    return {
      success: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
};

// ============================================================================
// Exports
// ============================================================================

export { buildResearchPrompt, buildSynthesisPrompt } from "./prompts.js";
