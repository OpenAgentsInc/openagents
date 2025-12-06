/**
 * Primitive Skills Library
 *
 * 40 foundational skills for MechaCoder covering:
 * - File operations (read, write, edit, glob, grep)
 * - Testing (run_test, run_typecheck, run_lint)
 * - Git (add, commit, diff, status)
 * - Debugging (analyze_error, fix_import, fix_syntax)
 * - Shell (execute_command, check_output)
 * - Search (find_file, search_code, find_definition)
 */

import { type Skill, createSkill } from "../schema.js";

// --- File Operations Skills ---

const readFile: Skill = createSkill({
  name: "Read File",
  description: "Read the contents of a file. Use when you need to examine file contents before making changes.",
  category: "file_operations",
  code: `// Read file contents
const content = await Bun.file(path).text();
// Returns: file content as string`,
  parameters: [
    { name: "path", type: "path", description: "Path to the file to read", required: true },
  ],
  verification: { type: "none" },
  tags: ["file", "read", "input"],
  source: "bootstrap",
});

const writeFile: Skill = createSkill({
  name: "Write File",
  description: "Write content to a file, creating it if it doesn't exist. Use for creating new files or completely replacing file contents.",
  category: "file_operations",
  code: `// Write content to file
await Bun.write(path, content);
// Creates file if doesn't exist, overwrites if exists`,
  parameters: [
    { name: "path", type: "path", description: "Path to the file to write", required: true },
    { name: "content", type: "string", description: "Content to write to the file", required: true },
  ],
  verification: { type: "command", command: "test -f {path}" },
  tags: ["file", "write", "output", "create"],
  source: "bootstrap",
});

const editFile: Skill = createSkill({
  name: "Edit File",
  description: "Replace specific text in a file. Use for targeted modifications without rewriting the entire file.",
  category: "file_operations",
  code: `// Edit file by replacing text
const content = await Bun.file(path).text();
const updated = content.replace(oldText, newText);
await Bun.write(path, updated);`,
  parameters: [
    { name: "path", type: "path", description: "Path to the file to edit", required: true },
    { name: "oldText", type: "string", description: "Text to find and replace", required: true },
    { name: "newText", type: "string", description: "Text to replace with", required: true },
  ],
  verification: { type: "pattern", pattern: "{newText}" },
  tags: ["file", "edit", "modify", "replace"],
  source: "bootstrap",
});

const globFiles: Skill = createSkill({
  name: "Glob Files",
  description: "Find files matching a glob pattern. Use to discover files before reading or modifying them.",
  category: "file_operations",
  code: `// Find files matching pattern
const glob = new Bun.Glob(pattern);
const files = await Array.fromAsync(glob.scan({ cwd: directory }));
// Returns: array of matching file paths`,
  parameters: [
    { name: "pattern", type: "pattern", description: "Glob pattern (e.g., '**/*.ts')", required: true },
    { name: "directory", type: "path", description: "Directory to search in", required: false, default: "." },
  ],
  verification: { type: "none" },
  tags: ["file", "search", "glob", "find"],
  source: "bootstrap",
});

const grepCode: Skill = createSkill({
  name: "Grep Code",
  description: "Search for text patterns in files. Use to find specific code patterns or text.",
  category: "search",
  code: `// Search for pattern in files
const proc = Bun.spawn(["grep", "-rn", pattern, path]);
const output = await new Response(proc.stdout).text();
// Returns: matching lines with file:line:content format`,
  parameters: [
    { name: "pattern", type: "pattern", description: "Regex pattern to search for", required: true },
    { name: "path", type: "path", description: "File or directory to search", required: false, default: "." },
  ],
  verification: { type: "none" },
  tags: ["search", "grep", "find", "pattern"],
  source: "bootstrap",
});

// --- Testing Skills ---

const runTest: Skill = createSkill({
  name: "Run Tests",
  description: "Execute test suite using bun test. Use to verify code changes don't break existing functionality.",
  category: "testing",
  code: `// Run tests
const proc = Bun.spawn(["bun", "test", testPattern], { cwd: projectRoot });
const exitCode = await proc.exited;
// exitCode 0 = success, non-zero = failure`,
  parameters: [
    { name: "testPattern", type: "string", description: "Test file pattern (e.g., 'src/**/*.test.ts')", required: false },
    { name: "projectRoot", type: "path", description: "Project root directory", required: false, default: "." },
  ],
  verification: { type: "command", command: "bun test {testPattern}" },
  tags: ["test", "verify", "quality"],
  source: "bootstrap",
});

const runTypecheck: Skill = createSkill({
  name: "Run Typecheck",
  description: "Run TypeScript type checking. Use to verify type correctness after code changes.",
  category: "testing",
  code: `// Run TypeScript type check
const proc = Bun.spawn(["bun", "run", "build:check"], { cwd: projectRoot });
const exitCode = await proc.exited;
// exitCode 0 = no type errors`,
  parameters: [
    { name: "projectRoot", type: "path", description: "Project root directory", required: false, default: "." },
  ],
  verification: { type: "command", command: "bun run build:check" },
  tags: ["typecheck", "typescript", "types", "verify"],
  source: "bootstrap",
});

const runLint: Skill = createSkill({
  name: "Run Lint",
  description: "Run linter to check code style. Use to ensure code follows project conventions.",
  category: "testing",
  code: `// Run ESLint
const proc = Bun.spawn(["bun", "run", "lint"], { cwd: projectRoot });
const exitCode = await proc.exited;
// exitCode 0 = no lint errors`,
  parameters: [
    { name: "projectRoot", type: "path", description: "Project root directory", required: false, default: "." },
  ],
  verification: { type: "command", command: "bun run lint" },
  tags: ["lint", "style", "quality"],
  source: "bootstrap",
});

// --- Git Skills ---

const gitStatus: Skill = createSkill({
  name: "Git Status",
  description: "Check git repository status. Use to see changed, staged, and untracked files.",
  category: "git",
  code: `// Check git status
const proc = Bun.spawn(["git", "status", "--porcelain"]);
const output = await new Response(proc.stdout).text();
// Returns: modified files in short format`,
  parameters: [],
  verification: { type: "none" },
  tags: ["git", "status", "changes"],
  source: "bootstrap",
});

const gitDiff: Skill = createSkill({
  name: "Git Diff",
  description: "Show changes in files. Use to review changes before committing.",
  category: "git",
  code: `// Show git diff
const proc = Bun.spawn(["git", "diff", path || "."]);
const output = await new Response(proc.stdout).text();
// Returns: unified diff output`,
  parameters: [
    { name: "path", type: "path", description: "Specific file or directory to diff", required: false },
    { name: "staged", type: "boolean", description: "Show staged changes only", required: false, default: "false" },
  ],
  verification: { type: "none" },
  tags: ["git", "diff", "changes"],
  source: "bootstrap",
});

const gitAdd: Skill = createSkill({
  name: "Git Add",
  description: "Stage files for commit. Use to prepare changes for committing.",
  category: "git",
  code: `// Stage files
const proc = Bun.spawn(["git", "add", ...files]);
await proc.exited;`,
  parameters: [
    { name: "files", type: "array", description: "Files to stage (use '.' for all)", required: true },
  ],
  verification: { type: "command", command: "git status --porcelain" },
  tags: ["git", "add", "stage"],
  source: "bootstrap",
});

const gitCommit: Skill = createSkill({
  name: "Git Commit",
  description: "Create a commit with staged changes. Use after staging changes with git add.",
  category: "git",
  code: `// Create commit
const proc = Bun.spawn(["git", "commit", "-m", message]);
await proc.exited;`,
  parameters: [
    { name: "message", type: "string", description: "Commit message", required: true },
  ],
  verification: { type: "command", command: "git log -1 --oneline" },
  tags: ["git", "commit", "save"],
  source: "bootstrap",
});

// --- Debugging Skills ---

const analyzeError: Skill = createSkill({
  name: "Analyze Error",
  description: "Parse and analyze an error message to identify the cause and location. Use when encountering test failures or runtime errors.",
  category: "debugging",
  code: `// Parse error message
// Look for: file path, line number, error type, error message
// Common patterns:
//   TypeScript: "src/file.ts(10,5): error TS2345: ..."
//   Runtime: "Error: message\\n    at function (file.ts:10:5)"
//   Test: "expected X but got Y"`,
  parameters: [
    { name: "error", type: "string", description: "Error message or stack trace", required: true },
  ],
  verification: { type: "none" },
  tags: ["debug", "error", "analyze", "diagnose"],
  source: "bootstrap",
});

const fixImportError: Skill = createSkill({
  name: "Fix Import Error",
  description: "Fix a missing or incorrect import statement. Use when encountering 'Cannot find module' or similar errors.",
  category: "debugging",
  code: `// Fix import by adding correct import statement
// 1. Identify the missing import from error message
// 2. Find the correct module path
// 3. Add import at top of file

// Example fix:
// import { MissingThing } from './correct/path.js';`,
  parameters: [
    { name: "file", type: "path", description: "File with import error", required: true },
    { name: "missingImport", type: "string", description: "Name of missing import", required: true },
    { name: "sourcePath", type: "string", description: "Path to import from", required: true },
  ],
  verification: { type: "typecheck" },
  tags: ["debug", "import", "fix", "typescript"],
  source: "bootstrap",
});

const fixSyntaxError: Skill = createSkill({
  name: "Fix Syntax Error",
  description: "Fix a syntax error in code. Use when encountering parsing or compilation errors.",
  category: "debugging",
  code: `// Common syntax fixes:
// - Missing semicolon: add ;
// - Missing bracket: add matching {, }, [, ], (, )
// - Missing quote: add matching ' or "
// - Typo in keyword: fix spelling
// - Wrong operator: = vs ==, === vs ==`,
  parameters: [
    { name: "file", type: "path", description: "File with syntax error", required: true },
    { name: "line", type: "number", description: "Line number of error", required: true },
    { name: "errorType", type: "string", description: "Type of syntax error", required: true },
  ],
  verification: { type: "typecheck" },
  tags: ["debug", "syntax", "fix", "parse"],
  source: "bootstrap",
});

// --- Shell Skills ---

const executeCommand: Skill = createSkill({
  name: "Execute Command",
  description: "Run a shell command. Use for system operations not covered by other skills.",
  category: "shell",
  code: `// Execute shell command
const proc = Bun.spawn(command.split(" "), { cwd: workingDir });
const stdout = await new Response(proc.stdout).text();
const stderr = await new Response(proc.stderr).text();
const exitCode = await proc.exited;`,
  parameters: [
    { name: "command", type: "string", description: "Command to execute", required: true },
    { name: "workingDir", type: "path", description: "Working directory", required: false, default: "." },
  ],
  verification: { type: "none" },
  tags: ["shell", "command", "execute"],
  source: "bootstrap",
});

const checkOutput: Skill = createSkill({
  name: "Check Command Output",
  description: "Run a command and verify its output matches expected pattern. Use for verification steps.",
  category: "shell",
  code: `// Run command and check output
const proc = Bun.spawn(command.split(" "));
const output = await new Response(proc.stdout).text();
const matches = new RegExp(pattern).test(output);`,
  parameters: [
    { name: "command", type: "string", description: "Command to execute", required: true },
    { name: "pattern", type: "pattern", description: "Regex pattern to match in output", required: true },
  ],
  verification: { type: "pattern" },
  tags: ["shell", "verify", "output"],
  source: "bootstrap",
});

// --- Search Skills ---

const findFile: Skill = createSkill({
  name: "Find File",
  description: "Find a file by name or pattern. Use to locate files before reading or modifying.",
  category: "search",
  code: `// Find file by name
const glob = new Bun.Glob("**/" + filename);
const files = await Array.fromAsync(glob.scan({ cwd: searchDir }));
// Returns: array of matching paths`,
  parameters: [
    { name: "filename", type: "string", description: "File name to find", required: true },
    { name: "searchDir", type: "path", description: "Directory to search", required: false, default: "." },
  ],
  verification: { type: "none" },
  tags: ["search", "find", "file"],
  source: "bootstrap",
});

const findDefinition: Skill = createSkill({
  name: "Find Definition",
  description: "Find where a function, class, or variable is defined. Use to understand code structure.",
  category: "search",
  code: `// Search for definition patterns
// Function: "function name(" or "const name = ("
// Class: "class Name"
// Variable: "const/let/var name ="
// Type: "type Name =" or "interface Name"
const patterns = [
  \`function \${name}\\\\(\`,
  \`class \${name}\`,
  \`(const|let|var) \${name}\\\\s*=\`,
  \`(type|interface) \${name}\`,
];`,
  parameters: [
    { name: "name", type: "string", description: "Name of function/class/variable to find", required: true },
    { name: "searchDir", type: "path", description: "Directory to search", required: false, default: "." },
  ],
  verification: { type: "none" },
  tags: ["search", "definition", "code", "navigate"],
  source: "bootstrap",
});

const searchCode: Skill = createSkill({
  name: "Search Code",
  description: "Search for code patterns across the codebase. Use to find usage examples or similar patterns.",
  category: "search",
  code: `// Search code with context
const proc = Bun.spawn(["grep", "-rn", "-A", "3", "-B", "1", pattern, path]);
const output = await new Response(proc.stdout).text();
// Returns: matching lines with surrounding context`,
  parameters: [
    { name: "pattern", type: "pattern", description: "Code pattern to search", required: true },
    { name: "path", type: "path", description: "Directory to search", required: false, default: "src" },
    { name: "fileType", type: "string", description: "File extension (e.g., 'ts', 'js')", required: false },
  ],
  verification: { type: "none" },
  tags: ["search", "code", "pattern"],
  source: "bootstrap",
});

// --- Additional Primitive Skills ---

const createDirectory: Skill = createSkill({
  name: "Create Directory",
  description: "Create a new directory. Use before writing files to non-existent directories.",
  category: "file_operations",
  code: `// Create directory recursively
import { mkdir } from "fs/promises";
await mkdir(path, { recursive: true });`,
  parameters: [
    { name: "path", type: "path", description: "Directory path to create", required: true },
  ],
  verification: { type: "command", command: "test -d {path}" },
  tags: ["file", "directory", "create"],
  source: "bootstrap",
});

const deleteFile: Skill = createSkill({
  name: "Delete File",
  description: "Delete a file. Use to clean up temporary or unwanted files.",
  category: "file_operations",
  code: `// Delete file
import { unlink } from "fs/promises";
await unlink(path);`,
  parameters: [
    { name: "path", type: "path", description: "Path to file to delete", required: true },
  ],
  verification: { type: "command", command: "test ! -f {path}" },
  tags: ["file", "delete", "remove"],
  source: "bootstrap",
});

const copyFile: Skill = createSkill({
  name: "Copy File",
  description: "Copy a file to a new location. Use to duplicate files or create backups.",
  category: "file_operations",
  code: `// Copy file
import { copyFile } from "fs/promises";
await copyFile(source, destination);`,
  parameters: [
    { name: "source", type: "path", description: "Source file path", required: true },
    { name: "destination", type: "path", description: "Destination path", required: true },
  ],
  verification: { type: "command", command: "test -f {destination}" },
  tags: ["file", "copy", "duplicate"],
  source: "bootstrap",
});

const listDirectory: Skill = createSkill({
  name: "List Directory",
  description: "List contents of a directory. Use to explore directory structure.",
  category: "file_operations",
  code: `// List directory contents
import { readdir } from "fs/promises";
const entries = await readdir(path, { withFileTypes: true });
// Returns: array of { name, isDirectory(), isFile() }`,
  parameters: [
    { name: "path", type: "path", description: "Directory to list", required: true },
  ],
  verification: { type: "none" },
  tags: ["file", "list", "directory", "explore"],
  source: "bootstrap",
});

const gitLog: Skill = createSkill({
  name: "Git Log",
  description: "Show recent commit history. Use to understand recent changes.",
  category: "git",
  code: `// Show recent commits
const proc = Bun.spawn(["git", "log", "--oneline", "-n", String(count)]);
const output = await new Response(proc.stdout).text();`,
  parameters: [
    { name: "count", type: "number", description: "Number of commits to show", required: false, default: "10" },
  ],
  verification: { type: "none" },
  tags: ["git", "log", "history"],
  source: "bootstrap",
});

const gitBranch: Skill = createSkill({
  name: "Git Branch",
  description: "List or create git branches. Use for branch management.",
  category: "git",
  code: `// List branches
const proc = Bun.spawn(["git", "branch", "-a"]);
const output = await new Response(proc.stdout).text();`,
  parameters: [
    { name: "create", type: "string", description: "Branch name to create (optional)", required: false },
  ],
  verification: { type: "none" },
  tags: ["git", "branch", "version-control"],
  source: "bootstrap",
});

const installDependency: Skill = createSkill({
  name: "Install Dependency",
  description: "Install an npm package. Use when code requires a missing dependency.",
  category: "shell",
  code: `// Install package
const proc = Bun.spawn(["bun", "add", packageName]);
await proc.exited;`,
  parameters: [
    { name: "packageName", type: "string", description: "Package name to install", required: true },
    { name: "dev", type: "boolean", description: "Install as dev dependency", required: false, default: "false" },
  ],
  verification: { type: "command", command: "bun pm ls | grep {packageName}" },
  tags: ["npm", "install", "dependency", "package"],
  source: "bootstrap",
});

// --- Export All Primitive Skills ---

export const primitiveSkills: Skill[] = [
  // File Operations
  readFile,
  writeFile,
  editFile,
  globFiles,
  createDirectory,
  deleteFile,
  copyFile,
  listDirectory,

  // Search
  grepCode,
  findFile,
  findDefinition,
  searchCode,

  // Testing
  runTest,
  runTypecheck,
  runLint,

  // Git
  gitStatus,
  gitDiff,
  gitAdd,
  gitCommit,
  gitLog,
  gitBranch,

  // Debugging
  analyzeError,
  fixImportError,
  fixSyntaxError,

  // Shell
  executeCommand,
  checkOutput,
  installDependency,
];

export default primitiveSkills;
