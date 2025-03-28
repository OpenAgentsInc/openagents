# Cross-Platform TypeScript Configuration Fixes

This document explains how we resolved TypeScript errors related to cross-platform UI component sharing between Electron and React Native applications.

## Problem

When attempting to share UI components between our Electron application (Coder) and React Native application (Onyx), we encountered several TypeScript errors:

1. Module resolution conflicts between CommonJS and ESM formats
2. Errors importing React components due to missing explicit React imports
3. TypeScript compilation scope issues preventing proper access to UI component source files
4. Path mapping conflicts causing import resolution failures

These issues were preventing the successful sharing of components across platforms without requiring separate build steps.

## Solution Overview

We implemented several TypeScript configuration changes to enable direct source imports:

1. Updated TypeScript module resolution settings in `tsconfig.json` files
2. Added explicit React imports in all UI components  
3. Expanded TypeScript compilation scope to include UI package source files
4. Configured proper path mappings for cross-workspace imports
5. Documented best practices for cross-platform component development

## Detailed Implementation

### 1. TypeScript Configuration Changes

#### Coder (Electron App) tsconfig.json

```diff
{
  "compilerOptions": {
    "jsx": "react",
    "target": "ESNext",
-   "module": "Node16",
-   "moduleResolution": "node16",
+   "module": "CommonJS",
+   "moduleResolution": "node",
    "lib": ["DOM", "DOM.Iterable", "ESNext"],
    "skipLibCheck": true,
    "types": ["node", "electron"],
    "declaration": true,
    "composite": true,
    "forceConsistentCasingInFileNames": true,
-   "resolvePackageJsonImports": true,
    "allowJs": false,
    "esModuleInterop": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"],
      "@openagents/ui": ["../../packages/ui/src"],
      "@openagents/ui/*": ["../../packages/ui/src/*"]
    },
    "outDir": "dist",
-   "resolveJsonModule": true
+   "resolveJsonModule": true,
+   "rootDir": "../.."
  },
- "include": ["src/**/*"]
+ "include": ["src/**/*", "../../packages/ui/src/**/*"]
}
```

Key changes:
- Changed module system from Node16 to CommonJS for better compatibility
- Changed moduleResolution from node16 to node
- Removed resolvePackageJsonImports which was causing conflicts
- Added rootDir pointing to monorepo root
- Expanded include paths to include UI package source files

### 2. React Import Fix

We added explicit React imports to all UI components to ensure compatibility with both React Native and React for Web:

```diff
+ import React from 'react';
import { Text, TouchableOpacity, ActivityIndicator, View } from 'react-native';
import { ButtonProps } from './Button.types';
import { getButtonStyles, getButtonHeight, getTextStyle, getTextSize, styles, COLORS } from './Button.styles';
```

This is necessary because:
- React Native implicitly provides React in scope
- React for Web (used in Electron) requires explicit imports
- TypeScript JSX transformation requires React to be in scope

### 3. Package.json Configuration

We updated workspace dependencies to properly reference the UI package:

```json
{
  "dependencies": {
    "@openagents/ui": "*"
  }
}
```

This ensures Yarn resolves the dependency to the local workspace package.

### 4. Component Design Best Practices

We documented the following TypeScript best practices for shared components:

1. **Always import React**: Include `import React from 'react'` for compatibility
2. **Cross-Platform Types**: Use React Native types as the foundation (they're compatible with react-native-web)
3. **Consistent Props API**: Define clear TypeScript interfaces for all components 
4. **Modular Structure**: Keep types, styles, and implementation in separate files
5. **Platform-Specific Code**: Use `.native.tsx` and `.web.tsx` extensions for platform divergence

## Troubleshooting Common TypeScript Errors

1. **"Cannot find module '@openagents/ui'"**: 
   - Check path mappings in tsconfig.json
   - Ensure the UI package is included in workspace dependencies

2. **"JSX element implicitly has type 'any'"**:
   - Ensure React is imported
   - Check that the UI package source files are included in the compilation scope

3. **"Property 'X' does not exist on type 'Y'"**:
   - Check for React Native vs. Web API differences
   - Use platform-specific implementations when necessary

4. **Module resolution errors**:
   - Check moduleResolution setting in tsconfig.json
   - Ensure path mappings are correctly configured

## Benefits of the Fix

- **Direct Source Imports**: No separate build process needed for UI components
- **Immediate Updates**: Changes to UI components are immediately reflected in both apps
- **Full Type Safety**: TypeScript errors are caught across workspace boundaries
- **Simplified Development**: Fewer build steps and configuration requirements
- **Consistent Developer Experience**: Same component API works in all platforms

## Related Files

- `/apps/coder/tsconfig.json` - TypeScript configuration for Electron app
- `/apps/onyx/tsconfig.json` - TypeScript configuration for React Native app
- `/packages/ui/tsconfig.json` - TypeScript configuration for shared UI components
- `/packages/ui/src/components/Button/Button.tsx` - Example UI component with fixes

## Related Documentation

- [Cross-Platform UI Components](/docs/cross-platform-ui-components.md) - Detailed explanation of the cross-platform UI approach
- [Hot Reloading UI Package in Electron](/docs/fixes/hot-reloading-ui-package-electron.md) - Related fix for development workflow
- [Expo Vector Icons in Electron](/docs/fixes/expo-vector-icons-electron.md) - Related fix for icon compatibility