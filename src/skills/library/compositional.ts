/**
 * Compositional Skills Library
 *
 * Higher-level skills that combine primitives for common coding patterns.
 * These skills are more complex and context-specific, achieving better
 * task completion rates by providing complete solution patterns.
 *
 * Based on Voyager research: compositional skills show 3.3x improvement.
 */

import { createSkill, type Skill } from "../schema.js";

// --- Error Fixing Skills ---

const fixTypescriptImportError = createSkill({
  name: "Fix TypeScript Import Error",
  description: "Fix missing or incorrect TypeScript imports by analyzing error messages and adding correct import statements",
  category: "debugging",
  source: "bootstrap",
  tags: ["typescript", "import", "error-fix"],
  parameters: [
    { name: "errorFile", type: "path", description: "File with the import error", required: true },
    { name: "missingSymbol", type: "string", description: "The symbol that's missing", required: true },
  ],
  code: `// 1. Analyze the error to identify the missing symbol
// 2. Search for where the symbol is exported:
const searchResult = await $\`grep -r "export.*{symbolName}" --include="*.ts" .\`.text();
// Or check common patterns:
// - Named export: export { Symbol } from './path'
// - Default export: export default Symbol
// - Type export: export type { Symbol }

// 3. Add the import at the top of the file:
const importStatement = \`import { \${symbolName} } from "\${sourcePath}";\`;
// Insert after existing imports or at file start

// 4. Run typecheck to verify fix:
await $\`bun tsc --noEmit \${errorFile}\`;`,
  examples: [
    {
      description: "Fix missing Effect import",
      input: { error: "TS2304: Cannot find name 'Effect'" },
      output: "import { Effect } from 'effect';",
    },
  ],
});

const fixTypescriptTypeError = createSkill({
  name: "Fix TypeScript Type Error",
  description: "Resolve TypeScript type mismatches by analyzing expected vs actual types and applying correct fixes",
  category: "debugging",
  source: "bootstrap",
  tags: ["typescript", "types", "error-fix"],
  parameters: [
    { name: "errorFile", type: "path", description: "File with the type error", required: true },
    { name: "errorLine", type: "number", description: "Line number of the error", required: true },
  ],
  code: `// Common type error patterns and fixes:

// Pattern 1: Type 'X' is not assignable to type 'Y'
// - Check if types are compatible (structural typing)
// - Use type assertion: value as Type
// - Use type guard: if (isType(value)) { ... }

// Pattern 2: Property 'X' does not exist on type 'Y'
// - Add optional chaining: obj?.property
// - Add type declaration: (obj as ExtendedType).property
// - Extend the interface/type

// Pattern 3: Argument of type 'X' is not assignable
// - Check function signature
// - Create proper type conversion
// - Use satisfies operator for better inference

// Pattern 4: exactOptionalPropertyTypes issues
// - Add undefined to optional property types: prop?: string | undefined
// - Or explicitly set the property rather than spreading

// After fix, verify with:
await $\`bun tsc --noEmit\`;`,
});

const fixSyntaxError = createSkill({
  name: "Fix Syntax Error",
  description: "Fix JavaScript/TypeScript syntax errors by analyzing parser output and correcting code structure",
  category: "debugging",
  source: "bootstrap",
  tags: ["syntax", "error-fix"],
  parameters: [
    { name: "errorFile", type: "path", description: "File with syntax error", required: true },
    { name: "errorLine", type: "number", description: "Line where error occurs", required: true },
  ],
  code: `// Common syntax error patterns:

// 1. Missing/extra brackets: Look for unbalanced {}, [], ()
// 2. Missing semicolons (though TypeScript usually handles this)
// 3. Incorrect template literals: Use backticks for \${...}
// 4. Arrow function issues: Check => placement
// 5. Object literal issues: Check for trailing commas in JSON

// Strategy:
// 1. Read the error message carefully for the expected token
// 2. Look at surrounding context (2-3 lines before/after)
// 3. Check for matching pairs of brackets
// 4. Verify string/template literal termination

const content = await Bun.file(errorFile).text();
const lines = content.split("\\n");
const errorContext = lines.slice(errorLine - 3, errorLine + 2);
// Analyze and fix the syntax issue`,
});

// --- Testing Skills ---

const addTestForFunction = createSkill({
  name: "Add Test for Function",
  description: "Create a test file or add tests to existing test file for a function",
  category: "testing",
  source: "bootstrap",
  tags: ["test", "bun", "vitest"],
  parameters: [
    { name: "functionFile", type: "path", description: "File containing the function", required: true },
    { name: "functionName", type: "string", description: "Name of the function to test", required: true },
  ],
  code: `// 1. Read the function to understand its signature and behavior
const sourceContent = await Bun.file(functionFile).text();
// Extract function signature, parameters, return type

// 2. Create or locate test file
const testFile = functionFile.replace(/\\.ts$/, '.test.ts');
const existingTests = await Bun.file(testFile).text().catch(() => '');

// 3. Generate test structure
const testTemplate = \`
import { describe, test, expect } from "bun:test";
import { \${functionName} } from "./\${basename(functionFile, '.ts')}";

describe("\${functionName}", () => {
  test("handles basic case", () => {
    // TODO: Add expected input/output
    const result = \${functionName}(/* args */);
    expect(result).toBe(/* expected */);
  });

  test("handles edge cases", () => {
    // TODO: Add edge case tests
  });

  test("throws on invalid input", () => {
    expect(() => \${functionName}(/* invalid */)).toThrow();
  });
});
\`;

// 4. Write and run tests
await Bun.write(testFile, existingTests + testTemplate);
await $\`bun test \${testFile}\`;`,
});

const runTestsWithCoverage = createSkill({
  name: "Run Tests with Coverage",
  description: "Run test suite and analyze code coverage, identifying untested paths",
  category: "testing",
  source: "bootstrap",
  tags: ["test", "coverage"],
  code: `// Run tests with coverage enabled
const result = await $\`bun test --coverage\`.text();

// Parse coverage output
// Look for:
// - File coverage percentages
// - Uncovered lines
// - Branch coverage

// Generate coverage report
console.log("Coverage Results:");
console.log(result);

// Identify files below threshold (e.g., 80%)
// Suggest adding tests for uncovered code`,
});

const fixFailingTest = createSkill({
  name: "Fix Failing Test",
  description: "Analyze and fix a failing test by understanding the mismatch between expected and actual values",
  category: "testing",
  source: "bootstrap",
  tags: ["test", "debugging"],
  parameters: [
    { name: "testFile", type: "path", description: "The failing test file", required: true },
  ],
  code: `// 1. Run the specific test to see failure output
const output = await $\`bun test \${testFile}\`.text();

// 2. Parse error to find:
// - Expected value
// - Actual value
// - Test location

// 3. Determine if the issue is:
// a) Test is wrong (update expected value)
// b) Code is wrong (fix implementation)
// c) Test setup is incomplete (add mocks/fixtures)

// 4. Common fixes:
// - Snapshot updates: bun test --update-snapshots
// - Async timing: Add await or adjust timeout
// - Mock data: Ensure test data matches expected structure

// 5. Re-run to verify fix
await $\`bun test \${testFile}\`;`,
});

// --- Git Workflow Skills ---

const createFeatureBranch = createSkill({
  name: "Create Feature Branch",
  description: "Create a new feature branch following project conventions and push to remote",
  category: "git",
  source: "bootstrap",
  tags: ["git", "branch", "workflow"],
  parameters: [
    { name: "featureName", type: "string", description: "Short name for the feature", required: true },
  ],
  code: `// 1. Ensure clean working directory
const status = await $\`git status --porcelain\`.text();
if (status.trim()) {
  console.log("Working directory not clean. Stash or commit changes first.");
  return;
}

// 2. Update main branch
await $\`git checkout main && git pull origin main\`;

// 3. Create feature branch with conventional name
const branchName = \`feat/\${featureName.toLowerCase().replace(/\\s+/g, '-')}\`;
await $\`git checkout -b \${branchName}\`;

// 4. Push and set upstream
await $\`git push -u origin \${branchName}\`;

console.log(\`Created and pushed branch: \${branchName}\`);`,
});

const createPullRequest = createSkill({
  name: "Create Pull Request",
  description: "Create a GitHub PR with proper title, description, and reviewers using gh CLI",
  category: "git",
  source: "bootstrap",
  tags: ["git", "github", "pr"],
  parameters: [
    { name: "title", type: "string", description: "PR title", required: true },
    { name: "body", type: "string", description: "PR description", required: false },
  ],
  code: `// 1. Ensure all changes are committed
const status = await $\`git status --porcelain\`.text();
if (status.trim()) {
  await $\`git add -A && git commit -m "WIP: pending changes"\`;
}

// 2. Push current branch
const branch = (await $\`git branch --show-current\`.text()).trim();
await $\`git push -u origin \${branch}\`;

// 3. Create PR using gh CLI
const prBody = body || \`
## Summary
- Add summary of changes here

## Test Plan
- [ ] Unit tests pass
- [ ] Manual testing completed

\`;

await $\`gh pr create --title "\${title}" --body "\${prBody}"\`;`,
});

const resolveGitConflict = createSkill({
  name: "Resolve Git Conflict",
  description: "Identify and resolve git merge conflicts by analyzing conflict markers and choosing appropriate resolution",
  category: "git",
  source: "bootstrap",
  tags: ["git", "merge", "conflict"],
  parameters: [
    { name: "conflictFile", type: "path", description: "File with merge conflict", required: true },
  ],
  code: `// 1. Read file and identify conflict markers
const content = await Bun.file(conflictFile).text();
const hasConflicts = content.includes('<<<<<<<');

// 2. Parse conflict sections
// <<<<<<< HEAD (current branch)
// =======
// >>>>>>> branch-name (incoming)

// 3. Resolution strategies:
// a) Keep ours: git checkout --ours <file>
// b) Keep theirs: git checkout --theirs <file>
// c) Manual merge: Edit file to combine changes

// 4. After resolving, mark as resolved
await $\`git add \${conflictFile}\`;

// 5. Continue merge/rebase if applicable
// git merge --continue or git rebase --continue`,
});

// --- Refactoring Skills ---

const extractFunction = createSkill({
  name: "Extract Function",
  description: "Extract a block of code into a reusable function with proper parameters and return type",
  category: "refactoring",
  source: "bootstrap",
  tags: ["refactor", "function", "code-quality"],
  parameters: [
    { name: "sourceFile", type: "path", description: "File containing code to extract", required: true },
    { name: "startLine", type: "number", description: "Start line of code block", required: true },
    { name: "endLine", type: "number", description: "End line of code block", required: true },
    { name: "functionName", type: "string", description: "Name for the new function", required: true },
  ],
  code: `// 1. Read the source file
const content = await Bun.file(sourceFile).text();
const lines = content.split("\\n");

// 2. Extract the code block
const codeBlock = lines.slice(startLine - 1, endLine).join("\\n");

// 3. Analyze dependencies
// - Variables used but not declared (become parameters)
// - Variables declared and used later (become return values)
// - External imports needed

// 4. Generate function signature
const func = \`
export function \${functionName}(/* params */): /* returnType */ {
  \${codeBlock}
}
\`;

// 5. Replace original code with function call
// 6. Add function to appropriate location (same file or new module)
// 7. Update imports if extracted to new file`,
});

const renameSymbol = createSkill({
  name: "Rename Symbol",
  description: "Safely rename a function, variable, or type across the entire codebase",
  category: "refactoring",
  source: "bootstrap",
  tags: ["refactor", "rename"],
  parameters: [
    { name: "oldName", type: "string", description: "Current symbol name", required: true },
    { name: "newName", type: "string", description: "New symbol name", required: true },
  ],
  code: `// 1. Find all occurrences
const occurrences = await $\`grep -rn "\\b\${oldName}\\b" --include="*.ts" --include="*.tsx" .\`.text();

// 2. Categorize by type:
// - Declarations (function/class/type/interface)
// - Imports/exports
// - References

// 3. Perform replacements in correct order:
// a) Update exports first
// b) Update imports
// c) Update local references

// 4. Use word boundary matching to avoid partial matches
await $\`find . -name "*.ts" -o -name "*.tsx" | xargs sed -i '' 's/\\b\${oldName}\\b/\${newName}/g'\`;

// 5. Verify with typecheck
await $\`bun tsc --noEmit\`;`,
});

const convertToEffect = createSkill({
  name: "Convert to Effect",
  description: "Convert Promise-based async code to Effect-TS patterns with proper error handling",
  category: "refactoring",
  source: "bootstrap",
  tags: ["effect", "refactor", "async"],
  parameters: [
    { name: "sourceFile", type: "path", description: "File to convert", required: true },
  ],
  code: `// Effect-TS conversion patterns:

// 1. Promise to Effect
// Before: async function foo(): Promise<T>
// After:  const foo = (): Effect.Effect<T, Error> =>

// 2. try/catch to Effect.gen
// Before:
// try { const x = await something(); } catch (e) { ... }
// After:
// Effect.gen(function* () {
//   const x = yield* Effect.tryPromise(() => something());
// })

// 3. Error types
// Before: throw new Error("...")
// After: yield* Effect.fail(new CustomError("..."))

// 4. Services/Context
// Before: const db = new Database();
// After: const db = yield* Database;

// 5. Layer composition
// Build layers for dependency injection

// Common imports needed:
// import { Effect, Context, Layer, pipe } from "effect";`,
});

// --- Code Generation Skills ---

const generateTypeFromJson = createSkill({
  name: "Generate Type from JSON",
  description: "Generate TypeScript type definitions from JSON data samples",
  category: "file_operations",
  source: "bootstrap",
  tags: ["typescript", "types", "json"],
  parameters: [
    { name: "jsonSample", type: "string", description: "JSON string or path to JSON file", required: true },
    { name: "typeName", type: "string", description: "Name for the generated type", required: true },
  ],
  code: `// 1. Parse JSON
const data = JSON.parse(jsonSample);

// 2. Infer types from values
function inferType(value: unknown): string {
  if (value === null) return 'null';
  if (Array.isArray(value)) {
    if (value.length === 0) return 'unknown[]';
    return \`\${inferType(value[0])}[]\`;
  }
  if (typeof value === 'object') {
    const props = Object.entries(value)
      .map(([k, v]) => \`  \${k}: \${inferType(v)};\`)
      .join('\\n');
    return \`{\\n\${props}\\n}\`;
  }
  return typeof value;
}

// 3. Generate type definition
const typeDef = \`export interface \${typeName} \${inferType(data)}\`;

console.log(typeDef);`,
});

const scaffoldComponent = createSkill({
  name: "Scaffold React Component",
  description: "Create a new React component with proper TypeScript types, tests, and story files",
  category: "file_operations",
  source: "bootstrap",
  tags: ["react", "component", "scaffold"],
  parameters: [
    { name: "componentName", type: "string", description: "Name of the component (PascalCase)", required: true },
    { name: "componentDir", type: "path", description: "Directory for the component", required: true },
  ],
  code: `// Create component directory structure
const dir = \`\${componentDir}/\${componentName}\`;
await $\`mkdir -p \${dir}\`;

// Component file
const componentCode = \`
import React from 'react';

export interface \${componentName}Props {
  // Add props here
}

export function \${componentName}({ }: \${componentName}Props) {
  return (
    <div>
      {\${componentName}}
    </div>
  );
}
\`;
await Bun.write(\`\${dir}/\${componentName}.tsx\`, componentCode);

// Test file
const testCode = \`
import { describe, test, expect } from "bun:test";
import { render } from "@testing-library/react";
import { \${componentName} } from "./\${componentName}";

describe("\${componentName}", () => {
  test("renders without crashing", () => {
    const { container } = render(<\${componentName} />);
    expect(container).toBeTruthy();
  });
});
\`;
await Bun.write(\`\${dir}/\${componentName}.test.tsx\`, testCode);

// Index file for easy imports
await Bun.write(\`\${dir}/index.ts\`, \`export * from './\${componentName}';\`);`,
});

const generateApiClient = createSkill({
  name: "Generate API Client",
  description: "Generate a type-safe API client from OpenAPI spec or endpoint definitions",
  category: "file_operations",
  source: "bootstrap",
  tags: ["api", "typescript", "client"],
  parameters: [
    { name: "baseUrl", type: "string", description: "Base URL for the API", required: true },
    { name: "endpoints", type: "string", description: "JSON array of endpoint definitions", required: true },
  ],
  code: `// Generate typed API client

const clientCode = \`
const BASE_URL = "\${baseUrl}";

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(\`\${BASE_URL}\${path}\`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
  });
  if (!res.ok) throw new Error(\`API error: \${res.status}\`);
  return res.json();
}

export const api = {
  // Generated from endpoints
  get: <T>(path: string) => request<T>(path, { method: 'GET' }),
  post: <T>(path: string, body: unknown) => request<T>(path, {
    method: 'POST',
    body: JSON.stringify(body),
  }),
  put: <T>(path: string, body: unknown) => request<T>(path, {
    method: 'PUT',
    body: JSON.stringify(body),
  }),
  delete: (path: string) => request<void>(path, { method: 'DELETE' }),
};
\`;

await Bun.write("api-client.ts", clientCode);`,
});

// --- Performance Skills ---

const profileCode = createSkill({
  name: "Profile Code Performance",
  description: "Profile code execution to identify performance bottlenecks",
  category: "debugging",
  source: "bootstrap",
  tags: ["performance", "profiling"],
  parameters: [
    { name: "targetFile", type: "path", description: "File to profile", required: true },
  ],
  code: `// Bun built-in profiling

// 1. Run with profiling enabled
await $\`bun --smol \${targetFile}\`;

// 2. Use console.time for specific sections
// console.time('operation');
// ... code ...
// console.timeEnd('operation');

// 3. Memory profiling
// const mem1 = process.memoryUsage();
// ... code ...
// const mem2 = process.memoryUsage();
// console.log('Memory delta:', mem2.heapUsed - mem1.heapUsed);

// 4. Identify hot paths
// - Look for loops with many iterations
// - Check for unnecessary allocations
// - Review async operation batching`,
});

const optimizeImports = createSkill({
  name: "Optimize Imports",
  description: "Remove unused imports and organize remaining imports by type",
  category: "refactoring",
  source: "bootstrap",
  tags: ["imports", "cleanup", "performance"],
  parameters: [
    { name: "targetFile", type: "path", description: "File to optimize", required: true },
  ],
  code: `// 1. Identify unused imports using TypeScript compiler
// bun tsc --noEmit will report unused imports with noUnusedLocals

// 2. Parse imports
const content = await Bun.file(targetFile).text();
const importLines = content.match(/^import .* from .*;?$/gm) || [];

// 3. Categorize imports:
// - Node built-ins (node:*)
// - External packages
// - Internal aliases (@/*)
// - Relative imports (./)

// 4. Sort and group
// - Remove duplicates
// - Combine named imports from same source
// - Order: built-ins, external, internal, relative

// 5. Rewrite import section`,
});

// --- Documentation Skills ---

const generateJsDoc = createSkill({
  name: "Generate JSDoc",
  description: "Generate JSDoc comments for functions and classes based on their implementation",
  category: "documentation",
  source: "bootstrap",
  tags: ["jsdoc", "documentation"],
  parameters: [
    { name: "targetFile", type: "path", description: "File to document", required: true },
  ],
  code: `// 1. Parse file for function/class declarations
const content = await Bun.file(targetFile).text();

// 2. For each declaration, generate JSDoc:
// /**
//  * Description based on function name and body
//  * @param {Type} name - Description
//  * @returns {Type} Description
//  * @throws {Error} When condition
//  * @example
//  * const result = functionName(arg);
//  */

// 3. Consider:
// - Parameter types and names
// - Return type
// - Thrown errors
// - Side effects
// - Usage examples`,
});

// --- Security Skills ---

const auditDependencies = createSkill({
  name: "Audit Dependencies",
  description: "Check dependencies for known vulnerabilities and outdated packages",
  category: "security",
  source: "bootstrap",
  tags: ["security", "dependencies", "audit"],
  code: `// 1. Check for vulnerabilities
await $\`bun pm audit\`;

// 2. List outdated packages
await $\`bun outdated\`;

// 3. Review package.json for:
// - Pinned versions vs ranges
// - Unused dependencies
// - Dev vs prod dependencies

// 4. Check for malicious packages
// - Verify package publishers
// - Check download counts
// - Review changelogs for major updates`,
});

const sanitizeInput = createSkill({
  name: "Sanitize User Input",
  description: "Add proper input validation and sanitization for user-provided data",
  category: "security",
  source: "bootstrap",
  tags: ["security", "validation", "input"],
  parameters: [
    { name: "inputType", type: "string", description: "Type of input (string, email, url, etc.)", required: true },
  ],
  code: `// Input sanitization patterns:

// 1. String sanitization
const sanitizeString = (input: string): string => {
  return input
    .trim()
    .replace(/[<>]/g, '') // Remove HTML tags
    .slice(0, 1000); // Limit length
};

// 2. Email validation
const isValidEmail = (email: string): boolean => {
  const pattern = /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/;
  return pattern.test(email);
};

// 3. URL validation
const isValidUrl = (url: string): boolean => {
  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
};

// 4. SQL injection prevention
// Use parameterized queries, never string concatenation

// 5. Path traversal prevention
const safePath = (path: string): string => {
  return path.replace(/\\.\\.\\//g, '').replace(/^\\//, '');
};`,
});

// --- Build Skills ---

const setupBunProject = createSkill({
  name: "Setup Bun Project",
  description: "Initialize a new Bun project with TypeScript, testing, and common configurations",
  category: "file_operations",
  source: "bootstrap",
  tags: ["bun", "setup", "project"],
  parameters: [
    { name: "projectName", type: "string", description: "Name of the project", required: true },
  ],
  code: `// 1. Initialize project
await $\`mkdir -p \${projectName} && cd \${projectName}\`;
await $\`bun init -y\`;

// 2. Add TypeScript config
const tsconfig = {
  compilerOptions: {
    target: "ES2022",
    module: "ESNext",
    moduleResolution: "bundler",
    strict: true,
    skipLibCheck: true,
    types: ["bun-types"],
  },
};
await Bun.write("tsconfig.json", JSON.stringify(tsconfig, null, 2));

// 3. Add common dependencies
await $\`bun add -d typescript bun-types\`;

// 4. Create source structure
await $\`mkdir -p src tests\`;
await Bun.write("src/index.ts", 'console.log("Hello from Bun!");');

// 5. Add scripts to package.json`,
});

const createDockerfile = createSkill({
  name: "Create Dockerfile",
  description: "Generate an optimized Dockerfile for Bun applications",
  category: "file_operations",
  source: "bootstrap",
  tags: ["docker", "deployment"],
  code: `const dockerfile = \`
FROM oven/bun:1 as builder

WORKDIR /app
COPY package.json bun.lockb ./
RUN bun install --frozen-lockfile

COPY . .
RUN bun run build

FROM oven/bun:1-slim
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/package.json ./

USER bun
EXPOSE 3000
CMD ["bun", "run", "dist/index.js"]
\`;

await Bun.write("Dockerfile", dockerfile);

// Also create .dockerignore
const dockerignore = \`
node_modules
.git
*.log
.env*
dist
\`;
await Bun.write(".dockerignore", dockerignore);`,
});

// --- Effect-TS Skills ---

const createEffectService = createSkill({
  name: "Create Effect Service",
  description: "Create an Effect-TS service with proper Context.Tag, interface, implementation, and Layer",
  category: "file_operations",
  source: "bootstrap",
  tags: ["effect", "service", "dependency-injection"],
  parameters: [
    { name: "serviceName", type: "string", description: "Name of the service (PascalCase)", required: true },
  ],
  code: `// Effect Service Template

const serviceTemplate = \`
import { Effect, Context, Layer } from "effect";

// --- Interface ---

export interface I\${serviceName} {
  readonly doSomething: (input: string) => Effect.Effect<string, \${serviceName}Error>;
}

// --- Error Type ---

export class \${serviceName}Error extends Error {
  readonly _tag = "\${serviceName}Error";
  constructor(
    readonly reason: string,
    message: string,
    readonly cause?: Error,
  ) {
    super(message);
    this.name = "\${serviceName}Error";
  }
}

// --- Service Tag ---

export class \${serviceName} extends Context.Tag("\${serviceName}")<
  \${serviceName},
  I\${serviceName}
>() {}

// --- Implementation ---

const make\${serviceName} = (): I\${serviceName} => ({
  doSomething: (input) =>
    Effect.gen(function* () {
      // Implementation here
      return \\\`Processed: \\\${input}\\\`;
    }),
});

// --- Layer ---

export const \${serviceName}Live: Layer.Layer<\${serviceName}> =
  Layer.succeed(\${serviceName}, make\${serviceName}());
\`;

await Bun.write(\`src/\${serviceName.toLowerCase()}.ts\`, serviceTemplate);`,
});

const handleEffectError = createSkill({
  name: "Handle Effect Error",
  description: "Add proper error handling and recovery to Effect-TS code",
  category: "file_operations",
  source: "bootstrap",
  tags: ["effect", "error-handling"],
  code: `// Effect error handling patterns:

// 1. Map error types
Effect.mapError((e) => new MyError("wrapped", e.message));

// 2. Catch specific errors
Effect.catchTag("NetworkError", (e) =>
  Effect.succeed(fallbackValue)
);

// 3. Catch all errors
Effect.catchAll((e) =>
  Effect.logError("Failed", e).pipe(
    Effect.flatMap(() => Effect.fail(e))
  )
);

// 4. Retry with backoff
Effect.retry(
  Schedule.exponential("100 millis").pipe(
    Schedule.compose(Schedule.recurs(3))
  )
);

// 5. Provide fallback
Effect.orElse(() => Effect.succeed(defaultValue));

// 6. Ensure cleanup
Effect.ensuring(cleanup());`,
});

// --- Database Skills ---

const createDatabaseMigration = createSkill({
  name: "Create Database Migration",
  description: "Create a database migration file with up and down functions",
  category: "file_operations",
  source: "bootstrap",
  tags: ["database", "migration", "sqlite"],
  parameters: [
    { name: "migrationName", type: "string", description: "Name describing the migration", required: true },
  ],
  code: `// Create timestamped migration file
const timestamp = Date.now();
const fileName = \`\${timestamp}_\${migrationName.toLowerCase().replace(/\\s+/g, '_')}.ts\`;

const migrationTemplate = \`
import { Database } from "bun:sqlite";

export const up = (db: Database) => {
  db.run(\\\`
    CREATE TABLE IF NOT EXISTS table_name (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      created_at TEXT DEFAULT CURRENT_TIMESTAMP,
      updated_at TEXT DEFAULT CURRENT_TIMESTAMP
    )
  \\\`);
};

export const down = (db: Database) => {
  db.run(\\\`DROP TABLE IF EXISTS table_name\\\`);
};
\`;

await Bun.write(\`migrations/\${fileName}\`, migrationTemplate);
console.log(\`Created migration: migrations/\${fileName}\`);`,
});

// --- Additional Debugging Skills ---

const debugAsyncIssue = createSkill({
  name: "Debug Async Issue",
  description: "Debug async/await issues by tracing promise chains and identifying race conditions",
  category: "debugging",
  source: "bootstrap",
  tags: ["async", "debugging", "promises"],
  code: `// Common async issues and fixes:

// 1. Missing await
// Before: const data = fetchData();
// After: const data = await fetchData();

// 2. Race conditions
// Use Promise.all for parallel, sequential for dependencies
const [a, b] = await Promise.all([fetchA(), fetchB()]);
const c = await fetchC(a); // depends on a

// 3. Unhandled rejections
process.on('unhandledRejection', (reason) => {
  console.error('Unhandled:', reason);
});

// 4. Timeout handling
const withTimeout = <T>(promise: Promise<T>, ms: number): Promise<T> =>
  Promise.race([
    promise,
    new Promise((_, reject) =>
      setTimeout(() => reject(new Error('Timeout')), ms)
    )
  ]);`,
});

const traceMemoryLeak = createSkill({
  name: "Trace Memory Leak",
  description: "Identify and fix memory leaks by analyzing object retention and garbage collection",
  category: "debugging",
  source: "bootstrap",
  tags: ["memory", "debugging", "performance"],
  code: `// Memory leak detection patterns:

// 1. Track heap snapshots
const before = process.memoryUsage().heapUsed;
// ... operation ...
const after = process.memoryUsage().heapUsed;
console.log('Memory delta:', (after - before) / 1024 / 1024, 'MB');

// 2. Common leak sources:
// - Event listeners not removed
// - Closures capturing large objects
// - Growing arrays/maps without cleanup
// - Circular references

// 3. Fix patterns:
// - Use WeakMap/WeakSet for caches
// - Implement dispose/cleanup methods
// - Remove event listeners in cleanup
// - Use AbortController for cancellation`,
});

// --- More Testing Skills ---

const mockDependency = createSkill({
  name: "Mock Dependency",
  description: "Create mocks and stubs for external dependencies in tests",
  category: "testing",
  source: "bootstrap",
  tags: ["test", "mock", "stub"],
  parameters: [
    { name: "modulePath", type: "string", description: "Path to module to mock", required: true },
  ],
  code: `import { mock, spyOn } from "bun:test";

// 1. Mock entire module
mock.module("./database", () => ({
  query: () => Promise.resolve([{ id: 1 }]),
  connect: () => Promise.resolve(),
}));

// 2. Spy on method
const spy = spyOn(object, "method");
// Later: expect(spy).toHaveBeenCalledWith("arg");

// 3. Mock with implementation
const mockFn = mock(() => "mocked value");

// 4. Restore after test
afterEach(() => {
  mock.restore();
});`,
});

const writeIntegrationTest = createSkill({
  name: "Write Integration Test",
  description: "Create integration tests that test multiple components working together",
  category: "testing",
  source: "bootstrap",
  tags: ["test", "integration"],
  code: `import { describe, test, expect, beforeAll, afterAll } from "bun:test";

describe("Integration: Feature X", () => {
  let server: ReturnType<typeof Bun.serve>;
  let db: Database;

  beforeAll(async () => {
    // Setup test fixtures
    db = new Database(":memory:");
    server = Bun.serve({
      port: 0, // Random port
      fetch: handler,
    });
  });

  afterAll(async () => {
    // Cleanup
    server.stop();
    db.close();
  });

  test("complete user flow", async () => {
    // 1. Create user
    const createRes = await fetch(\`http://localhost:\${server.port}/users\`, {
      method: "POST",
      body: JSON.stringify({ name: "Test" }),
    });
    expect(createRes.status).toBe(201);

    // 2. Get user
    const getRes = await fetch(\`http://localhost:\${server.port}/users/1\`);
    const user = await getRes.json();
    expect(user.name).toBe("Test");
  });
});`,
});

const writeSnapshotTest = createSkill({
  name: "Write Snapshot Test",
  description: "Create snapshot tests for complex output validation",
  category: "testing",
  source: "bootstrap",
  tags: ["test", "snapshot"],
  code: `import { test, expect } from "bun:test";

test("component renders correctly", () => {
  const output = renderComponent({ prop: "value" });
  expect(output).toMatchSnapshot();
});

// Update snapshots when output changes intentionally:
// bun test --update-snapshots

// Tips:
// - Use for complex objects/structures
// - Review snapshot diffs carefully
// - Keep snapshots small and focused`,
});

// --- More Git Skills ---

const cherryPickCommit = createSkill({
  name: "Cherry Pick Commit",
  description: "Apply specific commits from one branch to another",
  category: "git",
  source: "bootstrap",
  tags: ["git", "cherry-pick"],
  parameters: [
    { name: "commitSha", type: "string", description: "SHA of commit to cherry-pick", required: true },
  ],
  code: `// 1. Switch to target branch
await $\`git checkout target-branch\`;

// 2. Cherry-pick the commit
await $\`git cherry-pick \${commitSha}\`;

// 3. If conflicts occur:
// - Resolve conflicts in affected files
// - git add <resolved-files>
// - git cherry-pick --continue

// 4. To abort if needed:
// git cherry-pick --abort`,
});

const bisectBug = createSkill({
  name: "Git Bisect Bug",
  description: "Use git bisect to find the commit that introduced a bug",
  category: "git",
  source: "bootstrap",
  tags: ["git", "debugging", "bisect"],
  code: `// 1. Start bisect
await $\`git bisect start\`;

// 2. Mark current (bad) commit
await $\`git bisect bad\`;

// 3. Mark known good commit
await $\`git bisect good <commit-sha>\`;

// 4. Test each suggested commit:
// - Run your test
// - git bisect good OR git bisect bad

// 5. Automate with script:
await $\`git bisect run bun test path/to/failing-test.ts\`;

// 6. When done:
await $\`git bisect reset\`;`,
});

const revertCommit = createSkill({
  name: "Revert Commit",
  description: "Safely revert a commit while preserving history",
  category: "git",
  source: "bootstrap",
  tags: ["git", "revert"],
  parameters: [
    { name: "commitSha", type: "string", description: "SHA of commit to revert", required: true },
  ],
  code: `// 1. Revert creates a new commit that undoes changes
await $\`git revert \${commitSha}\`;

// 2. If reverting a merge commit:
await $\`git revert -m 1 \${commitSha}\`;
// -m 1 means keep the first parent (usually main branch)

// 3. To revert without committing (for review):
await $\`git revert --no-commit \${commitSha}\`;

// 4. After review, commit:
await $\`git commit -m "Revert: reason for reverting"\`;`,
});

// --- API Skills ---

const handleApiErrors = createSkill({
  name: "Handle API Errors",
  description: "Implement proper error handling for API endpoints with status codes",
  category: "file_operations",
  source: "bootstrap",
  tags: ["api", "error-handling", "http"],
  code: `// API Error handling pattern

class ApiError extends Error {
  constructor(
    public statusCode: number,
    message: string,
    public details?: unknown,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

// Error response helper
const errorResponse = (error: ApiError) =>
  new Response(
    JSON.stringify({
      error: error.message,
      details: error.details,
    }),
    {
      status: error.statusCode,
      headers: { 'Content-Type': 'application/json' },
    }
  );

// Usage in handler
try {
  // ... handle request
} catch (e) {
  if (e instanceof ApiError) {
    return errorResponse(e);
  }
  return errorResponse(new ApiError(500, 'Internal Server Error'));
}`,
});

const validateApiInput = createSkill({
  name: "Validate API Input",
  description: "Validate request body, query params, and headers for API endpoints",
  category: "file_operations",
  source: "bootstrap",
  tags: ["api", "validation", "security"],
  code: `// API Input validation pattern

interface ValidationResult<T> {
  success: boolean;
  data?: T;
  errors?: string[];
}

function validateBody<T>(
  body: unknown,
  schema: {
    [K in keyof T]: (value: unknown) => value is T[K];
  }
): ValidationResult<T> {
  if (!body || typeof body !== 'object') {
    return { success: false, errors: ['Body must be an object'] };
  }

  const errors: string[] = [];
  const result = {} as T;

  for (const [key, validator] of Object.entries(schema)) {
    const value = (body as Record<string, unknown>)[key];
    if (!validator(value)) {
      errors.push(\`Invalid \${key}\`);
    } else {
      (result as Record<string, unknown>)[key] = value;
    }
  }

  return errors.length ? { success: false, errors } : { success: true, data: result };
}`,
});

const implementRateLimit = createSkill({
  name: "Implement Rate Limiting",
  description: "Add rate limiting to API endpoints to prevent abuse",
  category: "file_operations",
  source: "bootstrap",
  tags: ["api", "security", "rate-limit"],
  code: `// Simple in-memory rate limiter

const rateLimits = new Map<string, { count: number; resetAt: number }>();

function rateLimit(
  key: string,
  limit: number = 100,
  windowMs: number = 60000
): { allowed: boolean; remaining: number; resetAt: number } {
  const now = Date.now();
  let entry = rateLimits.get(key);

  if (!entry || now > entry.resetAt) {
    entry = { count: 0, resetAt: now + windowMs };
    rateLimits.set(key, entry);
  }

  entry.count++;

  return {
    allowed: entry.count <= limit,
    remaining: Math.max(0, limit - entry.count),
    resetAt: entry.resetAt,
  };
}

// Usage in handler
const clientIp = request.headers.get('x-forwarded-for') || 'unknown';
const { allowed, remaining, resetAt } = rateLimit(clientIp);

if (!allowed) {
  return new Response('Too Many Requests', {
    status: 429,
    headers: {
      'X-RateLimit-Remaining': String(remaining),
      'X-RateLimit-Reset': String(resetAt),
    },
  });
}`,
});

// --- More Refactoring Skills ---

const splitLargeFile = createSkill({
  name: "Split Large File",
  description: "Split a large file into smaller, focused modules while maintaining imports",
  category: "refactoring",
  source: "bootstrap",
  tags: ["refactor", "modules", "organization"],
  code: `// File splitting strategy:

// 1. Identify logical groups
// - Types/interfaces
// - Constants
// - Helper functions
// - Main exports

// 2. Create new files for each group
// original/types.ts - Type definitions
// original/utils.ts - Helper functions
// original/index.ts - Main exports + re-exports

// 3. Update imports in original file to use new modules
// import { Type } from './types';
// import { helper } from './utils';

// 4. Create barrel export (index.ts)
export * from './types';
export * from './utils';
export { mainFunction } from './main';

// 5. Update all external imports to use the barrel`,
});

const inlineAbstraction = createSkill({
  name: "Inline Abstraction",
  description: "Remove premature abstractions by inlining rarely-used helpers",
  category: "refactoring",
  source: "bootstrap",
  tags: ["refactor", "simplify"],
  code: `// When to inline:
// - Helper is used only once
// - Abstraction adds more complexity than it removes
// - Name doesn't add clarity

// Steps:
// 1. Find all usages of the abstraction
const usages = await $\`grep -rn "helperName" --include="*.ts" .\`.text();

// 2. If used only once, inline the implementation
// 3. If used 2-3 times but simple, consider inlining

// 4. Remove the helper function after inlining all usages
// 5. Run typecheck to verify
await $\`bun tsc --noEmit\`;`,
});

// --- CLI Skills ---

const parseCliArgs = createSkill({
  name: "Parse CLI Arguments",
  description: "Parse command line arguments with flags, options, and positional args",
  category: "shell",
  source: "bootstrap",
  tags: ["cli", "args", "parsing"],
  code: `// Simple CLI argument parser

interface CliOptions {
  flags: Record<string, boolean>;
  options: Record<string, string>;
  positional: string[];
}

function parseArgs(args: string[]): CliOptions {
  const result: CliOptions = { flags: {}, options: {}, positional: [] };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    if (arg.startsWith('--')) {
      const key = arg.slice(2);
      if (args[i + 1] && !args[i + 1].startsWith('-')) {
        result.options[key] = args[++i];
      } else {
        result.flags[key] = true;
      }
    } else if (arg.startsWith('-')) {
      result.flags[arg.slice(1)] = true;
    } else {
      result.positional.push(arg);
    }
  }

  return result;
}

// Usage:
const { flags, options, positional } = parseArgs(Bun.argv.slice(2));`,
});

const createCliCommand = createSkill({
  name: "Create CLI Command",
  description: "Create a new CLI command with help text and argument validation",
  category: "shell",
  source: "bootstrap",
  tags: ["cli", "command"],
  parameters: [
    { name: "commandName", type: "string", description: "Name of the command", required: true },
  ],
  code: `// CLI command template

const HELP = \`
Usage: bun \${commandName} [options] <arg>

Options:
  --help, -h     Show this help
  --verbose, -v  Verbose output
  --output, -o   Output file path

Examples:
  bun \${commandName} input.txt
  bun \${commandName} --output result.txt input.txt
\`;

const args = Bun.argv.slice(2);

if (args.includes('--help') || args.includes('-h')) {
  console.log(HELP);
  process.exit(0);
}

// Parse and validate arguments
const verbose = args.includes('--verbose') || args.includes('-v');
const outputIndex = args.findIndex(a => a === '--output' || a === '-o');
const output = outputIndex >= 0 ? args[outputIndex + 1] : undefined;
const input = args.filter(a => !a.startsWith('-'))[0];

if (!input) {
  console.error('Error: Input file required');
  process.exit(1);
}`,
});

// --- WebSocket Skills ---

const handleWebSocket = createSkill({
  name: "Handle WebSocket Connection",
  description: "Implement WebSocket connection handling with message routing",
  category: "file_operations",
  source: "bootstrap",
  tags: ["websocket", "realtime"],
  code: `// WebSocket handling with Bun.serve

Bun.serve({
  websocket: {
    open(ws) {
      console.log('Client connected');
      ws.subscribe('global'); // Subscribe to broadcast channel
    },

    message(ws, message) {
      const data = JSON.parse(message as string);

      switch (data.type) {
        case 'ping':
          ws.send(JSON.stringify({ type: 'pong' }));
          break;

        case 'broadcast':
          ws.publish('global', JSON.stringify(data));
          break;

        case 'subscribe':
          ws.subscribe(data.channel);
          break;

        default:
          ws.send(JSON.stringify({ error: 'Unknown message type' }));
      }
    },

    close(ws) {
      console.log('Client disconnected');
    },
  },
});`,
});

// --- Configuration Skills ---

const loadConfig = createSkill({
  name: "Load Configuration",
  description: "Load configuration from environment, files, and defaults with validation",
  category: "file_operations",
  source: "bootstrap",
  tags: ["config", "environment"],
  code: `// Configuration loading pattern

interface Config {
  port: number;
  database: {
    host: string;
    port: number;
    name: string;
  };
  debug: boolean;
}

const loadConfig = (): Config => {
  // 1. Start with defaults
  const config: Config = {
    port: 3000,
    database: {
      host: 'localhost',
      port: 5432,
      name: 'app',
    },
    debug: false,
  };

  // 2. Load from config file if exists
  try {
    const fileConfig = JSON.parse(
      await Bun.file('config.json').text()
    );
    Object.assign(config, fileConfig);
  } catch { /* no config file */ }

  // 3. Override with environment variables
  if (process.env.PORT) config.port = parseInt(process.env.PORT);
  if (process.env.DB_HOST) config.database.host = process.env.DB_HOST;
  if (process.env.DEBUG === 'true') config.debug = true;

  // 4. Validate required values
  if (!config.database.host) {
    throw new Error('Database host is required');
  }

  return config;
};`,
});

// --- Logging Skills ---

const structuredLogging = createSkill({
  name: "Structured Logging",
  description: "Implement structured JSON logging for production applications",
  category: "file_operations",
  source: "bootstrap",
  tags: ["logging", "observability"],
  code: `// Structured logging helper

type LogLevel = 'debug' | 'info' | 'warn' | 'error';

interface LogEntry {
  timestamp: string;
  level: LogLevel;
  message: string;
  [key: string]: unknown;
}

const log = (level: LogLevel, message: string, meta?: Record<string, unknown>) => {
  const entry: LogEntry = {
    timestamp: new Date().toISOString(),
    level,
    message,
    ...meta,
  };

  const output = JSON.stringify(entry);

  if (level === 'error') {
    console.error(output);
  } else {
    console.log(output);
  }
};

// Usage:
log('info', 'Request handled', { path: '/api/users', duration: 45 });
log('error', 'Database connection failed', { error: err.message, retryIn: 5000 });`,
});

// --- Caching Skills ---

const implementCache = createSkill({
  name: "Implement Cache",
  description: "Add caching layer with TTL and LRU eviction",
  category: "performance",
  source: "bootstrap",
  tags: ["cache", "performance"],
  code: `// Simple cache with TTL

class Cache<T> {
  private store = new Map<string, { value: T; expiresAt: number }>();
  private maxSize: number;

  constructor(maxSize: number = 1000) {
    this.maxSize = maxSize;
  }

  get(key: string): T | undefined {
    const entry = this.store.get(key);
    if (!entry) return undefined;

    if (Date.now() > entry.expiresAt) {
      this.store.delete(key);
      return undefined;
    }

    return entry.value;
  }

  set(key: string, value: T, ttlMs: number = 60000): void {
    // Evict oldest if at capacity
    if (this.store.size >= this.maxSize) {
      const oldest = this.store.keys().next().value;
      this.store.delete(oldest);
    }

    this.store.set(key, {
      value,
      expiresAt: Date.now() + ttlMs,
    });
  }

  delete(key: string): boolean {
    return this.store.delete(key);
  }
}

const cache = new Cache<string>();`,
});

// --- Export All Compositional Skills ---

export const compositionalSkills: Skill[] = [
  // Error Fixing (3)
  fixTypescriptImportError,
  fixTypescriptTypeError,
  fixSyntaxError,

  // Testing (6)
  addTestForFunction,
  runTestsWithCoverage,
  fixFailingTest,
  mockDependency,
  writeIntegrationTest,
  writeSnapshotTest,

  // Git Workflow (6)
  createFeatureBranch,
  createPullRequest,
  resolveGitConflict,
  cherryPickCommit,
  bisectBug,
  revertCommit,

  // Refactoring (5)
  extractFunction,
  renameSymbol,
  convertToEffect,
  splitLargeFile,
  inlineAbstraction,

  // Code Generation (3)
  generateTypeFromJson,
  scaffoldComponent,
  generateApiClient,

  // Performance (3)
  profileCode,
  optimizeImports,
  implementCache,

  // Documentation (1)
  generateJsDoc,

  // Security (2)
  auditDependencies,
  sanitizeInput,

  // Build (2)
  setupBunProject,
  createDockerfile,

  // Effect-TS (2)
  createEffectService,
  handleEffectError,

  // Database (1)
  createDatabaseMigration,

  // Debugging (2)
  debugAsyncIssue,
  traceMemoryLeak,

  // API (3)
  handleApiErrors,
  validateApiInput,
  implementRateLimit,

  // CLI (2)
  parseCliArgs,
  createCliCommand,

  // WebSocket (1)
  handleWebSocket,

  // Configuration (1)
  loadConfig,

  // Observability (1)
  structuredLogging,
];

export default compositionalSkills;
