import { readFileSync, existsSync } from "fs";
import { dirname, join, resolve } from "path";
import { homedir } from "os";

const CONTEXT_FILES = ["AGENTS.md", "CLAUDE.md"];

const readIfExists = (path: string): string | null => {
  if (!existsSync(path)) return null;
  try {
    return readFileSync(path, "utf8");
  } catch {
    return null;
  }
};

const findContextInDir = (dir: string): string | null => {
  for (const filename of CONTEXT_FILES) {
    const content = readIfExists(join(dir, filename));
    if (content) return content;
  }
  return null;
};

export const loadContextFiles = (cwd: string = process.cwd()): string[] => {
  const contexts: string[] = [];

  // Global
  const home = homedir();
  const globalCtx = findContextInDir(resolve(home, ".pi", "agent")) || findContextInDir(home);
  if (globalCtx) contexts.push(globalCtx);

  // Ancestors
  let current = resolve(cwd);
  const root = dirname(current);
  const visited = new Set<string>();
  while (!visited.has(current)) {
    visited.add(current);
    const ctx = findContextInDir(current);
    if (ctx) contexts.push(ctx);
    if (current === root) break;
    const parent = dirname(current);
    if (parent === current) break;
    current = parent;
  }

  return contexts;
};

export const buildSystemPromptWithContext = (basePrompt: string, cwd: string = process.cwd()): string => {
  const contexts = loadContextFiles(cwd);
  const now = new Date().toISOString();
  const header = [`Current time: ${now}`, `CWD: ${cwd}`, `Context files loaded: ${contexts.length}`].join("\n");
  return [header, ...contexts, basePrompt].join("\n\n");
};
