# Shared UI Components for Coder and Onyx

This document outlines the architecture and implementation approach for creating shared UI components between the Tauri-based Coder application and the React Native-based Onyx application.

## Overview

We need to create a shared UI component library (`@openagents/ui`) that works across:
1. Coder - React + Tauri (Web/Desktop)
2. Onyx - React Native (Mobile)

The key challenge is to create components that adapt to each platform's rendering capabilities while maintaining a consistent API.

## Proposed Architecture

### Option 1: Platform-specific Implementations with Unified API

```
packages/ui/
├── src/
│   ├── components/
│   │   ├── Button/
│   │   │   ├── Button.tsx             # Default export (web)
│   │   │   ├── Button.native.tsx      # React Native implementation
│   │   │   ├── Button.styles.ts       # Shared styles (if applicable)
│   │   │   ├── Button.types.ts        # Shared types
│   │   │   └── index.ts               # Re-export
│   │   └── ... (other components)
│   └── index.ts                       # Main export
├── package.json
└── tsconfig.json
```

With this approach:
- Each component has platform-specific implementations
- The bundler (Metro for RN, Vite for Tauri) picks the correct file based on platform
- Components share a consistent API defined in `.types.ts` files

### Option 2: React Native Web Approach

```
packages/ui/
├── src/
│   ├── components/
│   │   ├── Button/
│   │   │   ├── Button.tsx             # React Native implementation
│   │   │   ├── Button.styles.ts       # RN styles
│   │   │   └── index.ts               # Re-export
│   │   └── ... (other components)
│   └── index.ts                       # Main export
├── package.json
└── tsconfig.json
```

With this approach:
- Build components using React Native primitives
- Use `react-native-web` to render them in Coder (web context)
- Single implementation that adapts to platform capabilities

## Implementation Plan

### Phase 1: Setup Package Structure

1. Create the basic package structure and configuration
2. Set up build tools for web and native targets
3. Define shared type system

### Phase 2: Create Base Components

1. Implement Button component as proof-of-concept
2. Add basic styling system that works cross-platform
3. Create thorough tests for each platform

### Phase 3: Integration with Apps

1. Add the package as a dependency in both apps
2. Use the Button component in each
3. Adjust as needed based on real-world usage

## Technical Requirements

### Dependencies

For Option 1 (Separate Implementations):
- `react`: ^18.3.1
- `react-dom`: ^18.3.1 (web/Tauri)
- `react-native`: ^0.76.7 (mobile)
- TypeScript for type sharing

For Option 2 (React Native Web):
- `react`: ^18.3.1
- `react-native`: ^0.76.7
- `react-native-web`: latest version
- `@types/react-native-web`

### Build Configuration

- TypeScript configuration that supports both targets
- Package.json with appropriate entry points:
  ```json
  {
    "main": "dist/index.js",       // Default for Node.js
    "module": "dist/index.mjs",    // ESM
    "react-native": "src/index.ts" // React Native
  }
  ```

## Recommended Approach

Given the current state of both applications and the desire for consistency, **Option 2 with react-native-web** is recommended as the primary approach:

1. It provides a more consistent component experience
2. It simplifies maintenance (one implementation vs. two)
3. react-native-web is mature and widely used in production applications

However, we should be prepared to create platform-specific overrides where needed for optimal UX on each platform.

## Next Steps

1. Create the base package structure
2. Implement a Button component as proof-of-concept
3. Integrate with both apps
4. Evaluate the real-world performance and developer experience