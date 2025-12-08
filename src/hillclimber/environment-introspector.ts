/**
 * Environment Introspector
 *
 * Collects environment information from inside a container.
 * Runs during task setup phase to gather:
 * - Available languages and packages
 * - System tools (available and prohibited)
 * - File system structure and previews
 * - System resources and limits
 *
 * This information is used by the test generator to create
 * environment-aware tests that catch both correctness and gaming.
 */

import { Effect } from "effect";
import type {
  EnvironmentInfo,
  PlatformInfo,
  LanguageEnvironments,
  ToolsInfo,
  FilesInfo,
  ResourceInfo,
  FileEntry,
  FilePreview,
  PackageInfo,
  IntrospectionError,
  DetectedFileType,
  ExtractedStructure,
} from "./environment-info.js";
import {
  introspectionError,
  inferProhibitedTools,
  detectFileType,
} from "./environment-info.js";

// ============================================================================
// Command Execution Interface
// ============================================================================

/**
 * Interface for executing commands in the environment.
 * Can be implemented for containers, local execution, etc.
 */
export interface CommandExecutor {
  /**
   * Execute a command and return stdout.
   * Returns empty string if command fails.
   */
  exec(command: string): Effect.Effect<string, IntrospectionError>;

  /**
   * Execute a command and return exit code.
   */
  execWithCode(command: string): Effect.Effect<{ stdout: string; exitCode: number }, IntrospectionError>;
}

// ============================================================================
// Local Command Executor
// ============================================================================

/**
 * Execute commands locally using Bun.spawn.
 */
export const localCommandExecutor: CommandExecutor = {
  exec: (command: string) =>
    Effect.tryPromise({
      try: async () => {
        const proc = Bun.spawn(["sh", "-c", command], {
          stdout: "pipe",
          stderr: "pipe",
        });
        const stdout = await new Response(proc.stdout).text();
        await proc.exited;
        return stdout.trim();
      },
      catch: (e) =>
        introspectionError("platform", `Failed to execute: ${command}`, e),
    }),

  execWithCode: (command: string) =>
    Effect.tryPromise({
      try: async () => {
        const proc = Bun.spawn(["sh", "-c", command], {
          stdout: "pipe",
          stderr: "pipe",
        });
        const stdout = await new Response(proc.stdout).text();
        const exitCode = await proc.exited;
        return { stdout: stdout.trim(), exitCode };
      },
      catch: (e) =>
        introspectionError("platform", `Failed to execute: ${command}`, e),
    }),
};

// ============================================================================
// Main Introspection Function
// ============================================================================

/**
 * Introspect the execution environment.
 *
 * @param executor Command executor (container or local)
 * @param workspace Working directory
 * @param taskDescription Task description for inferring prohibited tools
 */
export const introspectEnvironment = (
  executor: CommandExecutor,
  workspace: string,
  taskDescription: string,
): Effect.Effect<EnvironmentInfo, IntrospectionError> =>
  Effect.gen(function* () {
    // Run all introspection phases
    const platform = yield* introspectPlatform(executor);
    const languages = yield* introspectLanguages(executor);
    const tools = yield* introspectTools(executor, taskDescription);
    const files = yield* introspectFiles(executor, workspace, taskDescription);
    const resources = yield* introspectResources(executor);
    const env = yield* introspectEnv(executor);

    return {
      platform,
      languages,
      tools,
      files,
      resources,
      env,
      introspectedAt: new Date().toISOString(),
    };
  });

/**
 * Introspect with local command execution.
 */
export const introspectLocalEnvironment = (
  workspace: string,
  taskDescription: string,
): Effect.Effect<EnvironmentInfo, IntrospectionError> =>
  introspectEnvironment(localCommandExecutor, workspace, taskDescription);

// ============================================================================
// Platform Detection
// ============================================================================

const introspectPlatform = (
  executor: CommandExecutor,
): Effect.Effect<PlatformInfo, IntrospectionError> =>
  Effect.gen(function* () {
    // Check if running in Docker
    const { exitCode: dockerCheck } = yield* executor.execWithCode(
      "[ -f /.dockerenv ] && echo 'docker' || echo 'local'",
    );

    // Get OS release info
    const osRelease = yield* executor.exec(
      "cat /etc/os-release 2>/dev/null || echo ''",
    );

    let osDistro: string | undefined;
    let osVersion: string | undefined;

    if (osRelease) {
      const idMatch = osRelease.match(/^ID=(.+)$/m);
      const versionMatch = osRelease.match(/^VERSION_ID=(.+)$/m);
      osDistro = idMatch?.[1]?.replace(/"/g, "");
      osVersion = versionMatch?.[1]?.replace(/"/g, "");
    }

    // Check for container image env var
    const containerImage = yield* executor.exec(
      "echo $CONTAINER_IMAGE 2>/dev/null || echo ''",
    );

    const isDocker = dockerCheck === 0 || (yield* executor.exec("[ -f /.dockerenv ] && echo 'yes' || echo 'no'")) === "yes";

    const result: PlatformInfo = { type: isDocker ? "docker" : "local" };
    if (osDistro) result.osDistro = osDistro;
    if (osVersion) result.osVersion = osVersion;
    if (containerImage) result.containerImage = containerImage;
    return result;
  });

// ============================================================================
// Language Detection
// ============================================================================

const introspectLanguages = (
  executor: CommandExecutor,
): Effect.Effect<LanguageEnvironments, IntrospectionError> =>
  Effect.gen(function* () {
    const languages: LanguageEnvironments = {};

    // Python
    const pythonVersion = yield* executor.exec(
      "python3 --version 2>/dev/null | cut -d' ' -f2 || echo ''",
    );
    if (pythonVersion) {
      const pipOutput = yield* executor.exec(
        "pip list --format=freeze 2>/dev/null || pip3 list --format=freeze 2>/dev/null || echo ''",
      );
      const packages = parsePipPackages(pipOutput);
      const executable = yield* executor.exec("which python3 2>/dev/null || which python 2>/dev/null || echo ''");

      languages.python = {
        version: pythonVersion,
        packages,
        executable: executable || "/usr/bin/python3",
      };
    }

    // Node.js
    const nodeVersion = yield* executor.exec(
      "node --version 2>/dev/null | tr -d 'v' || echo ''",
    );
    if (nodeVersion) {
      const npmVersion = yield* executor.exec(
        "npm --version 2>/dev/null || echo ''",
      );
      const npmOutput = yield* executor.exec(
        "npm list --depth=0 --json 2>/dev/null || echo '{}'",
      );
      const packages = parseNpmPackages(npmOutput);

      languages.node = { version: nodeVersion, packages };
      if (npmVersion) languages.node.npmVersion = npmVersion;
    }

    // R
    const rVersion = yield* executor.exec(
      "R --version 2>/dev/null | head -1 | grep -oP '\\d+\\.\\d+\\.\\d+' || echo ''",
    );
    if (rVersion) {
      const rPackages = yield* executor.exec(
        `Rscript -e "cat(paste(installed.packages()[,1], installed.packages()[,3], sep='==', collapse='\\n'))" 2>/dev/null || echo ''`,
      );
      const packages = parseRPackages(rPackages);

      languages.r = {
        version: rVersion,
        packages,
      };
    }

    // Rust
    const rustVersion = yield* executor.exec(
      "rustc --version 2>/dev/null | grep -oP '\\d+\\.\\d+\\.\\d+' || echo ''",
    );
    if (rustVersion) {
      const cargoVersion = yield* executor.exec(
        "cargo --version 2>/dev/null | grep -oP '\\d+\\.\\d+\\.\\d+' || echo ''",
      );
      languages.rust = { version: rustVersion };
      if (cargoVersion) languages.rust.cargoVersion = cargoVersion;
    }

    // Go
    const goVersion = yield* executor.exec(
      "go version 2>/dev/null | grep -oP 'go\\d+\\.\\d+(\\.\\d+)?' | tr -d 'go' || echo ''",
    );
    if (goVersion) {
      languages.go = { version: goVersion };
    }

    // Java
    const javaVersion = yield* executor.exec(
      "java -version 2>&1 | head -1 | grep -oP '\"[^\"]+\"' | tr -d '\"' || echo ''",
    );
    if (javaVersion) {
      languages.java = { version: javaVersion };
    }

    return languages;
  });

// ============================================================================
// Tools Detection
// ============================================================================

const COMMON_TOOLS = [
  "git", "curl", "wget", "make", "gcc", "g++", "clang",
  "docker", "jq", "sed", "awk", "grep", "tar", "zip", "unzip",
  "cmake", "ninja", "pkg-config", "autoconf", "automake",
];

const introspectTools = (
  executor: CommandExecutor,
  taskDescription: string,
): Effect.Effect<ToolsInfo, IntrospectionError> =>
  Effect.gen(function* () {
    // Check which common tools are available
    const available: Array<{ name: string; path: string; version?: string }> = [];

    for (const tool of COMMON_TOOLS) {
      const { stdout, exitCode } = yield* executor.execWithCode(`which ${tool} 2>/dev/null`);
      if (exitCode === 0 && stdout) {
        const version = yield* executor.exec(
          `${tool} --version 2>/dev/null | head -1 | grep -oP '\\d+\\.\\d+(\\.\\d+)?' | head -1 || echo ''`,
        );
        const entry: { name: string; path: string; version?: string } = { name: tool, path: stdout };
        if (version) entry.version = version;
        available.push(entry);
      }
    }

    // Infer prohibited tools from task description
    const prohibited = inferProhibitedTools(taskDescription);

    // Check if prohibited tools exist (they shouldn't)
    const prohibitedCheck: Record<string, boolean> = {};
    for (const tool of prohibited) {
      const { exitCode } = yield* executor.execWithCode(`which ${tool.name} 2>/dev/null`);
      prohibitedCheck[tool.name] = exitCode === 0;
      tool.found = exitCode === 0;
    }

    return {
      available,
      prohibited,
      prohibitedCheck,
    };
  });

// ============================================================================
// Files Detection
// ============================================================================

const introspectFiles = (
  executor: CommandExecutor,
  workspace: string,
  taskDescription: string,
): Effect.Effect<FilesInfo, IntrospectionError> =>
  Effect.gen(function* () {
    // Get directory listing
    const lsOutput = yield* executor.exec(
      `ls -la ${workspace} 2>/dev/null | tail -n +4 | head -50`,
    );
    const listing = parseLsOutput(lsOutput, workspace);

    // Find task-relevant files to preview
    const taskFiles: FilePreview[] = [];

    // Preview common script/data files
    const extensions = ["py", "r", "R", "stan", "json", "csv", "yaml", "yml", "toml", "sh", "c", "cpp", "rs", "go"];
    const findPattern = extensions.map(ext => `-name "*.${ext}"`).join(" -o ");

    const foundFiles = yield* executor.exec(
      `find ${workspace} -maxdepth 3 -type f \\( ${findPattern} \\) 2>/dev/null | head -20`,
    );

    for (const filePath of foundFiles.split("\n").filter(Boolean)) {
      const preview = yield* getFilePreview(executor, filePath);
      if (preview) {
        taskFiles.push(preview);
      }
    }

    return {
      workdir: workspace,
      listing,
      taskFiles,
    };
  });

const getFilePreview = (
  executor: CommandExecutor,
  filePath: string,
): Effect.Effect<FilePreview | null, IntrospectionError> =>
  Effect.gen(function* () {
    // Get line count
    const lineCountStr = yield* executor.exec(
      `wc -l < "${filePath}" 2>/dev/null || echo '0'`,
    );
    const lineCount = parseInt(lineCountStr, 10) || 0;

    // Get preview (first 50 lines or 2KB)
    const preview = yield* executor.exec(
      `head -50 "${filePath}" 2>/dev/null | head -c 2048`,
    );

    if (!preview) return null;

    const extension = filePath.split(".").pop() || "";
    const detectedType = detectFileType(filePath, preview);
    const structure = extractStructure(preview, detectedType);

    const result: FilePreview = {
      path: filePath,
      extension,
      lineCount,
      preview,
      detectedType,
    };
    if (structure) result.structure = structure;
    return result;
  });

// ============================================================================
// Resources Detection
// ============================================================================

const introspectResources = (
  executor: CommandExecutor,
): Effect.Effect<ResourceInfo, IntrospectionError> =>
  Effect.gen(function* () {
    // Memory limit (cgroups v1 and v2)
    let memoryLimitMB: number | undefined;

    const cgroupV2Memory = yield* executor.exec(
      "cat /sys/fs/cgroup/memory.max 2>/dev/null || echo ''",
    );
    if (cgroupV2Memory && cgroupV2Memory !== "max") {
      memoryLimitMB = Math.floor(parseInt(cgroupV2Memory, 10) / 1024 / 1024);
    } else {
      const cgroupV1Memory = yield* executor.exec(
        "cat /sys/fs/cgroup/memory/memory.limit_in_bytes 2>/dev/null || echo ''",
      );
      if (cgroupV1Memory) {
        const bytes = parseInt(cgroupV1Memory, 10);
        // Check if it's not the "unlimited" value
        if (bytes < 9223372036854771712) {
          memoryLimitMB = Math.floor(bytes / 1024 / 1024);
        }
      }
    }

    // Available memory
    const memInfo = yield* executor.exec(
      "cat /proc/meminfo 2>/dev/null | grep MemAvailable | awk '{print $2}' || echo ''",
    );
    const memoryAvailableMB = memInfo ? Math.floor(parseInt(memInfo, 10) / 1024) : undefined;

    // CPU count
    const cpuCount = yield* executor.exec("nproc 2>/dev/null || echo ''");

    // Disk space
    const diskOutput = yield* executor.exec(
      "df -m . 2>/dev/null | tail -1 | awk '{print $4}' || echo ''",
    );
    const diskSpaceMB = diskOutput ? parseInt(diskOutput, 10) : undefined;

    const result: ResourceInfo = {};
    if (memoryLimitMB !== undefined) result.memoryLimitMB = memoryLimitMB;
    if (memoryAvailableMB !== undefined) result.memoryAvailableMB = memoryAvailableMB;
    if (cpuCount) result.cpuCount = parseInt(cpuCount, 10);
    if (diskSpaceMB !== undefined && !isNaN(diskSpaceMB)) result.diskSpaceMB = diskSpaceMB;
    return result;
  });

// ============================================================================
// Environment Variables
// ============================================================================

const SENSITIVE_PATTERNS = [
  /password/i,
  /secret/i,
  /token/i,
  /key/i,
  /credential/i,
  /auth/i,
  /api_key/i,
];

const introspectEnv = (
  executor: CommandExecutor,
): Effect.Effect<Record<string, string>, IntrospectionError> =>
  Effect.gen(function* () {
    const envOutput = yield* executor.exec("env 2>/dev/null || echo ''");
    const env: Record<string, string> = {};

    for (const line of envOutput.split("\n")) {
      const eqIndex = line.indexOf("=");
      if (eqIndex > 0) {
        const key = line.slice(0, eqIndex);
        const value = line.slice(eqIndex + 1);

        // Filter out sensitive variables
        if (!SENSITIVE_PATTERNS.some(p => p.test(key))) {
          env[key] = value;
        }
      }
    }

    return env;
  });

// ============================================================================
// Parsing Helpers
// ============================================================================

const parsePipPackages = (output: string): PackageInfo[] => {
  const packages: PackageInfo[] = [];
  for (const line of output.split("\n")) {
    const match = line.match(/^([^=]+)==(.+)$/);
    if (match) {
      packages.push({ name: match[1], version: match[2] });
    }
  }
  return packages;
};

const parseNpmPackages = (output: string): PackageInfo[] => {
  const packages: PackageInfo[] = [];
  try {
    const json = JSON.parse(output);
    if (json.dependencies) {
      for (const [name, info] of Object.entries(json.dependencies)) {
        const version = typeof info === "object" && info !== null && "version" in info
          ? String((info as { version: unknown }).version)
          : "unknown";
        packages.push({ name, version });
      }
    }
  } catch {
    // Ignore parse errors
  }
  return packages;
};

const parseRPackages = (output: string): PackageInfo[] => {
  const packages: PackageInfo[] = [];
  for (const line of output.split("\n")) {
    const match = line.match(/^([^=]+)==(.+)$/);
    if (match) {
      packages.push({ name: match[1], version: match[2] });
    }
  }
  return packages;
};

const parseLsOutput = (output: string, workdir: string): FileEntry[] => {
  const entries: FileEntry[] = [];
  for (const line of output.split("\n")) {
    if (!line.trim()) continue;

    // Parse ls -la output: permissions links owner group size date time name
    const parts = line.split(/\s+/);
    if (parts.length < 9) continue;

    const permissions = parts[0];
    const size = parseInt(parts[4], 10) || 0;
    const name = parts.slice(8).join(" ");

    let type: "file" | "directory" | "symlink" = "file";
    if (permissions.startsWith("d")) type = "directory";
    if (permissions.startsWith("l")) type = "symlink";

    entries.push({
      name,
      path: `${workdir}/${name}`,
      type,
      size,
      permissions,
    });
  }
  return entries;
};

// ============================================================================
// Structure Extraction
// ============================================================================

const extractStructure = (
  content: string,
  fileType: DetectedFileType,
): ExtractedStructure | undefined => {
  const structure: ExtractedStructure = {};

  switch (fileType) {
    case "python_script": {
      // Extract imports
      const imports = content.match(/^(?:from\s+\S+\s+)?import\s+.+$/gm);
      if (imports) structure.imports = imports.map(s => s.trim());

      // Extract function names
      const functions = content.match(/^def\s+(\w+)/gm);
      if (functions) structure.functions = functions.map(s => s.replace("def ", ""));

      // Extract class names
      const classes = content.match(/^class\s+(\w+)/gm);
      if (classes) structure.classes = classes.map(s => s.replace("class ", "").split("(")[0]);

      // Extract variable assignments at module level
      const variables = content.match(/^(\w+)\s*=/gm);
      if (variables) structure.variables = variables.map(s => s.replace(/\s*=.*/, ""));
      break;
    }

    case "r_script": {
      // Extract library calls
      const imports = content.match(/library\(['""]?(\w+)['""]?\)/g);
      if (imports) structure.imports = imports.map(s => s.replace(/library\(['""]?/, "").replace(/['""]?\)/, ""));

      // Extract function definitions
      const functions = content.match(/(\w+)\s*<-\s*function/g);
      if (functions) structure.functions = functions.map(s => s.split("<-")[0].trim());

      // Extract variable assignments
      const variables = content.match(/^(\w+)\s*<-/gm);
      if (variables) structure.variables = variables.map(s => s.replace(/\s*<-.*/, ""));
      break;
    }

    case "stan_model": {
      // Extract parameters block variables
      const paramsMatch = content.match(/parameters\s*\{([^}]+)\}/s);
      if (paramsMatch) {
        const params = paramsMatch[1].match(/\b(real|int|vector|matrix)\s*(?:<[^>]+>)?\s+(\w+)/g);
        if (params) {
          structure.parameters = params.map(s => {
            const parts = s.split(/\s+/);
            return parts[parts.length - 1];
          });
        }
      }

      // Extract data block variables
      const dataMatch = content.match(/data\s*\{([^}]+)\}/s);
      if (dataMatch) {
        const data = dataMatch[1].match(/\b(real|int|vector|matrix)\s*(?:<[^>]+>)?\s+(\w+)/g);
        if (data) {
          structure.variables = data.map(s => {
            const parts = s.split(/\s+/);
            return parts[parts.length - 1];
          });
        }
      }
      break;
    }

    case "csv": {
      // Extract column names from first line
      const firstLine = content.split("\n")[0];
      if (firstLine) {
        structure.columns = firstLine.split(",").map(s => s.trim().replace(/^["']|["']$/g, ""));
      }
      break;
    }

    case "json": {
      try {
        const json = JSON.parse(content);
        if (typeof json === "object" && json !== null) {
          structure.variables = Object.keys(json);
        }
      } catch {
        // Partial JSON, extract keys with regex
        const keys = content.match(/"(\w+)"\s*:/g);
        if (keys) {
          structure.variables = [...new Set(keys.map(s => s.replace(/"(\w+)"\s*:/, "$1")))];
        }
      }
      break;
    }
  }

  return Object.keys(structure).length > 0 ? structure : undefined;
};
