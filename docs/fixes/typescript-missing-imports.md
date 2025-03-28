# TypeScript Missing Import Fixes

This document covers fixes for TypeScript errors related to missing imports in the codebase.

## Issue: Missing GitHubError Import in MCP GitHub Server

### Problem

When running TypeScript type checking for the MCP GitHub server, the following error occurred:

```
src/common/utils.ts:83:28 - error TS2304: Cannot find name 'GitHubError'.

83     if (!(error instanceof GitHubError)) {
                              ~~~~~~~~~~~
```

This error occurred because:
1. The `utils.ts` file was using the `GitHubError` class in a type check
2. The class was defined in `errors.ts` but not imported into `utils.ts`

### Solution

The fix was straightforward - add the `GitHubError` to the import statement from the errors module:

```diff
import { getUserAgent } from "universal-user-agent";
- import { createGitHubError } from "./errors.js";
+ import { createGitHubError, GitHubError } from "./errors.js";
import { VERSION } from "./version.js";
```

### Why This Error Occurred

This type of error can occur in TypeScript for several reasons:
1. When refactoring and moving classes between files
2. When a developer uses a class but forgets to add it to the imports
3. When auto-imports fail to recognize a class usage in a type-checking context like `instanceof`

### Prevention Tips

To prevent similar errors in the future:

1. **Use IDE Features**: Rely on IDE auto-imports when using new classes or types
2. **Run TypeScript Checks**: Regularly run `tsc --noEmit` to catch these issues early
3. **Consider ESLint Rules**: Add ESLint rules to detect missing imports
4. **Include Type Checking in CI**: Ensure your CI pipeline includes TypeScript checks

## Other Common TypeScript Import Issues

### 1. Circular Dependencies

Circular dependencies can cause TypeScript import issues. For example:

```typescript
// fileA.ts
import { B } from './fileB';
export class A {
  b: B = new B();
}

// fileB.ts
import { A } from './fileA';
export class B {
  a: A = new A(); // Circular reference!
}
```

**Solution**: Break the circular dependency by:
- Creating an interface in a third file
- Using dependency injection
- Restructuring your code to avoid circular references

### 2. Default vs Named Exports

Confusion between default and named exports can cause import errors:

```typescript
// Incorrect:
import MyClass from './myFile'; // Trying to use default import

// When the export is:
export class MyClass { } // Named export, not default
```

**Solution**: Be consistent with export style and make sure imports match the export type.

### 3. Path Aliases Not Configured

When using path aliases in TypeScript, errors occur if they're not properly configured:

```typescript
// In code:
import { Something } from '@common/utils';

// Error if @common is not configured in tsconfig.json
```

**Solution**: Ensure path aliases are correctly set up in both `tsconfig.json` and build tooling.

## Related Files

- `/apps/mcp-github-server/src/common/utils.ts` - Location of the fixed error
- `/apps/mcp-github-server/src/common/errors.ts` - Source of the `GitHubError` class

## Testing the Fix

To verify the fix, run TypeScript type checking:

```bash
cd /path/to/apps/mcp-github-server
yarn tsc --noEmit
```

If successful, no errors will be displayed, indicating that TypeScript can now properly resolve all types and imports.