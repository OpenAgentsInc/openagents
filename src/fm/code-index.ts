/**
 * Code Index Service for FM Navigation
 *
 * Builds a structural index of TypeScript/JavaScript files to enable
 * FM to request code by semantic handle (e.g., "func:validateCredentials")
 * instead of raw line numbers.
 *
 * Based on docs/logs/20251206/1421-coding-thoughts.md section 8:
 * "Chunk the repo structurally, not by raw size"
 */

// --- Schema ---

/**
 * Type of code chunk.
 */
export type ChunkType = "function" | "type" | "interface" | "class" | "const" | "export";

/**
 * A code chunk with semantic identity and line range.
 */
export interface CodeChunk {
  /** Unique ID like "func:validateCredentials" or "type:AuthResult" */
  id: string;
  /** Type of chunk */
  type: ChunkType;
  /** Name of the symbol */
  name: string;
  /** File path relative to workDir */
  path: string;
  /** Start line (1-indexed) */
  startLine: number;
  /** End line (1-indexed) */
  endLine: number;
  /** Character count (for FM context budget) */
  charCount: number;
  /** Optional signature/header for quick reference */
  signature?: string;
  /** Dependencies (other chunk IDs this references) */
  deps?: string[];
}

/**
 * File index containing all chunks from a single file.
 */
export interface FileIndex {
  path: string;
  chunks: CodeChunk[];
  totalLines: number;
  lastModified: number;
}

/**
 * Full code index for a directory.
 */
export interface CodeIndex {
  workDir: string;
  files: Map<string, FileIndex>;
  chunkById: Map<string, CodeChunk>;
  createdAt: number;
}

// --- Error Types ---

export class CodeIndexError extends Error {
  readonly _tag = "CodeIndexError";
  constructor(
    readonly reason: "file_not_found" | "parse_error" | "chunk_not_found",
    message: string,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "CodeIndexError";
  }
}

// --- Regex Patterns for TypeScript ---

const PATTERNS = {
  // Match function declarations: function name(...) or async function name(...)
  function: /^(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*\(/gm,

  // Match arrow functions: const name = (...) => or const name = async (...) =>
  arrowFunction: /^(?:export\s+)?const\s+(\w+)\s*=\s*(?:async\s+)?(?:\([^)]*\)|[^=])\s*=>/gm,

  // Match type declarations: type Name = ...
  type: /^(?:export\s+)?type\s+(\w+)\s*(?:<[^>]*>)?\s*=/gm,

  // Match interface declarations: interface Name { ... }
  interface: /^(?:export\s+)?interface\s+(\w+)\s*(?:<[^>]*>)?\s*\{?/gm,

  // Match class declarations: class Name { ... }
  class: /^(?:export\s+)?(?:abstract\s+)?class\s+(\w+)/gm,

  // Match const declarations: const name = ...
  const: /^(?:export\s+)?const\s+(\w+)\s*(?::\s*[^=]+)?\s*=/gm,
};

// --- Chunk ID Generation ---

function makeChunkId(type: ChunkType, name: string): string {
  const prefix = type === "function" || type === "const" ? "func" : type;
  return `${prefix}:${name}`;
}

// --- Line Range Detection ---

/**
 * Find the end of a code block starting at a given line.
 * Uses brace matching for functions/classes/interfaces.
 */
function findBlockEnd(lines: string[], startLine: number): number {
  let braceCount = 0;
  let foundFirstBrace = false;
  let inString = false;
  let stringChar = "";

  for (let i = startLine; i < lines.length; i++) {
    const line = lines[i];

    for (let j = 0; j < line.length; j++) {
      const char = line[j];
      const prevChar = j > 0 ? line[j - 1] : "";

      // Handle string literals
      if ((char === '"' || char === "'" || char === "`") && prevChar !== "\\") {
        if (!inString) {
          inString = true;
          stringChar = char;
        } else if (char === stringChar) {
          inString = false;
        }
        continue;
      }

      if (inString) continue;

      // Skip comments
      if (char === "/" && line[j + 1] === "/") break; // Line comment

      // Count braces
      if (char === "{") {
        braceCount++;
        foundFirstBrace = true;
      } else if (char === "}") {
        braceCount--;
        if (foundFirstBrace && braceCount === 0) {
          return i + 1; // 1-indexed
        }
      }
    }

    // For single-line declarations without braces (like type aliases)
    if (i === startLine && !foundFirstBrace && (line.includes(";") || line.trim().endsWith(";"))) {
      return i + 1;
    }
  }

  // If no closing brace found, return a reasonable end (10 lines or end of file)
  return Math.min(startLine + 10, lines.length);
}

/**
 * Find end of a type alias (ends with semicolon, possibly multi-line).
 */
function findTypeEnd(lines: string[], startLine: number): number {
  let depth = 0;
  let foundStart = false;

  for (let i = startLine; i < lines.length; i++) {
    const line = lines[i];

    for (const char of line) {
      if (char === "{" || char === "<" || char === "(") {
        depth++;
        foundStart = true;
      } else if (char === "}" || char === ">" || char === ")") {
        depth--;
      } else if (char === ";" && depth === 0) {
        return i + 1;
      }
    }

    // Single-line type with semicolon
    if (i === startLine && line.includes(";") && !foundStart) {
      return i + 1;
    }
  }

  return Math.min(startLine + 5, lines.length);
}

// --- File Indexer ---

/**
 * Index a single TypeScript/JavaScript file.
 */
export function indexFile(path: string, content: string): FileIndex {
  const lines = content.split("\n");
  const chunks: CodeChunk[] = [];

  // Reset regex lastIndex
  for (const pattern of Object.values(PATTERNS)) {
    pattern.lastIndex = 0;
  }

  // Find all function declarations
  let match: RegExpExecArray | null;
  while ((match = PATTERNS.function.exec(content)) !== null) {
    const name = match[1];
    const startLine = content.substring(0, match.index).split("\n").length;
    const endLine = findBlockEnd(lines, startLine - 1);
    const chunkContent = lines.slice(startLine - 1, endLine).join("\n");

    chunks.push({
      id: makeChunkId("function", name),
      type: "function",
      name,
      path,
      startLine,
      endLine,
      charCount: chunkContent.length,
      signature: lines[startLine - 1].trim(),
    });
  }

  // Find arrow functions (const name = () => ...)
  PATTERNS.arrowFunction.lastIndex = 0;
  while ((match = PATTERNS.arrowFunction.exec(content)) !== null) {
    const name = match[1];
    // Skip if already found as regular function
    if (chunks.some((c) => c.name === name)) continue;

    const startLine = content.substring(0, match.index).split("\n").length;
    const endLine = findBlockEnd(lines, startLine - 1);
    const chunkContent = lines.slice(startLine - 1, endLine).join("\n");

    chunks.push({
      id: makeChunkId("function", name),
      type: "function",
      name,
      path,
      startLine,
      endLine,
      charCount: chunkContent.length,
      signature: lines[startLine - 1].trim().slice(0, 80),
    });
  }

  // Find type declarations
  PATTERNS.type.lastIndex = 0;
  while ((match = PATTERNS.type.exec(content)) !== null) {
    const name = match[1];
    const startLine = content.substring(0, match.index).split("\n").length;
    const endLine = findTypeEnd(lines, startLine - 1);
    const chunkContent = lines.slice(startLine - 1, endLine).join("\n");

    chunks.push({
      id: makeChunkId("type", name),
      type: "type",
      name,
      path,
      startLine,
      endLine,
      charCount: chunkContent.length,
      signature: lines[startLine - 1].trim().slice(0, 80),
    });
  }

  // Find interface declarations
  PATTERNS.interface.lastIndex = 0;
  while ((match = PATTERNS.interface.exec(content)) !== null) {
    const name = match[1];
    const startLine = content.substring(0, match.index).split("\n").length;
    const endLine = findBlockEnd(lines, startLine - 1);
    const chunkContent = lines.slice(startLine - 1, endLine).join("\n");

    chunks.push({
      id: makeChunkId("interface", name),
      type: "interface",
      name,
      path,
      startLine,
      endLine,
      charCount: chunkContent.length,
      signature: lines[startLine - 1].trim(),
    });
  }

  // Find class declarations
  PATTERNS.class.lastIndex = 0;
  while ((match = PATTERNS.class.exec(content)) !== null) {
    const name = match[1];
    const startLine = content.substring(0, match.index).split("\n").length;
    const endLine = findBlockEnd(lines, startLine - 1);
    const chunkContent = lines.slice(startLine - 1, endLine).join("\n");

    chunks.push({
      id: makeChunkId("class", name),
      type: "class",
      name,
      path,
      startLine,
      endLine,
      charCount: chunkContent.length,
      signature: lines[startLine - 1].trim(),
    });
  }

  return {
    path,
    chunks,
    totalLines: lines.length,
    lastModified: Date.now(),
  };
}

// --- Simple In-Memory Code Index ---

/**
 * Create a code index from file contents.
 * This is a pure function that doesn't use Effect for simplicity.
 */
export function createCodeIndex(
  workDir: string,
  fileContents: Map<string, string>,
): CodeIndex {
  const files = new Map<string, FileIndex>();
  const chunkById = new Map<string, CodeChunk>();

  for (const [path, content] of fileContents) {
    const fileIndex = indexFile(path, content);
    files.set(path, fileIndex);

    for (const chunk of fileIndex.chunks) {
      chunkById.set(chunk.id, chunk);
    }
  }

  return {
    workDir,
    files,
    chunkById,
    createdAt: Date.now(),
  };
}

/**
 * Get chunks that fit within a character budget.
 */
export function getChunksWithinBudget(
  index: CodeIndex,
  chunkIds: string[],
  maxChars: number,
): { chunks: CodeChunk[]; totalChars: number } {
  const result: CodeChunk[] = [];
  let totalChars = 0;

  for (const id of chunkIds) {
    const chunk = index.chunkById.get(id);
    if (!chunk) continue;

    if (totalChars + chunk.charCount <= maxChars) {
      result.push(chunk);
      totalChars += chunk.charCount;
    }
  }

  return { chunks: result, totalChars };
}

/**
 * Find chunks matching a query (name substring).
 */
export function findChunks(index: CodeIndex, query: string, limit = 10): CodeChunk[] {
  const lowerQuery = query.toLowerCase();
  const matches: CodeChunk[] = [];

  for (const chunk of index.chunkById.values()) {
    if (
      chunk.name.toLowerCase().includes(lowerQuery) ||
      chunk.id.toLowerCase().includes(lowerQuery)
    ) {
      matches.push(chunk);
      if (matches.length >= limit) break;
    }
  }

  return matches;
}

/**
 * Get content for a chunk from the original file content.
 */
export function getChunkContent(
  chunk: CodeChunk,
  fileContent: string,
): string {
  const lines = fileContent.split("\n");
  return lines.slice(chunk.startLine - 1, chunk.endLine).join("\n");
}

/**
 * Format chunks for FM prompt injection.
 * Returns formatted code with chunk IDs and line numbers.
 */
export function formatChunksForPrompt(
  chunks: CodeChunk[],
  fileContents: Map<string, string>,
  maxChars = 800,
): string {
  if (chunks.length === 0) {
    return "No code chunks available.";
  }

  const parts: string[] = [];
  let totalChars = 0;

  for (const chunk of chunks) {
    const fileContent = fileContents.get(chunk.path);
    if (!fileContent) continue;

    const content = getChunkContent(chunk, fileContent);
    if (totalChars + content.length > maxChars) break;

    parts.push(`// ${chunk.id} (${chunk.path}:${chunk.startLine}-${chunk.endLine})\n${content}`);
    totalChars += content.length + 50; // Account for header
  }

  return parts.join("\n\n");
}

/**
 * Build index from a directory using Bun's file system.
 * This is async and uses Bun.file directly.
 */
export async function buildIndexFromDirectory(
  workDir: string,
  patterns: string[] = ["**/*.ts", "**/*.tsx"],
): Promise<CodeIndex> {
  const fileContents = new Map<string, string>();

  // Use Bun's glob to find files
  const glob = new Bun.Glob(patterns.join("|"));

  for await (const file of glob.scan({
    cwd: workDir,
    absolute: false,
    onlyFiles: true,
  })) {
    // Skip test files, declaration files, node_modules
    if (
      file.endsWith(".test.ts") ||
      file.endsWith(".d.ts") ||
      file.includes("node_modules") ||
      file.includes("dist")
    ) {
      continue;
    }

    try {
      const fullPath = `${workDir}/${file}`;
      const content = await Bun.file(fullPath).text();
      fileContents.set(file, content);
    } catch {
      // Skip files we can't read
    }
  }

  return createCodeIndex(workDir, fileContents);
}

/**
 * Quick utility to get chunk IDs for FM context.
 * Returns the most relevant chunks for a given task.
 */
export function selectChunksForTask(
  index: CodeIndex,
  keywords: string[],
  maxChars = 600,
): CodeChunk[] {
  const allMatches: CodeChunk[] = [];

  for (const keyword of keywords) {
    const matches = findChunks(index, keyword, 5);
    for (const match of matches) {
      if (!allMatches.some((m) => m.id === match.id)) {
        allMatches.push(match);
      }
    }
  }

  // Sort by relevance (shorter = more specific)
  allMatches.sort((a, b) => a.charCount - b.charCount);

  // Return chunks that fit in budget
  return getChunksWithinBudget(index, allMatches.map((c) => c.id), maxChars).chunks;
}
