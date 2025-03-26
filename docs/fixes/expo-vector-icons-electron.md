# Cross-Platform Compatibility Fix: Expo Vector Icons in Electron

This document explains how we resolved compatibility issues with `@expo/vector-icons` when using shared UI components in both React Native and Electron applications.

## Problem

Our shared UI package used `@expo/vector-icons` (specifically `Ionicons`) in the `Toast` component. While this worked fine in the React Native app (Onyx), it caused build failures in the Electron app (Coder) with errors:

1. JSX syntax extension not enabled errors in `@expo/vector-icons` files
2. TypeScript errors with React Native types

## Solution Overview

We implemented several fixes to ensure cross-platform compatibility:

1. Added proper dependencies
2. Created a shim for Expo Vector Icons
3. Updated build configurations
4. Fixed TypeScript configuration

## Detailed Steps

### 1. Dependencies

We added the necessary dependencies to the appropriate packages:

- **UI Package**: Added `@expo/vector-icons` as a direct dependency
  ```json
  "dependencies": {
    "@expo/vector-icons": "^14.0.0",
    "react-native-web": "^0.19.10"
  }
  ```

- **Electron App**: Added React Native web dependencies
  ```json
  "dependencies": {
    "react-native-vector-icons": "^10.0.3",
    "react-native-web": "^0.19.10"
  }
  ```

### 2. Expo Vector Icons Shim

Created a shim file (`src/shims/expo-vector-icons.ts`) in the Electron app to map Expo icons to React Native Vector Icons:

```typescript
// This file provides a shim for Expo vector icons in Electron
import * as RNVIonicons from 'react-native-vector-icons/dist/Ionicons';

// Re-export Ionicons from react-native-vector-icons as Expo's Ionicons
export const Ionicons = RNVIonicons;
```

### 3. Vite Configuration Changes

Updated the Vite configuration in the Electron app to:

- Set up proper aliasing for React Native and Expo packages
- Configure esbuild to handle JSX in `.js` files
- Include React Native dependencies in optimizeDeps

**vite.renderer.config.mts**:
```typescript
export default defineConfig({
  // ...
  resolve: {
    preserveSymlinks: true,
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "react-native": "react-native-web",
      "react-native$": "react-native-web",
      "@openagents/ui": path.resolve(__dirname, "../../packages/ui/src"),
      "@openagents/ui/*": path.resolve(__dirname, "../../packages/ui/src/*"),
      // Add aliases for Expo packages
      "@expo/vector-icons": path.resolve(__dirname, "./src/shims/expo-vector-icons.ts"),
    },
  },
  optimizeDeps: {
    include: [
      'react-native-web',
      '@openagents/ui',
    ],
    esbuildOptions: {
      loader: {
        '.js': 'jsx',
      },
      resolveExtensions: ['.web.js', '.js', '.ts', '.jsx', '.tsx', '.json'],
      mainFields: ['browser', 'module', 'main'],
    },
  },
});
```

Similar changes were made to `vite.main.config.ts` and `vite.preload.config.ts`.

### 4. TypeScript Configuration

Updated TypeScript configuration to include React Native types:

```json
"compilerOptions": {
  "types": ["node", "electron", "react-native", "@types/react-native"],
  // ...
}
```

## Benefits of the Fix

- **Cross-Platform Compatibility**: The same UI components now work in both React Native and Electron
- **No Code Changes Needed**: The component code remains the same, with platform-specific adaptations handled through configuration
- **Maintainability**: Future UI components can use similar patterns without additional work

## Future Considerations

1. Consider creating a more comprehensive shim for other Expo vector icon sets if needed
2. Investigate using platform-specific component implementations for more complex cases
3. Document additional react-native components that may need similar treatment when used in Electron

## Related Files

- `/packages/ui/package.json` - Added `@expo/vector-icons` dependency
- `/apps/coder/package.json` - Added React Native dependencies
- `/apps/coder/src/shims/expo-vector-icons.ts` - Icon shim implementation
- `/apps/coder/vite.renderer.config.mts` - Updated Vite configuration
- `/apps/coder/vite.main.config.ts` - Main process build configuration
- `/apps/coder/vite.preload.config.ts` - Preload script build configuration
- `/apps/coder/tsconfig.json` - TypeScript configuration updates