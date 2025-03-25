# Cross-Platform UI Components in OpenAgents

This document explains our approach to sharing UI components between Coder (Electron/CommonJS) and Onyx (React Native/Expo) applications without requiring separate build steps.

## Overview

OpenAgents uses a monorepo structure with three main packages:

1. `apps/coder` - Electron-based desktop application using CommonJS
2. `apps/onyx` - React Native mobile application using Expo
3. `packages/ui` - Shared UI components that work across both platforms

The goal is to allow both applications to directly consume the UI components from source without requiring a separate build/compilation step.

## Implementation Approach

Our approach uses direct imports from the source files combined with proper TypeScript configuration:

```
openagents/
├── apps/
│   ├── coder/            # Electron app (CommonJS)
│   │   ├── package.json
│   │   ├── tsconfig.json # Configured to include UI package
│   │   └── src/
│   │       └── pages/    # Imports from @openagents/ui
│   │
│   └── onyx/             # React Native app
│       ├── package.json
│       ├── tsconfig.json # Configured to include UI package
│       └── app/
│
└── packages/
    └── ui/
        ├── package.json
        ├── tsconfig.json
        └── src/
            ├── index.ts
            └── components/
                └── Button/
                    ├── Button.tsx
                    ├── Button.types.ts
                    ├── Button.styles.ts
                    └── index.ts
```

### How It Works

1. **React Native Web for Cross-Platform Rendering**:
   - Components are built using React Native primitives
   - `react-native-web` handles rendering in web environments
   - Single implementation adapts to platform capabilities

2. **Direct Source Imports**:
   - Both apps import directly from the UI package source
   - This eliminates the need for a separate build step
   - Changes to components are immediately reflected in both apps

3. **Workspace Package References**:
   - Each app lists the UI package as a dependency: `"@openagents/ui": "*"`
   - Yarn workspaces resolves this to the local package

4. **Path Mapping in TypeScript**:
   - Each app's `tsconfig.json` includes path mappings:
     ```json
     "paths": {
       "@openagents/ui": ["../../packages/ui/src"],
       "@openagents/ui/*": ["../../packages/ui/src/*"]
     }
     ```

5. **UI Package Structure**:
   - Clear component organization with separate files for types, styles, and implementation
   - Proper exports through index files
   - React Native primitives as the foundation

## TypeScript Configuration

### For the UI Package

```json
{
  "compilerOptions": {
    "target": "es2019",
    "module": "esnext",
    "lib": ["DOM", "DOM.Iterable", "ESNext"],
    "jsx": "react-jsx",
    "declaration": true,
    "moduleResolution": "node",
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "outDir": "dist",
    "paths": {
      "react": ["./node_modules/@types/react"],
      "react-native": ["./node_modules/@types/react-native"]
    }
  },
  "include": ["src"],
  "exclude": ["node_modules", "dist"]
}
```

### For Coder (Electron App)

```json
{
  "compilerOptions": {
    "jsx": "react",
    "target": "ESNext",
    "module": "CommonJS",
    "moduleResolution": "node",
    "lib": ["DOM", "DOM.Iterable", "ESNext"],
    "skipLibCheck": true,
    "esModuleInterop": true,
    "baseUrl": ".",
    "rootDir": "../..",
    "paths": {
      "@/*": ["./src/*"],
      "@openagents/ui": ["../../packages/ui/src"],
      "@openagents/ui/*": ["../../packages/ui/src/*"]
    }
  },
  "include": ["src/**/*", "../../packages/ui/src/**/*"]
}
```

### For Onyx (React Native App)

```json
{
  "extends": "expo/tsconfig.base",
  "compilerOptions": {
    "strict": true,
    "paths": {
      "@/*": ["./app/*"],
      "@openagents/ui": ["../../packages/ui/src"],
      "@openagents/ui/*": ["../../packages/ui/src/*"]
    }
  }
}
```

## Component Design Principles

1. **Always import React**: Include `import React from 'react'` for compatibility
2. **Cross-Platform Styles**: Use platform-agnostic style properties when possible
3. **Consistent Props API**: Define clear TypeScript interfaces for all components
4. **Modular Structure**: Keep types, styles, and implementation in separate files
5. **Clean Exports**: Export through index files for clean import statements

## Example Component

Here's how a typical component is structured:

### Button.types.ts
```typescript
import { TouchableOpacityProps } from 'react-native';

export interface ButtonProps extends TouchableOpacityProps {
  label: string;
  variant?: 'primary' | 'secondary' | 'tertiary';
  size?: 'small' | 'medium' | 'large';
  loading?: boolean;
  disabled?: boolean;
}
```

### Button.tsx
```typescript
import React from 'react';
import { Text, TouchableOpacity, ActivityIndicator, View } from 'react-native';
import { ButtonProps } from './Button.types';
import { getButtonStyles, getButtonHeight, getTextStyle, getTextSize, styles, COLORS } from './Button.styles';

export const Button = ({
  label,
  variant = 'primary',
  size = 'medium',
  loading = false,
  disabled = false,
  style,
  onPress,
  ...rest
}: ButtonProps) => {
  const buttonStyles = getButtonStyles(variant, disabled);
  const height = getButtonHeight(size);
  const textStyles = getTextStyle(variant, disabled);
  const fontSize = getTextSize(size);

  return (
    <TouchableOpacity
      style={[buttonStyles, { height }, style]}
      disabled={disabled || loading}
      onPress={onPress}
      activeOpacity={0.8}
      {...rest}
    >
      <View style={{ flexDirection: 'row', alignItems: 'center' }}>
        {loading && (
          <ActivityIndicator
            size="small"
            color={variant === 'tertiary' ? COLORS.black : COLORS.white}
            style={styles.activityIndicator}
          />
        )}
        <Text style={[textStyles, { fontSize }]}>{label}</Text>
      </View>
    </TouchableOpacity>
  );
};

export default Button;
```

## Benefits of This Approach

1. **No Build Step**: Components can be edited and immediately reflected in both apps
2. **Simplified Development**: No need to manage separate builds or watch processes
3. **TypeScript Integration**: Full type checking across all workspaces
4. **Dependency Management**: No duplicate dependencies or version conflicts
5. **Consistent Experience**: Same component implementation on all platforms

## Troubleshooting Common Issues

1. **TypeScript Errors**: Ensure the consuming app includes UI source files in its compilation scope
2. **Module Resolution**: Check path mappings in tsconfig.json if imports aren't resolving
3. **React Native Web**: Make sure react-native-web is properly set up in the web application
4. **React Import**: Always include `import React from 'react'` in component files
5. **Platform Differences**: Use platform-specific files (.native.tsx) for significant platform divergence

## Conclusion

By using this direct source import approach, we maintain a streamlined development workflow while ensuring that UI components work consistently across all OpenAgents applications. Instead of relying on a separate build process for the UI components, we leverage TypeScript's path mapping and workspace configuration to allow direct imports from the source files, resulting in a more efficient development experience.