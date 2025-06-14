# Creating New Packages in OpenAgents Monorepo

This checklist ensures consistent setup of new packages within the @openagentsinc monorepo.

## Pre-requisites
- [ ] Decide on package name (format: `@openagentsinc/[name]`)
- [ ] Determine package dependencies (which other packages it depends on)
- [ ] Plan initial directory structure

## Step 1: Create Package Directory Structure

```bash
# From monorepo root
mkdir -p packages/[name]/{src,test}
cd packages/[name]
```

Create the following files:

### 1.1 package.json
```json
{
  "name": "@openagentsinc/[name]",
  "version": "0.0.0",
  "type": "module",
  "license": "CC0-1.0",
  "description": "[Package description]",
  "repository": {
    "type": "git",
    "url": "https://github.com/OpenAgentsInc/openagents",
    "directory": "packages/[name]"
  },
  "publishConfig": {
    "access": "public",
    "directory": "dist"
  },
  "scripts": {
    "codegen": "build-utils prepare-v2",
    "build": "pnpm build-esm && pnpm build-annotate && pnpm build-cjs && build-utils pack-v2",
    "build-esm": "tsc -b tsconfig.build.json",
    "build-cjs": "babel build/esm --plugins @babel/transform-export-namespace-from --plugins @babel/transform-modules-commonjs --out-dir build/cjs --source-maps",
    "build-annotate": "babel build/esm --plugins annotate-pure-calls --out-dir build/esm --source-maps",
    "check": "tsc -b tsconfig.json",
    "test": "vitest",
    "coverage": "vitest --coverage"
  },
  "dependencies": {
    "effect": "3.16.3"
    // Add other dependencies as needed
  },
  "effect": {
    "generateExports": {
      "include": [
        "**/*.ts"
      ]
    },
    "generateIndex": {
      "include": [
        "**/*.ts"
      ]
    }
  }
}
```

### 1.2 tsconfig.json
```json
{
  "extends": "../../tsconfig.base.json",
  "include": [],
  "references": [
    { "path": "tsconfig.src.json" },
    { "path": "tsconfig.test.json" }
  ]
}
```

### 1.3 tsconfig.src.json
```json
{
  "extends": "../../tsconfig.base.json",
  "include": ["src"],
  "references": [
    // Add references to other packages as needed
    // { "path": "../domain" }
  ],
  "compilerOptions": {
    "types": ["node"],
    "outDir": "build/src",
    "tsBuildInfoFile": ".tsbuildinfo/src.tsbuildinfo",
    "rootDir": "src"
  }
}
```

### 1.4 tsconfig.test.json
```json
{
  "extends": "../../tsconfig.base.json",
  "include": ["test"],
  "references": [
    { "path": "tsconfig.src.json" }
    // Add references to other packages as needed
  ],
  "compilerOptions": {
    "types": ["node"],
    "tsBuildInfoFile": ".tsbuildinfo/test.tsbuildinfo",
    "rootDir": "test",
    "noEmit": true
  }
}
```

### 1.5 tsconfig.build.json
```json
{
  "extends": "./tsconfig.src.json",
  "references": [
    // Add build references to other packages as needed
    // { "path": "../domain/tsconfig.build.json" }
  ],
  "compilerOptions": {
    "types": ["node"],
    "tsBuildInfoFile": ".tsbuildinfo/build.tsbuildinfo",
    "outDir": "build/esm",
    "declarationDir": "build/dts",
    "stripInternal": true
  }
}
```

### 1.6 vitest.config.ts
```typescript
import { mergeConfig, type UserConfigExport } from "vitest/config"
import shared from "../../vitest.shared.js"

const config: UserConfigExport = {}

export default mergeConfig(shared, config)
```

### 1.7 src/index.ts
```typescript
// This file will be auto-generated by build-utils
// Add your exports in other files
```

### 1.8 test/Dummy.test.ts
```typescript
import { describe, it, expect } from "vitest"

describe("[PackageName]", () => {
  it("should have tests", () => {
    expect(true).toBe(true)
  })
})
```

### 1.9 README.md
```markdown
# @openagentsinc/[name]

[Package description]

## Installation

```bash
pnpm add @openagentsinc/[name]
```

## Usage

[Usage examples]

## License

CC0-1.0
```

### 1.10 LICENSE
Copy the LICENSE file from another package or the root directory.

## Step 2: Update Root Configuration

### 2.1 Update tsconfig.base.json
Add paths for your new package:
```json
{
  "compilerOptions": {
    "paths": {
      // ... existing paths ...
      "@openagentsinc/[name]": ["./packages/[name]/src/index.js"],
      "@openagentsinc/[name]/*": ["./packages/[name]/src/*.js"],
      "@openagentsinc/[name]/test/*": ["./packages/[name]/test/*.js"]
    }
  }
}
```

### 2.2 Update root build script (if needed)
If your package has dependencies on other packages, update the build order in `package.json`:
```json
{
  "scripts": {
    "build": "pnpm --filter=@openagentsinc/domain run build && pnpm --filter=@openagentsinc/[name] run build && ..."
  }
}
```

## Step 3: Install Dependencies

```bash
# From monorepo root
pnpm install
```

## Step 4: Generate Initial Files

```bash
# From monorepo root
pnpm --filter=@openagentsinc/[name] codegen
```

## Step 5: Verify Setup

Run these commands to ensure everything is configured correctly:

```bash
# Type checking
pnpm --filter=@openagentsinc/[name] check

# Build
pnpm --filter=@openagentsinc/[name] build

# Test
pnpm --filter=@openagentsinc/[name] test
```

## Step 6: Update ESLint (if index.ts is ignored)

If ESLint is configured to ignore generated `index.ts` files, your package's generated index will be automatically excluded.

## Common Issues and Solutions

### Issue: TypeScript can't find references
**Solution**: Ensure all referenced packages are listed in the appropriate `tsconfig.*.json` files and that they've been built at least once.

### Issue: Codegen doesn't generate index.ts
**Solution**: Make sure you have at least one `.ts` file in `src/` directory (other than index.ts).

### Issue: Build fails with module resolution errors
**Solution**: Check that all dependencies are properly listed in `package.json` and use `workspace:^` for internal dependencies.

## Package Types

### Library Package (default)
- Exports functionality for use by other packages
- Includes `publishConfig` in package.json
- Has comprehensive test coverage

### Application Package
- End-user application (CLI, server, etc.)
- May not need `publishConfig`
- Focus on integration tests

### Internal Package
- Used only within the monorepo
- Set `"private": true` in package.json
- No `publishConfig` needed

## Best Practices

1. **Naming**: Use lowercase with hyphens (e.g., `ui-components`, not `UIComponents`)
2. **Dependencies**: Use exact versions for external deps, `workspace:^` for internal
3. **Exports**: Let build-utils generate index.ts, export from other files
4. **Tests**: Write tests from the start, aim for good coverage
5. **Documentation**: Keep README updated with usage examples
6. **Types**: Export all public types from your package
7. **Effect**: Follow Effect patterns for services and errors

## Next Steps

After creating your package:
1. Add meaningful source files in `src/`
2. Write comprehensive tests in `test/`
3. Document public APIs in README.md
4. Consider adding examples if complexity warrants it
5. Update CHANGELOG.md when making changes (use changesets)
