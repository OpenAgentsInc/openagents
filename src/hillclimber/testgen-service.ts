/**
 * Test Generation Service
 *
 * Wraps environment-aware test generation with streaming HUD message emission.
 * Used by the desktop handler to run test generation with real-time updates.
 */

import { Effect } from "effect";
import { BunContext } from "@effect/platform-bun";
import { loadTerminalBenchSuite, type TerminalBenchTask } from "../bench/terminal-bench.js";
import { generateTestsFromEnvironment } from "./test-generator.js";
import { emptyEnvironmentInfo, inferProhibitedTools, detectFileType } from "./environment-info.js";
import type { EnvironmentInfo, FilePreview } from "./environment-info.js";
import type { GeneratedTest } from "./test-generator.js";
import type {
  TestGenStartMessage,
  TestGenTestMessage,
  TestGenCompleteMessage,
  TestGenErrorMessage,
} from "../hud/protocol.js";

// ============================================================================
// Types
// ============================================================================

/**
 * Callback interface for emitting HUD messages during test generation.
 */
export interface TestGenEmitter {
  /** Called when test generation starts */
  onStart: (msg: TestGenStartMessage) => void;

  /** Called for each test generated (streamed one at a time) */
  onTest: (msg: TestGenTestMessage) => void;

  /** Called when test generation completes successfully */
  onComplete: (msg: TestGenCompleteMessage) => void;

  /** Called if test generation fails */
  onError: (msg: TestGenErrorMessage) => void;
}

/**
 * Options for test generation.
 */
export interface TestGenOptions {
  /** Model to use for generation */
  model: "local" | "claude";

  /** Path to TB2 task directory (for environment building) */
  tb2Path?: string;
}

// ============================================================================
// Main Service
// ============================================================================

/**
 * Run test generation with streaming HUD messages.
 *
 * @param suitePath - Path to TB suite JSON (e.g., "tasks/terminal-bench-2.json")
 * @param taskId - Specific task ID, or undefined to pick random
 * @param sessionId - Unique session ID for correlating messages
 * @param emitter - Callback interface for HUD messages
 * @param options - Generation options
 */
export async function runTestGenWithStreaming(
  suitePath: string,
  taskId: string | undefined,
  sessionId: string,
  emitter: TestGenEmitter,
  options: TestGenOptions,
): Promise<void> {
  const startTime = Date.now();

  try {
    // Load TB suite
    const suite = await Effect.runPromise(
      loadTerminalBenchSuite(suitePath).pipe(Effect.provide(BunContext.layer))
    );

    // Pick task (random or specified)
    let task: TerminalBenchTask;
    if (taskId) {
      const found = suite.tasks.find((t) => t.id === taskId);
      if (!found) {
        throw new Error(`Task not found: ${taskId}`);
      }
      task = found;
    } else {
      const randomIndex = Math.floor(Math.random() * suite.tasks.length);
      task = suite.tasks[randomIndex];
    }

    // Build mock environment from task
    const tb2Path = options.tb2Path ?? "/Users/christopherdavid/code/terminal-bench-2";
    const env = await buildMockEnvironmentFromTask(task.id, task.description, tb2Path);

    // Emit start message
    const prohibitedTools = env.tools.prohibited.map(t => t.name);
    const languages: string[] = [];
    if (env.languages.python) languages.push(`Python ${env.languages.python.version}`);
    if (env.languages.node) languages.push(`Node ${env.languages.node.version}`);
    if (env.languages.rust) languages.push(`Rust ${env.languages.rust.version}`);
    if (env.languages.go) languages.push(`Go ${env.languages.go.version}`);
    if (env.languages.r) languages.push(`R ${env.languages.r.version}`);
    if (env.languages.ruby) languages.push(`Ruby ${env.languages.ruby.version}`);
    if (env.languages.java) languages.push(`Java ${env.languages.java.version}`);

    emitter.onStart({
      type: "testgen_start",
      sessionId,
      taskId: task.id,
      taskDescription: task.description,
      environment: {
        platform: env.platform.type,
        prohibitedTools,
        languages,
        fileCount: env.files.listing.length,
        filePreviews: env.files.taskFiles.length,
      },
    });

    // Generate tests
    const result = await generateTestsFromEnvironment(
      task.description,
      task.id,
      env,
      {
        model: options.model,
        verbose: false,
      }
    );

    // Emit tests one at a time (by category)
    const emitTestsForCategory = (tests: GeneratedTest[], category: string) => {
      for (const test of tests) {
        emitter.onTest({
          type: "testgen_test",
          sessionId,
          test: {
            id: test.id,
            category,
            input: test.input,
            expectedOutput: test.expectedOutput ?? null,
            reasoning: test.reasoning,
            confidence: test.confidence,
          },
        });
      }
    };

    emitTestsForCategory(result.antiCheatTests, "anti_cheat");
    emitTestsForCategory(result.existenceTests, "existence");
    emitTestsForCategory(result.correctnessTests, "correctness");
    emitTestsForCategory(result.boundaryTests, "boundary");
    emitTestsForCategory(result.integrationTests, "integration");

    // Calculate total tests
    const totalTests =
      result.antiCheatTests.length +
      result.existenceTests.length +
      result.correctnessTests.length +
      result.boundaryTests.length +
      result.integrationTests.length;

    // Emit complete message
    const durationMs = Date.now() - startTime;
    emitter.onComplete({
      type: "testgen_complete",
      sessionId,
      totalTests,
      durationMs,
      uncertainties: result.uncertainties,
    });
  } catch (error) {
    // Emit error message
    emitter.onError({
      type: "testgen_error",
      sessionId,
      error: error instanceof Error ? error.message : String(error),
    });
  }
}

// ============================================================================
// Environment Building
// ============================================================================

/**
 * Build a mock environment from the task folder in TB2.
 * Based on buildMockEnvironmentFromTask from test-gen-compare.ts.
 */
async function buildMockEnvironmentFromTask(
  taskId: string,
  description: string,
  tb2Path: string,
): Promise<EnvironmentInfo> {
  const fs = await import("node:fs");
  const path = await import("node:path");

  const taskDir = `${tb2Path}/${taskId}`;
  const env = emptyEnvironmentInfo();
  env.platform = { type: "docker" };

  // Infer prohibited tools from description
  env.tools.prohibited = inferProhibitedTools(description);
  env.tools.prohibitedCheck = {};
  for (const tool of env.tools.prohibited) {
    env.tools.prohibitedCheck[tool.name] = false; // Assume not found (correct for conversion tasks)
  }

  // Look for common files in task directory
  if (fs.existsSync(taskDir)) {
    const files = fs.readdirSync(taskDir);
    env.files.workdir = "/app";
    env.files.listing = files.slice(0, 20).map((name) => {
      const fullPath = path.join(taskDir, name);
      const stats = fs.statSync(fullPath);
      return {
        name,
        path: `/app/${name}`,
        type: stats.isDirectory() ? ("directory" as const) : ("file" as const),
        size: stats.size,
        permissions: "-rw-r--r--",
      };
    });

    // Get file previews for source files
    const previewExtensions = [".py", ".r", ".R", ".stan", ".c", ".rs", ".go", ".java", ".js", ".ts"];
    const sourceFiles = files
      .filter((f) => previewExtensions.some((ext) => f.endsWith(ext)))
      .slice(0, 5);

    env.files.taskFiles = [];
    for (const file of sourceFiles) {
      const filePath = path.join(taskDir, file);
      try {
        const content = fs.readFileSync(filePath, "utf-8");
        const lines = content.split("\n");
        const preview = lines.slice(0, 50).join("\n");
        const extension = file.split(".").pop() || "";

        const filePreview: FilePreview = {
          path: `/app/${file}`,
          extension,
          lineCount: lines.length,
          preview,
          detectedType: detectFileType(file, preview),
        };

        // Extract structure from preview
        const structure = extractStructureFromContent(preview, filePreview.detectedType);
        if (structure) filePreview.structure = structure;

        env.files.taskFiles.push(filePreview);
      } catch {
        // Skip files that can't be read
      }
    }
  }

  // Set some common tools based on description
  if (description.toLowerCase().includes("python")) {
    env.languages.python = { version: "3.11.0", packages: [], executable: "/usr/bin/python3" };
  }
  if (description.toLowerCase().includes("rust")) {
    env.languages.rust = { version: "1.75.0" };
  }
  if (description.toLowerCase().includes("node") || description.toLowerCase().includes("javascript")) {
    env.languages.node = { version: "20.10.0", packages: [] };
  }

  return env;
}

/**
 * Extract structure from file content.
 */
function extractStructureFromContent(
  content: string,
  detectedType: string | undefined,
): { variables?: string[]; functions?: string[]; parameters?: string[] } | undefined {
  const result: { variables?: string[]; functions?: string[]; parameters?: string[] } = {};

  // Python/R variables
  const varMatches = content.match(/^(\w+)\s*=\s*/gm);
  if (varMatches) {
    result.variables = varMatches
      .map((m) => m.split("=")[0].trim())
      .filter((v) => !v.startsWith("_"));
  }

  // Python/JS functions
  const funcMatches = content.match(/def\s+(\w+)|function\s+(\w+)|fn\s+(\w+)/g);
  if (funcMatches) {
    result.functions = funcMatches.map((m) => m.split(/\s+/)[1]).filter(Boolean);
  }

  // Stan parameters
  if (detectedType === "stan_model") {
    const paramBlock = content.match(/parameters\s*\{([^}]+)\}/);
    if (paramBlock) {
      const params = paramBlock[1].match(/\b(\w+)\s*[;\n]/g);
      if (params) {
        result.parameters = params.map((p) => p.trim().replace(/[;\n]/g, ""));
      }
    }
  }

  return Object.keys(result).length > 0 ? result : undefined;
}
