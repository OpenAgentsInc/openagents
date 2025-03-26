# Cross-Platform Compatibility Fix: Expo Vector Icons in Electron

This document explains how we resolved compatibility issues with `@expo/vector-icons` when using shared UI components in both React Native and Electron applications.

## Problem

Our shared UI package used `@expo/vector-icons` (specifically `Ionicons`) in the `Toast` component. While this worked fine in the React Native app (Onyx), it caused build failures in the Electron app (Coder) with errors:

1. JSX syntax extension not enabled errors in `@expo/vector-icons` files
2. TypeScript errors with React Native types
3. Runtime errors: "Element type is invalid: expected a string (for built-in components) or a class/function (for composite components) but got: object"

## Solution Overview

We implemented several fixes to ensure cross-platform compatibility:

1. Added proper dependencies
2. Created a custom mock implementation of Ionicons
3. Updated build configurations
4. Fixed TypeScript configuration
5. Added font loading support for vector icons

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

### 2. Custom Mock Implementation for Ionicons

Instead of trying to directly use the React Native Vector Icons in Electron (which causes runtime errors), we created a custom SVG-based implementation:

**src/shims/mock-ionicons.tsx**:
```tsx
// A simple React component to mock Ionicons in case the main library fails
import React from 'react';

// Map of icon names to simple SVG paths
const iconPaths: Record<string, string> = {
  'heart': 'M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z',
  'settings-outline': 'M19.14,12.94c0.04-0.3,0.06-0.61,0.06-0.94c0-0.32-0.02-0.64-0.07-0.94l2.03-1.58c0.18-0.14,0.23-0.41,0.12-0.61 l-1.92-3.32c-0.12-0.22-0.37-0.29-0.59-0.22l-2.39,0.96c-0.5-0.38-1.03-0.7-1.62-0.94L14.4,2.81c-0.04-0.24-0.24-0.41-0.48-0.41 h-3.84c-0.24,0-0.43,0.17-0.47,0.41L9.25,5.35C8.66,5.59,8.12,5.92,7.63,6.29L5.24,5.33c-0.22-0.08-0.47,0-0.59,0.22L2.74,8.87 C2.62,9.08,2.66,9.34,2.86,9.48l2.03,1.58C4.84,11.36,4.8,11.69,4.8,12s0.02,0.64,0.07,0.94l-2.03,1.58 c-0.18,0.14-0.23,0.41-0.12,0.61l1.92,3.32c0.12,0.22,0.37,0.29,0.59,0.22l2.39-0.96c0.5,0.38,1.03,0.7,1.62,0.94l0.36,2.54 c0.05,0.24,0.24,0.41,0.48,0.41h3.84c0.24,0,0.44-0.17,0.47-0.41l0.36-2.54c0.59-0.24,1.13-0.56,1.62-0.94l2.39,0.96 c0.22,0.08,0.47,0,0.59-0.22l1.92-3.32c0.12-0.22,0.07-0.47-0.12-0.61L19.14,12.94z M12,15.6c-1.98,0-3.6-1.62-3.6-3.6 s1.62-3.6,3.6-3.6s3.6,1.62,3.6,3.6S13.98,15.6,12,15.6z',
  'close': 'M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z',
  // Add more icons as needed
};

interface IoniconsMockProps {
  name: string;
  size?: number;
  color?: string;
  style?: React.CSSProperties;
}

// A simple component that renders SVG icons
export const IoniconsMock: React.FC<IoniconsMockProps> = ({ 
  name, 
  size = 24, 
  color = 'currentColor',
  style = {}
}) => {
  // If we don't have this icon, render a placeholder
  if (!iconPaths[name]) {
    console.warn(`Icon "${name}" not found in mock Ionicons`);
    return (
      <div 
        style={{ 
          width: size, 
          height: size, 
          backgroundColor: 'lightgray',
          borderRadius: '50%',
          display: 'inline-block',
          ...style
        }} 
      />
    );
  }

  return (
    <svg 
      width={size} 
      height={size} 
      viewBox="0 0 24 24" 
      fill={color}
      stroke="none"
      style={style}
    >
      <path d={iconPaths[name]} />
    </svg>
  );
};

export default IoniconsMock;
```

**src/shims/expo-vector-icons.ts**:
```ts
// This file provides a shim for Expo vector icons in Electron
import React from 'react';
import { IoniconsMock } from './mock-ionicons';

// Export our mock implementation directly
export const Ionicons = IoniconsMock;
```

### 3. Icon Font Loading Support

Created a utility to load icon fonts in the Electron environment:

**src/shims/load-icon-fonts.ts**:
```typescript
// Load icon fonts for React Native Vector Icons in Electron
import IoniconsFont from 'react-native-vector-icons/Fonts/Ionicons.ttf';

// Create a style element to load the icon font
const iconFontStyles = `
@font-face {
  font-family: "Ionicons";
  src: url(${IoniconsFont}) format("truetype");
  font-weight: normal;
  font-style: normal;
}
`;

// Inject the styles into the document
export const loadIconFonts = () => {
  // Don't add the styles if they're already present
  if (document.getElementById('ionicons-font-styles')) return;

  const style = document.createElement('style');
  style.id = 'ionicons-font-styles';
  style.type = 'text/css';
  style.appendChild(document.createTextNode(iconFontStyles));
  document.head.appendChild(style);
};

export default loadIconFonts;
```

### 4. Vite Configuration Changes

Updated the Vite configuration in the Electron app to:

- Set up proper aliasing for React Native and Expo packages
- Configure esbuild to handle JSX in `.js` files
- Include React Native dependencies in optimizeDeps
- Add support for handling font files

**vite.renderer.config.mts**:
```typescript
export default defineConfig({
  // Configure asset handling
  assetsInclude: ['**/*.ttf'],
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
      'react-native-vector-icons',
      'react-native-vector-icons/Ionicons',
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

### 5. TypeScript Configuration

Updated TypeScript configuration to include React Native types:

```json
"compilerOptions": {
  "types": ["node", "electron", "react-native", "@types/react-native"],
  // ...
}
```

## Usage Example

With these fixes in place, we can now safely use Ionicons in our components across both platforms:

```tsx
import React, { useEffect } from "react";
import { Button } from "@openagents/ui";
import { Ionicons } from "@expo/vector-icons";
import loadIconFonts from "../shims/load-icon-fonts";

export default function HomePage() {
  // Load icon fonts on component mount
  useEffect(() => {
    loadIconFonts();
  }, []);
  
  // Function to render Ionicons
  const renderIcon = (iconName: string) => {
    return <Ionicons name={iconName} size={20} color="#ffffff" />;
  };

  return (
    <div className="flex h-full flex-col items-center justify-center gap-4">
      <Button label="Normal Button" variant="primary" />
      
      <Button 
        label="Icon Button" 
        variant="secondary" 
        leftIcon="heart" 
        renderIcon={renderIcon} 
      />
      
      <Button 
        label="Settings" 
        variant="primary" 
        leftIcon="settings-outline" 
        renderIcon={renderIcon} 
      />
    </div>
  );
}
```

## Benefits of the Fix

- **Cross-Platform Compatibility**: The same UI components now work in both React Native and Electron
- **No UI Component Changes Needed**: The original UI component code remains unchanged
- **Vector Icon Support**: Proper icon rendering in both platforms with the same API
- **Maintainability**: Future UI components can use similar patterns without additional work
- **Graceful Fallbacks**: Custom SVG implementation for unsupported icons

## Future Considerations

1. Expand the mock implementation for other Expo vector icon sets (FontAwesome, MaterialIcons, etc.)
2. Create a more comprehensive SVG path library for commonly used icons
3. Consider using a dedicated cross-platform icon library designed for both React Native and web
4. Automatic icon path extraction from official icon libraries to maintain consistency

## Related Files

- `/packages/ui/package.json` - Added `@expo/vector-icons` dependency
- `/apps/coder/package.json` - Added React Native dependencies
- `/apps/coder/src/shims/expo-vector-icons.ts` - Icon shim implementation
- `/apps/coder/src/shims/mock-ionicons.tsx` - Custom SVG icon implementation
- `/apps/coder/src/shims/load-icon-fonts.ts` - Font loading utility
- `/apps/coder/vite.renderer.config.mts` - Updated Vite configuration
- `/apps/coder/vite.main.config.ts` - Main process build configuration
- `/apps/coder/vite.preload.config.ts` - Preload script build configuration
- `/apps/coder/tsconfig.json` - TypeScript configuration updates