# Fix for fs-extra Module Loading Issue in Packaged App

## Problem

When packaging the Electron app, the following error was occurring:

```
Error: Cannot find module 'fs-extra'
Require stack:
- /Users/christopherdavid/code/openagents/apps/coder/out/Coder-darwin-arm64/Coder.app/Contents/Resources/app.asar/.vite/build/main-DE-1Awy6.js
- /Users/christopherdavid/code/openagents/apps/coder/out/Coder-darwin-arm64/Coder.app/Contents/Resources/app.asar/.vite/build/main.js
```

This is a common issue when packaging Electron applications with external Node.js dependencies. The error occurs because the module is being bundled incorrectly by Vite/Rollup during the build process. When the application is run from the ASAR archive, it cannot properly resolve the `fs-extra` module.

## Solution

The following changes were implemented to fix this issue:

### 1. Extended Vite External Modules Configuration

Updated `vite.main.config.ts` to prevent fs-extra and its dependencies from being bundled:

```javascript
build: {
  rollupOptions: {
    external: [
      'fs-extra',
      'graceful-fs',    // fs-extra dependency
      'jsonfile',       // fs-extra dependency
      'universalify',   // fs-extra dependency
      'electron',
      'node:path',
      'node:fs',
      'node:fs/promises',
      'node:child_process',
      'path',
      'fs',
      'os',
    ]
  }
},
```

### 2. Enhanced ASAR Unpacking in Forge Config

Updated `forge.config.cjs` to unpack fs-extra and its dependencies from the ASAR archive:

```javascript
asar: {
  unpack: [
    "**/node_modules/fs-extra/**",
    "**/node_modules/graceful-fs/**",
    "**/node_modules/jsonfile/**",
    "**/node_modules/universalify/**"
  ]
},
```

### 3. Modified Import Style

Changed imports in `dbService.ts` to use a more explicit import style:

```javascript
// Before
import fs from 'fs-extra';

// After
import * as fs from 'fs-extra';
```

### 4. Used Explicit require() in Main Process

In `main.ts`, switched to using `require()` for fs-extra to ensure it loads correctly:

```javascript
import path from 'node:path';
// Import fs-extra with explicit require to ensure it loads properly in packaged app
const fs = require('fs-extra');
```

### 5. Consistent Usage of the fs-extra Module

Ensured the same fs-extra instance is used throughout the application:

```javascript
// Before
if(await require('fs-extra').pathExists(iconPath)) app.dock.setIcon(iconPath);

// After
if(await fs.pathExists(iconPath)) app.dock.setIcon(iconPath);
```

## Why This Works

1. **Externalization:** Prevents Vite/Rollup from attempting to bundle Node.js modules that should be loaded at runtime
2. **ASAR Unpacking:** Keeps the fs-extra module and its dependencies as regular files outside the ASAR archive
3. **Explicit require:** Using require() instead of ESM import can be more reliable for native Node.js modules in Electron
4. **Consistent usage:** Reusing the same instance prevents multiple copies from being loaded

This approach aligns with the fixes attempted in PR #851, but is more comprehensive by addressing all potential points of failure.

## References

- [PR #851](https://github.com/OpenAgentsInc/openagents/pull/851)
- [Electron Forge Packaging Documentation](https://www.electronforge.io/config/packagerconfig)
- [Vite External Modules Documentation](https://vitejs.dev/config/build-options.html#build-rollupoptions-external)