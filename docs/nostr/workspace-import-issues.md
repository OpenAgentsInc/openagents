# Nostr Package Workspace Import Issues

## Problem Summary

There is a fundamental issue with importing from `@openagentsinc/nostr` in CI environments that prevents TypeScript compilation. This was "fixed" in PR #1010 by eliminating the imports entirely, but this is a workaround that will become a problem when we actually need to import Nostr functionality.

## The Issue

When SDK browser services try to import from `@openagentsinc/nostr`:

```typescript
import * as Nostr from "@openagentsinc/nostr"
```

The build fails in CI with:
```
error TS2307: Cannot find module '@openagentsinc/nostr' or its corresponding type declarations.
```

## Root Cause Analysis

### 1. PNPM Workspace Resolution Differences

**Local Environment**: PNPM correctly creates symlinks for workspace dependencies
- `packages/sdk/node_modules/@openagentsinc/nostr` → `../nostr/dist`
- TypeScript can resolve the module

**CI Environment**: Workspace symlinks are not properly established during the build process
- Even after building dependencies, the symlink may not exist
- TypeScript cannot find the module during compilation

### 2. Build Order Dependencies

The issue occurs because:
1. CI installs dependencies with `pnpm install`
2. Later, packages are built in order (AI → Nostr → SDK)
3. Even though Nostr is built, the workspace link isn't re-established
4. SDK compilation fails to find `@openagentsinc/nostr`

### 3. Effect.js Build Process Complexity

Effect packages use a complex build process:
1. `build-esm` - TypeScript compilation
2. `build-annotate` - Pure call annotations
3. `build-cjs` - CommonJS transformation
4. `pack-v2` - Effect build utils packaging
5. `fix-dist` - Flatten dist/dist structure

The workspace resolution issues are exacerbated by this multi-step process.

## Attempted Solutions

### 1. Build Dependencies First in CI ❌
```yaml
- name: Build dependencies first
  run: pnpm --filter=@openagentsinc/ai run build && pnpm --filter=@openagentsinc/nostr run build
```
**Result**: Dependencies built successfully, but workspace links still not available for SDK compilation

### 2. Force Workspace Link Re-establishment ❌
```yaml
- name: Re-establish workspace links
  run: pnpm install --force
```
**Result**: No change, SDK still couldn't find nostr module

### 3. Manual Symlink Creation ❌
```bash
ln -sf ../nostr/dist packages/sdk/node_modules/@openagentsinc/nostr
```
**Result**: Symlink created but still didn't resolve the TypeScript compilation issue

### 4. Updated TypeScript Paths ❌
```json
"paths": {
  "@openagentsinc/nostr": ["../../packages/nostr/dist/dts"],
  "@openagentsinc/nostr/*": ["../../packages/nostr/dist/dts/*"]
}
```
**Result**: Worked locally but failed in CI

### 5. Import Strategy Changes ❌
- Changed from `import type` to regular `import`
- Changed from subpath imports to namespace imports
- **Result**: None of these resolved the CI issue

### 6. Inline Type Definitions ✅ (WORKAROUND)
```typescript
// Instead of: import * as Nostr from "@openagentsinc/nostr"
// Use inline type definition:
type NostrEvent = {
  id: string
  pubkey: string
  created_at: number
  kind: number
  tags: ReadonlyArray<ReadonlyArray<string>>
  content: string
  sig: string
}
```
**Result**: CI passes, but this avoids the real problem

## Current Workaround

**Files affected**:
- `packages/sdk/src/browser/AgentService.ts`
- `packages/sdk/src/browser/ChannelService.ts`
- `packages/sdk/src/browser/ServiceOfferingService.ts`

**Change made**: Replaced `import * as Nostr from "@openagentsinc/nostr"` with inline `NostrEvent` type definition.

**Limitation**: This only works because we were only importing the `NostrEvent` type. When we need to import actual functions, services, or more complex types from the nostr package, this workaround will break.

## Future Solutions Needed

### 1. Fix PNPM Workspace Resolution in CI

**Investigation needed**:
- Why does `pnpm install` not properly establish workspace symlinks in CI?
- Does CI need a different pnpm configuration?
- Should we use a different package manager or installation strategy?

### 2. TypeScript Project References

Consider using TypeScript project references properly:
```json
{
  "references": [
    { "path": "../nostr" },
    { "path": "../ai" }
  ]
}
```

### 3. Alternative Build Strategies

**Option A**: Build all packages before any TypeScript compilation
```bash
pnpm --filter=@openagentsinc/nostr run build-esm
pnpm --filter=@openagentsinc/sdk run build-esm
```

**Option B**: Use a monorepo build tool that understands dependencies
- Consider tools like Nx, Rush, or Bazel
- These tools can properly handle workspace dependencies

**Option C**: Investigate Effect.js specific solutions
- Check if Effect has recommended patterns for workspace dependencies
- Look at how other Effect.js monorepos handle this

## When This Will Become Critical

This issue will resurface when we need to:

1. **Import Nostr services in SDK**: When SDK needs to use `RelayService`, `EventService`, etc.
2. **Import Nostr schemas**: When SDK needs complex schema validation from nostr package
3. **Import Nostr utilities**: When SDK needs crypto utilities, key derivation, etc.
4. **Cross-package service composition**: When building complex Effect.js layer compositions

## Debugging Steps for Future Investigation

1. **Check workspace structure in CI**:
   ```bash
   ls -la packages/sdk/node_modules/@openagentsinc/
   ```

2. **Verify symlink creation**:
   ```bash
   readlink packages/sdk/node_modules/@openagentsinc/nostr
   ```

3. **Test TypeScript resolution manually**:
   ```bash
   cd packages/sdk && npx tsc --traceResolution --noEmit
   ```

4. **Compare local vs CI pnpm-lock.yaml**:
   Check if there are differences in how dependencies are resolved

5. **Test with different Node.js versions**:
   CI might have different behavior with symlinks

## Related Files

- `package.json` - Root workspace configuration
- `packages/sdk/package.json` - SDK dependencies
- `packages/sdk/tsconfig.json` - TypeScript paths configuration
- `.github/workflows/check.yml` - CI configuration that exhibits the issue

## Conclusion

This is a genuine technical debt that needs proper investigation. The current workaround of inline type definitions is fragile and will break as soon as we need real functionality from the nostr package. The root cause appears to be PNPM workspace resolution differences between local and CI environments, possibly related to the Effect.js build process complexity.

**Priority**: High - this will block future nostr package integration work.