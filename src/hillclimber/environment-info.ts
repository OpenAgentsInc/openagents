/**
 * Environment Information Types
 *
 * Structured types for capturing execution environment context.
 * Used by the test generator to create environment-aware tests.
 *
 * The environment tells us things the task description doesn't:
 * - What tools/languages are available (boundaries)
 * - What tools should NOT exist (anti-cheat)
 * - What files exist and their structure (parameter discovery)
 */

// ============================================================================
// Core Types
// ============================================================================

/**
 * Complete environment information collected from the execution context.
 */
export interface EnvironmentInfo {
  /** Platform identification */
  platform: PlatformInfo;

  /** Available programming languages with versions and packages */
  languages: LanguageEnvironments;

  /** System tools availability */
  tools: ToolsInfo;

  /** File system information */
  files: FilesInfo;

  /** System resources and limits */
  resources: ResourceInfo;

  /** Environment variables (filtered for safety) */
  env: Record<string, string>;

  /** Timestamp when introspection was performed */
  introspectedAt: string;
}

/**
 * Platform identification.
 */
export interface PlatformInfo {
  /** Execution context type */
  type: "docker" | "container" | "local";

  /** Container image name if applicable */
  containerImage?: string;

  /** OS distribution (e.g., "ubuntu", "debian", "alpine") */
  osDistro?: string;

  /** OS version */
  osVersion?: string;
}

// ============================================================================
// Language Environments
// ============================================================================

/**
 * All detected programming language environments.
 */
export interface LanguageEnvironments {
  python?: PythonEnvironment;
  node?: NodeEnvironment;
  ruby?: RubyEnvironment;
  rust?: RustEnvironment;
  go?: GoEnvironment;
  r?: REnvironment;
  java?: JavaEnvironment;
}

/**
 * Python environment details.
 */
export interface PythonEnvironment {
  /** Python version (e.g., "3.11.4") */
  version: string;

  /** Installed packages from pip list --format=freeze */
  packages: PackageInfo[];

  /** Path to python executable */
  executable: string;
}

/**
 * Node.js environment details.
 */
export interface NodeEnvironment {
  /** Node version (e.g., "20.10.0") */
  version: string;

  /** Installed packages from npm list */
  packages: PackageInfo[];

  /** npm version */
  npmVersion?: string;
}

/**
 * Ruby environment details.
 */
export interface RubyEnvironment {
  version: string;
  gems: PackageInfo[];
}

/**
 * Rust environment details.
 */
export interface RustEnvironment {
  version: string;
  cargoVersion?: string;
}

/**
 * Go environment details.
 */
export interface GoEnvironment {
  version: string;
}

/**
 * R environment details.
 */
export interface REnvironment {
  version: string;
  packages: PackageInfo[];
}

/**
 * Java environment details.
 */
export interface JavaEnvironment {
  version: string;
  vendor?: string;
}

/**
 * Generic package information.
 */
export interface PackageInfo {
  name: string;
  version: string;
}

// ============================================================================
// Tools Information
// ============================================================================

/**
 * System tools availability.
 */
export interface ToolsInfo {
  /** Tools confirmed to be available (which <tool> succeeded) */
  available: ToolInfo[];

  /** Tools that SHOULD NOT exist (inferred from task description) */
  prohibited: ProhibitedTool[];

  /** Result of checking prohibited tools */
  prohibitedCheck: Record<string, boolean>;
}

/**
 * Information about an available tool.
 */
export interface ToolInfo {
  name: string;
  path: string;
  version?: string;
}

/**
 * A tool that should NOT be present (anti-cheat).
 */
export interface ProhibitedTool {
  /** Tool name (e.g., "R", "Rscript") */
  name: string;

  /** Why this tool should be prohibited */
  reason: string;

  /** Whether the tool was found (should be false) */
  found: boolean;
}

// ============================================================================
// Files Information
// ============================================================================

/**
 * File system information.
 */
export interface FilesInfo {
  /** Working directory */
  workdir: string;

  /** Directory listing */
  listing: FileEntry[];

  /** Previews of task-relevant files */
  taskFiles: FilePreview[];
}

/**
 * A file system entry.
 */
export interface FileEntry {
  /** File name */
  name: string;

  /** Full path */
  path: string;

  /** Entry type */
  type: "file" | "directory" | "symlink";

  /** Size in bytes */
  size: number;

  /** Permissions string (e.g., "-rw-r--r--") */
  permissions: string;

  /** Last modified timestamp */
  modifiedAt?: string;
}

/**
 * Preview of a file's contents.
 */
export interface FilePreview {
  /** File path */
  path: string;

  /** File extension */
  extension: string;

  /** Total line count in file */
  lineCount: number;

  /** Preview content (first 50 lines or 2KB, whichever is smaller) */
  preview: string;

  /** Detected file type for special handling */
  detectedType?: DetectedFileType;

  /** Extracted structure (e.g., function names, variables) */
  structure?: ExtractedStructure;
}

/**
 * Detected file types for special handling.
 */
export type DetectedFileType =
  | "python_script"
  | "r_script"
  | "stan_model"
  | "json"
  | "csv"
  | "yaml"
  | "toml"
  | "dockerfile"
  | "makefile"
  | "shell_script"
  | "c_source"
  | "cpp_source"
  | "rust_source"
  | "go_source"
  | "unknown";

/**
 * Structure extracted from file contents.
 */
export interface ExtractedStructure {
  /** Variable names found */
  variables?: string[];

  /** Function/method names found */
  functions?: string[];

  /** Class names found */
  classes?: string[];

  /** Import statements */
  imports?: string[];

  /** Column names (for CSV/data files) */
  columns?: string[];

  /** Parameters (for Stan/config files) */
  parameters?: string[];
}

// ============================================================================
// Resources Information
// ============================================================================

/**
 * System resources and limits.
 */
export interface ResourceInfo {
  /** Memory limit in MB */
  memoryLimitMB?: number;

  /** Available memory in MB */
  memoryAvailableMB?: number;

  /** CPU count */
  cpuCount?: number;

  /** Disk space available in MB */
  diskSpaceMB?: number;

  /** Timeout configured for task */
  timeoutSeconds?: number;
}

// ============================================================================
// Test Generation Input
// ============================================================================

/**
 * Complete input for environment-aware test generation.
 */
export interface TestGenerationInput {
  /** Task description from Terminal-Bench */
  taskDescription: string;

  /** Task identifier */
  taskId: string;

  /** Environment information */
  environment: EnvironmentInfo;

  /** Additional hints (optional) */
  hints?: string[];
}

// ============================================================================
// Utility Types
// ============================================================================

/**
 * Error that can occur during environment introspection.
 */
export interface IntrospectionError {
  _tag: "IntrospectionError";
  phase: "platform" | "languages" | "tools" | "files" | "resources" | "env";
  message: string;
  cause?: unknown;
}

/**
 * Create an IntrospectionError.
 */
export const introspectionError = (
  phase: IntrospectionError["phase"],
  message: string,
  cause?: unknown,
): IntrospectionError => ({
  _tag: "IntrospectionError",
  phase,
  message,
  cause,
});

// ============================================================================
// Default/Empty Values
// ============================================================================

/**
 * Create an empty EnvironmentInfo for testing or fallback.
 */
export const emptyEnvironmentInfo = (): EnvironmentInfo => ({
  platform: { type: "local" },
  languages: {},
  tools: { available: [], prohibited: [], prohibitedCheck: {} },
  files: { workdir: "/app", listing: [], taskFiles: [] },
  resources: {},
  env: {},
  introspectedAt: new Date().toISOString(),
});

// ============================================================================
// File Type Detection
// ============================================================================

/**
 * Detect file type from extension and content.
 */
export const detectFileType = (
  path: string,
  content?: string,
): DetectedFileType => {
  const ext = path.split(".").pop()?.toLowerCase();

  // Extension-based detection
  switch (ext) {
    case "py":
      return "python_script";
    case "r":
      return "r_script";
    case "stan":
      return "stan_model";
    case "json":
      return "json";
    case "csv":
      return "csv";
    case "yaml":
    case "yml":
      return "yaml";
    case "toml":
      return "toml";
    case "c":
    case "h":
      return "c_source";
    case "cpp":
    case "cc":
    case "hpp":
      return "cpp_source";
    case "rs":
      return "rust_source";
    case "go":
      return "go_source";
    case "sh":
    case "bash":
      return "shell_script";
  }

  // Name-based detection
  const name = path.split("/").pop()?.toLowerCase();
  if (name === "dockerfile") return "dockerfile";
  if (name === "makefile" || name === "gnumakefile") return "makefile";

  // Content-based detection (if available)
  if (content) {
    if (content.startsWith("#!/usr/bin/env python") || content.startsWith("#!/usr/bin/python")) {
      return "python_script";
    }
    if (content.startsWith("#!/usr/bin/env Rscript") || content.startsWith("#!/usr/bin/Rscript")) {
      return "r_script";
    }
    if (content.startsWith("#!/bin/bash") || content.startsWith("#!/bin/sh")) {
      return "shell_script";
    }
  }

  return "unknown";
};

// ============================================================================
// Prohibited Tool Inference
// ============================================================================

/**
 * Patterns to detect what tools should be prohibited.
 */
export const PROHIBITION_PATTERNS: Array<{
  pattern: RegExp;
  tools: string[];
  reason: string;
}> = [
  {
    pattern: /convert.*r\s+(script|code)?.*to.*python/i,
    tools: ["R", "Rscript", "rstan"],
    reason: "R→Python conversion: R tools should not be used",
  },
  {
    pattern: /convert.*python.*to.*rust/i,
    tools: ["python", "python3"],
    reason: "Python→Rust conversion: Python should not be used for solution",
  },
  {
    pattern: /implement\s+(from\s+)?scratch|without\s+using/i,
    tools: [], // Context-dependent
    reason: "From scratch implementation: no pre-built solutions",
  },
  {
    pattern: /do\s+not\s+use|must\s+not\s+use|cannot\s+use/i,
    tools: [], // Extract from context
    reason: "Explicit prohibition in task description",
  },
  {
    pattern: /write\s+your\s+own|implement\s+yourself/i,
    tools: [], // Context-dependent
    reason: "Manual implementation required",
  },
];

/**
 * Infer prohibited tools from task description.
 */
export const inferProhibitedTools = (description: string): ProhibitedTool[] => {
  const prohibited: ProhibitedTool[] = [];

  for (const { pattern, tools, reason } of PROHIBITION_PATTERNS) {
    if (pattern.test(description)) {
      for (const tool of tools) {
        prohibited.push({
          name: tool,
          reason,
          found: false, // Will be updated by introspector
        });
      }
    }
  }

  return prohibited;
};
