# TypeScript Build Configuration Issues Analysis

## Overview

This document contains an analysis of the TypeScript build configuration issues encountered during the debugging of the "Service not found: PlanManager" error in the OpenAgents engine. It identifies the underlying problems in the TypeScript configuration, explains their impact on the build process, and documents the solutions implemented.

## Problem Description

During debugging of "Service not found: PlanManager" errors in the Effect.js application, we encountered build failures with the error:

```
Error: Cannot find module '/Users/christopherdavid/code/openagents/apps/engine/build/esm/index.js'
```

The root cause was an incorrectly modified `tsconfig.build.json` file, which was restricting the TypeScript compilation to only a few specific files, resulting in missing output JavaScript files that were needed at runtime.

## Technical Analysis

### The `files` Array Problem

In TypeScript configuration, when the `files` array is specified in a tsconfig file, it instructs the TypeScript compiler to **only** compile the listed files (and their dependencies). It completely overrides the more general `include` and `exclude` patterns.

The problematic configuration in `tsconfig.build.json` was:

```json
{
  "extends": "./tsconfig.src.json",
  "compilerOptions": {
    /* ... compilerOptions ... */
  },
  "include": ["src/**/*"],  // This was being completely ignored
  "files": [                // This was the problem
    "src/index.ts",
    "src/Program.ts",
    "src/Server.ts"
  ]
}
```

Similarly, the debug configuration had the same issue:

```json
{
  "extends": "./tsconfig.src.json",
  "compilerOptions": {
    /* ... compilerOptions ... */
  },
  "include": ["src/**/*"],  // This was being ignored
  "exclude": ["test/**/*"], // This was being ignored
  "files": [                // This was the problem
    "src/index.ts",
    "src/Program.ts",
    "src/Server.ts",
    "src/github/PlanManager.ts"
  ]
}
```

### Impact of the Configuration Issue

This configuration caused several critical problems:

1. **Limited Compilation Scope:** The TypeScript compiler only processed the explicitly listed files, ignoring all other source files in the codebase.

2. **Missing Output Files:** When the build command was run, only the explicitly listed files were compiled to JavaScript, leaving out essential files like those in the `src/github/` directory.

3. **Runtime Module Resolution Failures:** When the application ran, it tried to import modules from files that weren't compiled (e.g., `import { PlanManagerLayer } from "./github/PlanManager.js"`), resulting in Node.js `ERR_MODULE_NOT_FOUND` errors.

4. **Build/Runtime Mismatch:** The build process appeared to succeed (no TypeScript errors), but runtime failures occurred because required files were missing.

### TypeScript Project References

The project uses TypeScript's Project References feature with multiple configuration files:

1. `tsconfig.base.json` - Contains shared compiler options
2. `tsconfig.json` - Root configuration that references the source and test configurations
3. `tsconfig.src.json` - Configuration for the source code
4. `tsconfig.build.json` - Configuration for production builds
5. `tsconfig.debug.json` - Configuration for debugging builds
6. `tsconfig.test.json` - Configuration for tests

## Solution

The solution was to remove the restrictive `files` array from the affected configuration files, allowing the more general `include` pattern to control which files get compiled:

### Updated `tsconfig.build.json`:

```json
{
  "extends": "./tsconfig.src.json",
  "compilerOptions": {
    "types": ["node"],
    "tsBuildInfoFile": ".tsbuildinfo/build.tsbuildinfo",
    "outDir": "build/esm",
    "declarationDir": "build/dts",
    "stripInternal": true
  },
  "include": ["src/**/*"]  // This is now respected
}
```

### Updated `tsconfig.debug.json`:

```json
{
  "extends": "./tsconfig.src.json",
  "compilerOptions": {
    "types": ["node"],
    "tsBuildInfoFile": ".tsbuildinfo/debug.tsbuildinfo",
    "outDir": "build/debug",
    "declarationDir": "build/dts",
    "stripInternal": true,
    "noEmitOnError": false
  },
  "include": ["src/**/*"],
  "exclude": ["test/**/*"]
}
```

## Best Practices for TypeScript Configuration

1. **Use `include`/`exclude` When Possible:** For most projects, it's better to use `include` and `exclude` patterns rather than explicit `files` lists, as they provide more flexibility and are less prone to errors.

2. **Be Aware of Override Behavior:** Understand that `files` completely overrides `include` and `exclude` patterns.

3. **Check Build Output:** Regularly verify that the build output contains all expected files, especially after changing TypeScript configuration.

4. **Project References:** For complex projects, consider using TypeScript's Project References feature correctly to manage dependencies between different parts of the codebase.

5. **Document Configuration Decisions:** Maintain documentation explaining why certain TypeScript configuration choices were made, especially for multi-configuration setups.

## Conclusion

The seemingly minor addition of a `files` array to the TypeScript configuration had far-reaching consequences for the build process, leading to runtime errors that were difficult to diagnose. By understanding how TypeScript's configuration options interact and following best practices, we can avoid similar issues in the future.

This incident highlights the importance of thorough testing after configuration changes and maintaining a good understanding of the build toolchain in complex JavaScript/TypeScript projects.