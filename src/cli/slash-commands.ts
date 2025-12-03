/**
 * Slash commands system for workflow shortcuts.
 *
 * Loads markdown commands from:
 *   - ~/.openagents/commands/ (user commands)
 *   - .openagents/commands/ (project commands)
 *
 * Each command is a .md file with optional YAML frontmatter:
 *
 * ```markdown
 * ---
 * description: Run tests for a specific file
 * args:
 *   - name: file
 *     description: The file to test
 *     required: true
 * ---
 * Run tests for the file: $1
 *
 * Make sure all tests pass before continuing.
 * ```
 *
 * Usage:
 *   /test src/foo.ts
 *   -> Expands to the command body with $1 = "src/foo.ts"
 *
 * Argument substitution:
 *   - $1, $2, $3... - Positional arguments
 *   - $@ - All arguments joined by space
 *   - $ARGS - Same as $@
 */

import { existsSync, readdirSync, readFileSync } from "fs";
import { join } from "path";
import { homedir } from "os";

export interface CommandArg {
  name: string;
  description?: string;
  required?: boolean;
  default?: string;
}

export interface SlashCommand {
  /** Command name (filename without .md extension) */
  name: string;
  /** Full path to the command file */
  path: string;
  /** Short description from frontmatter */
  description?: string;
  /** Argument definitions from frontmatter */
  args?: CommandArg[];
  /** Whether this is a user (~/.openagents) or project (.openagents) command */
  source: "user" | "project";
  /** Raw markdown body (after frontmatter) */
  body: string;
}

export interface ExpandedCommand {
  /** Original slash command input (e.g., "/test foo.ts") */
  input: string;
  /** The matched command */
  command: SlashCommand;
  /** Arguments passed to the command */
  arguments: string[];
  /** Expanded prompt text */
  prompt: string;
}

/**
 * Parse YAML frontmatter from markdown content.
 * Returns the parsed frontmatter object and the remaining body.
 */
export function parseFrontmatter(content: string): {
  frontmatter: Record<string, unknown>;
  body: string;
} {
  const trimmed = content.trimStart();

  // Check for frontmatter delimiter
  if (!trimmed.startsWith("---")) {
    return { frontmatter: {}, body: content };
  }

  // Find closing delimiter
  const endIndex = trimmed.indexOf("\n---", 3);
  if (endIndex === -1) {
    return { frontmatter: {}, body: content };
  }

  const yamlContent = trimmed.slice(4, endIndex).trim();
  const body = trimmed.slice(endIndex + 4).trim();

  // Simple YAML parser for our use case (handles description, args array)
  const frontmatter: Record<string, unknown> = {};

  const lines = yamlContent.split("\n");
  let currentKey: string | null = null;
  let currentArray: Record<string, unknown>[] | null = null;
  let currentArrayItem: Record<string, unknown> | null = null;

  for (const line of lines) {
    // Skip empty lines
    if (!line.trim()) continue;

    // Check for top-level key
    const topLevelMatch = line.match(/^(\w+):\s*(.*)?$/);
    if (topLevelMatch) {
      // Save previous array if exists
      if (currentArray !== null && currentKey) {
        if (currentArrayItem !== null) {
          currentArray.push(currentArrayItem);
        }
        frontmatter[currentKey] = currentArray;
      }

      currentKey = topLevelMatch[1];
      const value = topLevelMatch[2]?.trim();

      if (value) {
        // Simple key: value
        frontmatter[currentKey] = value;
        currentArray = null;
        currentArrayItem = null;
      } else {
        // Might be start of array or nested object
        currentArray = [];
        currentArrayItem = null;
      }
      continue;
    }

    // Check for array item (- name: value or just - value)
    const arrayItemMatch = line.match(/^\s+-\s+(\w+):\s*(.*)$/);
    if (arrayItemMatch && currentArray !== null) {
      // Save previous item
      if (currentArrayItem !== null) {
        currentArray.push(currentArrayItem);
      }
      currentArrayItem = { [arrayItemMatch[1]]: arrayItemMatch[2].trim() };
      continue;
    }

    // Check for continuation of array item (indented key: value)
    const continuationMatch = line.match(/^\s{4,}(\w+):\s*(.*)$/);
    if (continuationMatch && currentArrayItem !== null) {
      const val = continuationMatch[2].trim();
      // Handle boolean conversion
      if (val === "true") {
        currentArrayItem[continuationMatch[1]] = true;
      } else if (val === "false") {
        currentArrayItem[continuationMatch[1]] = false;
      } else {
        currentArrayItem[continuationMatch[1]] = val;
      }
      continue;
    }
  }

  // Save final array if exists
  if (currentArray !== null && currentKey) {
    if (currentArrayItem !== null) {
      currentArray.push(currentArrayItem);
    }
    frontmatter[currentKey] = currentArray;
  }

  return { frontmatter, body };
}

/**
 * Load a single slash command from a file path.
 */
export function loadCommand(
  filePath: string,
  source: "user" | "project",
): SlashCommand | null {
  try {
    const content = readFileSync(filePath, "utf-8");
    const name = filePath.split("/").pop()?.replace(/\.md$/, "") ?? "";

    const { frontmatter, body } = parseFrontmatter(content);

    return {
      name,
      path: filePath,
      description: frontmatter.description as string | undefined,
      args: frontmatter.args as CommandArg[] | undefined,
      source,
      body,
    };
  } catch {
    return null;
  }
}

/**
 * Load all commands from a directory.
 */
export function loadCommandsFromDir(
  dir: string,
  source: "user" | "project",
): SlashCommand[] {
  if (!existsSync(dir)) {
    return [];
  }

  try {
    const files = readdirSync(dir);
    const commands: SlashCommand[] = [];

    for (const file of files) {
      if (!file.endsWith(".md")) continue;
      const command = loadCommand(join(dir, file), source);
      if (command) {
        commands.push(command);
      }
    }

    return commands;
  } catch {
    return [];
  }
}

/**
 * Load all available slash commands from both user and project directories.
 * Project commands take precedence over user commands with the same name.
 */
export function loadAllCommands(cwd: string = process.cwd()): Map<string, SlashCommand> {
  const commands = new Map<string, SlashCommand>();

  // Load user commands first (lower priority)
  const userDir = join(homedir(), ".openagents", "commands");
  for (const cmd of loadCommandsFromDir(userDir, "user")) {
    commands.set(cmd.name, cmd);
  }

  // Load project commands (higher priority, overrides user commands)
  const projectDir = join(cwd, ".openagents", "commands");
  for (const cmd of loadCommandsFromDir(projectDir, "project")) {
    commands.set(cmd.name, cmd);
  }

  return commands;
}

/**
 * Substitute arguments in the command body.
 *
 * Supports:
 *   - $1, $2, $3... - Positional arguments
 *   - $@ - All arguments joined by space
 *   - $ARGS - Same as $@
 */
export function substituteArgs(body: string, args: string[]): string {
  let result = body;

  // Replace $@ and $ARGS with all arguments
  const allArgs = args.join(" ");
  result = result.replace(/\$@/g, allArgs);
  result = result.replace(/\$ARGS/g, allArgs);

  // Replace positional arguments $1, $2, etc.
  // Start from highest to avoid $1 matching in $10
  for (let i = Math.max(args.length, 9); i >= 1; i--) {
    const value = args[i - 1] ?? "";
    result = result.replace(new RegExp(`\\$${i}`, "g"), value);
  }

  return result;
}

/**
 * Parse a slash command input string.
 *
 * Examples:
 *   "/test foo.ts" -> { name: "test", args: ["foo.ts"] }
 *   "/review --detailed" -> { name: "review", args: ["--detailed"] }
 */
export function parseSlashInput(input: string): { name: string; args: string[] } | null {
  const trimmed = input.trim();

  if (!trimmed.startsWith("/")) {
    return null;
  }

  // Remove leading slash and split
  const parts = trimmed.slice(1).split(/\s+/);
  const name = parts[0];

  if (!name) {
    return null;
  }

  return {
    name,
    args: parts.slice(1),
  };
}

/**
 * Expand a slash command input into a full prompt.
 *
 * @param input - The slash command input (e.g., "/test foo.ts")
 * @param commands - Map of available commands (from loadAllCommands)
 * @returns The expanded command or null if not found
 */
export function expandSlashCommand(
  input: string,
  commands: Map<string, SlashCommand>,
): ExpandedCommand | null {
  const parsed = parseSlashInput(input);
  if (!parsed) {
    return null;
  }

  const command = commands.get(parsed.name);
  if (!command) {
    return null;
  }

  const prompt = substituteArgs(command.body, parsed.args);

  return {
    input,
    command,
    arguments: parsed.args,
    prompt,
  };
}

/**
 * Check if input starts with a slash command.
 */
export function isSlashCommand(input: string): boolean {
  return input.trim().startsWith("/");
}

/**
 * Get a list of available commands with their descriptions.
 * Useful for help/autocomplete.
 */
export function listCommands(
  commands: Map<string, SlashCommand>,
): Array<{ name: string; description?: string; source: "user" | "project" }> {
  return Array.from(commands.values()).map((cmd) => ({
    name: cmd.name,
    description: cmd.description,
    source: cmd.source,
  }));
}
