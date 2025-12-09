# Effuse No-Build Migration Plan

Adopting DHH's "#nobuild" philosophy to solve JavaScript bundling/caching issues.

## Current State

- **Bundling**: `effuse-main.ts` → `effuse-main.js` (IIFE bundle via Bun.build)
- **HMR**: Rebuilds entire bundle on file changes
- **Problem**: Cache invalidation issues, full bundle rebuilds, complex asset pipeline

## Target State

- **ES Modules**: Serve TypeScript modules directly (or compile on-demand as separate modules)
- **Import Maps**: Resolve Effect and dependencies via import maps
- **Cache Granularity**: Individual modules cached separately
- **Simpler Pipeline**: No bundling step, just serve files

## Architecture

### 1. Import Map Setup

Create `importmap.json` or inline in HTML:

```json
{
  "imports": {
    "effect": "https://esm.sh/effect@3.19.8",
    "effect/": "https://esm.sh/effect@3.19.8/"
  }
}
```

Or use a local Effect build (better for offline/dev):

```json
{
  "imports": {
    "effect": "/node_modules/.vite/deps/effect.js",
    "effect/": "/node_modules/.vite/deps/effect/"
  }
}
```

### 2. Module Structure

Instead of:
```
effuse-main.ts → [Bun.build] → effuse-main.js (bundle)
```

Do:
```
effuse/
├── index.ts (main entry)
├── widgets/
│   ├── intro-card.ts
│   ├── apm-widget.ts
│   └── ...
└── services/
    ├── dom.ts
    └── ...

mainview/
├── new-main.ts (entry point)
└── effuse-main.ts (entry point)
```

Each file served as separate module, browser resolves imports.

### 3. Server Changes

**Current** (`src/desktop/server.ts`):
- Builds TypeScript on-demand into single bundle
- Serves as `effuse-main.js`

**New**:
- Serve TypeScript files directly (or compile to ESM on-demand)
- Add import map to HTML
- Let browser handle module resolution

### 4. HTML Changes

**Current** (`src/mainview/index.html`):
```html
<script defer src="effuse-main.js?v=20251208-1422"></script>
```

**New**:
```html
<script type="importmap">
{
  "imports": {
    "effect": "https://esm.sh/effect@3.19.8",
    "effect/": "https://esm.sh/effect@3.19.8/"
  }
}
</script>
<script type="module" src="/effuse-main.ts"></script>
```

### 5. TypeScript Compilation

**Option A: Serve TS directly** (requires browser TS support - not viable)

**Option B: Compile on-demand to ESM** (recommended):
- When `/effuse-main.ts` requested, compile to ESM
- Output separate modules, preserve import statements
- Browser resolves via import map

**Option C: Pre-compile to ESM** (build step, but simpler):
- `bun build --format=esm --splitting` outputs ESM modules
- Serve pre-compiled `.js` files
- Still get cache granularity

## Implementation Steps

### Phase 1: Proof of Concept (new.html)

1. **Create import map** for Effect
2. **Convert `new-main.ts`** to use ESM imports
3. **Update server** to compile TypeScript to ESM (not IIFE)
4. **Update `new.html`** to use `<script type="module">`
5. **Test** that it works

### Phase 2: Migrate Effuse Core

1. **Update all Effuse files** to use ESM imports
2. **Create import map** with all dependencies
3. **Update server** to handle ESM compilation
4. **Update `index.html`** to use modules
5. **Test** widgets still work

### Phase 3: Optimize

1. **Add module preloading** for critical paths
2. **Implement module-level HMR** (reload only changed modules)
3. **Add module caching** headers
4. **Profile** and optimize load times

## Benefits

1. **Better Cache Granularity**: Only changed modules need re-download
2. **Simpler Pipeline**: No bundling step, just serve files
3. **Easier Debugging**: Source maps match source files exactly
4. **Faster HMR**: Only reload changed modules, not entire bundle
5. **Standard Approach**: Uses browser-native module system

## Trade-offs

1. **HTTP/2 Required**: Multiple requests (but HTTP/2 handles this well)
2. **Import Map Support**: Requires modern browsers (all current browsers support it)
3. **Effect ESM**: Need to ensure Effect is available as ESM (esm.sh or local build)
4. **Initial Load**: May be slightly slower due to multiple requests (but better caching)

## Dependencies to Handle

- `effect` - Main dependency, use esm.sh or local ESM build
- `effect/Stream`, `effect/Layer`, etc. - Subpath imports via import map
- Internal modules - Relative imports work naturally

## Example: new-main.ts as ESM

```typescript
// new-main.ts
import { Effect, Layer } from "effect"
import { mountWidget, DomServiceLive, StateServiceLive, SocketServiceFromClient, IntroCardWidget } from "../effuse/index.js"
import { getSocketClient } from "./socket-client.js"

// Rest of code unchanged - just imports are ESM
```

## Server ESM Compilation

```typescript
// In server.ts, when .ts file requested:
if (ext === "ts" || (ext === "js" && !(await file.exists()))) {
  const tsPath = ext === "ts" ? fullPath : fullPath.replace(/\.js$/, ".ts")
  const tsFile = Bun.file(tsPath)
  if (await tsFile.exists()) {
    const result = await Bun.build({
      entrypoints: [tsPath],
      target: "browser",
      format: "esm",  // ESM instead of IIFE
      minify: false,
      splitting: true,  // Output separate modules
    })
    // Serve as ESM module
  }
}
```

## Next Steps

1. Start with `new.html` as proof of concept
2. Test Effect import via import map
3. Verify module resolution works
4. Migrate `effuse-main.ts` if successful
5. Update HMR to reload individual modules
