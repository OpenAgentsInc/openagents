# Three.js Setup with No-Build ESM

We're using **Option 2: Soft #nobuild** - TypeScript → ES modules with on-demand compilation.

## Architecture

- **No bundler** - Modules served separately
- **On-demand compilation** - Server compiles `.ts` → ESM when requested
- **Import maps** - Browser resolves `three` and `effect` via CDN
- **External dependencies** - `three` and `effect` marked as external, import statements preserved

## How It Works

1. **Import Map** (`new.html`):
   ```html
   <script type="importmap">
   {
     "imports": {
       "effect": "https://esm.sh/effect@3.19.8",
       "effect/": "https://esm.sh/effect@3.19.8/",
       "three": "https://esm.sh/three@0.169.0",
       "three/": "https://esm.sh/three@0.169.0/"
     }
   }
   </script>
   ```

2. **TypeScript Files** - Write normal TypeScript:
   ```typescript
   import * as THREE from "three"

   export function createScene(canvas: HTMLCanvasElement) {
     const scene = new THREE.Scene()
     // ...
   }
   ```

3. **Server Compilation** - When browser requests `scene.ts`:
   - Server compiles to ESM format
   - Marks `three` as external (preserves `import * as THREE from "three"`)
   - Browser resolves `three` via import map → loads from esm.sh CDN

4. **HTML Usage**:
   ```html
   <script type="module" src="three-scene.ts"></script>
   ```

## Example

See `src/mainview/three-scene.ts` for a simple rotating cube example.

## Benefits

- ✅ Full TypeScript syntax (interfaces, generics, type annotations)
- ✅ TypeScript types for Three.js (`@types/three`)
- ✅ No bundling step - modules cached separately
- ✅ Better cache granularity - only changed modules re-download
- ✅ Standard browser-native module system
- ✅ On-demand compilation - no build step needed

## Adding More Dependencies

To add another external dependency:

1. **Add to import map** (`new.html`):
   ```json
   {
     "imports": {
       "my-lib": "https://esm.sh/my-lib@1.0.0"
     }
   }
   ```

2. **Mark as external** (`src/desktop/server.ts`):
   ```typescript
   external: format === "esm" ? ["effect", "three", "my-lib"] : [],
   ```

3. **Install types** (if available):
   ```bash
   bun add -d @types/my-lib
   ```

4. **Use in TypeScript**:
   ```typescript
   import { something } from "my-lib"
   ```

## Why This Approach?

- **Matches current architecture** - We're already doing on-demand TS→ESM compilation
- **TypeScript throughout** - Codebase uses TS, not JSDoc
- **Minimal changes** - Just add to import map and external list
- **Best of both worlds** - TypeScript ergonomics + no-build simplicity
