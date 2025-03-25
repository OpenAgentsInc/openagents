# Shared UI Components for Coder and Onyx

> **NOTE:** This document contains the original planning and proposals for the shared UI component approach. For the current implementation documentation, please see [Cross-Platform UI Components](./cross-platform-ui-components.md).

This document outlines the initial architecture and implementation approach options considered for creating shared UI components between OpenAgents applications.

## Original Proposal

We need to create a shared UI component library (`@openagents/ui`) that works across:
1. Coder - React + Electron (Desktop)
2. Onyx - React Native + Expo (Mobile)

The key challenge is to create components that adapt to each platform's rendering capabilities while maintaining a consistent API.

## Implementation Options Considered

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
```

With this approach:
- Each component has platform-specific implementations
- The bundler picks the correct file based on platform
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
```

With this approach:
- Build components using React Native primitives
- Use `react-native-web` to render them in web context
- Single implementation that adapts to platform capabilities

## Current Implementation

We chose a modified version of Option 2, using React Native Web but with direct source imports rather than a build step. This approach provides the best development experience while maintaining cross-platform compatibility.

For details on the current implementation, see:
- [Cross-Platform UI Components](./cross-platform-ui-components.md)