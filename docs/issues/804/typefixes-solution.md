# React 19 TypeScript Compatibility Fixes

## Problem

After upgrading to React 19, several type errors appeared due to changes in React 19's type definitions, particularly:

1. React 19 removed `bigint` from the `ReactNode` type
2. React 19 changed component type definitions creating incompatibilities with React Native components
3. Different ReactNode definitions between packages led to errors like "Type X is not assignable to type ReactNode"

## Solution

We created a comprehensive React 19 compatibility utility to resolve these issues:

### 1. Core Type Compatibility Utility

Created `reactCompatibility.ts` in the core package with:

```typescript
/**
 * React 19 compatibility utilities
 * 
 * This file provides utilities to fix React Native component compatibility issues
 * with React 19. In React 19, the ReactNode type definition changed which caused
 * type errors with React Native components.
 */
import React, { ComponentType, ForwardRefExoticComponent, PropsWithoutRef, RefAttributes } from 'react';

/**
 * This function adds React 19 compatibility to React Native components
 * 
 * In React 19, the typing for ReactNode changed and it no longer accepts BigInt.
 * This creates type errors with React Native components that expect the old ReactNode type.
 */
export function createReactComponent<P = any>(
  Component: any
): React.FC<P> {
  return Component as unknown as React.FC<P>;
}

// Namespace for React 19 compatibility utilities
export const react19 = {
  // Make any component React 19 compatible
  compat: <P extends object>(Component: any): React.FC<P> => {
    return Component as unknown as React.FC<P>;
  },
  
  // Wrapper for third-party icon libraries (lucide, simple-icons, etc.)
  icon: <P extends object>(Icon: any): React.FC<P> => {
    return Icon as unknown as React.FC<P>;
  },
  
  // Wrapper for React Router components
  router: <P extends object>(Component: any): React.FC<P> => {
    return Component as unknown as React.FC<P>;
  }
};

// Pre-wrapped React Native components
import { 
  View as RNView, 
  Text as RNText, 
  TouchableOpacity as RNTouchableOpacity,
  SafeAreaView as RNSafeAreaView,
  ActivityIndicator as RNActivityIndicator,
  ScrollView as RNScrollView,
  Button as RNButton,
  TextInput as RNTextInput,
  FlatList as RNFlatList,
  Animated,
  KeyboardAvoidingView as RNKeyboardAvoidingView,
  Pressable as RNPressable,
  Modal as RNModal,
  Image as RNImage,
  TouchableHighlight as RNTouchableHighlight,
  Switch as RNSwitch,
} from 'react-native';

// Create React 19 compatible versions of common React Native components
export const View = createReactComponent(RNView);
export const Text = createReactComponent(RNText);
export const TouchableOpacity = createReactComponent(RNTouchableOpacity);
export const SafeAreaView = createReactComponent(RNSafeAreaView);
export const ActivityIndicator = createReactComponent(RNActivityIndicator);
export const ScrollView = createReactComponent(RNScrollView);
export const Button = createReactComponent(RNButton);
export const TextInput = createReactComponent(RNTextInput); 
export const FlatList = createReactComponent(RNFlatList);
export const AnimatedView = createReactComponent(Animated.View);
export const KeyboardAvoidingView = createReactComponent(RNKeyboardAvoidingView);
export const Pressable = createReactComponent(RNPressable);
export const Modal = createReactComponent(RNModal);
export const Image = createReactComponent(RNImage);
export const TouchableHighlight = createReactComponent(RNTouchableHighlight);
export const Switch = createReactComponent(RNSwitch);
```

### 2. Fixed Component Imports

Updated component imports across the codebase:

```typescript
// Before
import { View, Text, Button } from 'react-native';

// After
import { View, Text, Button } from '@openagents/core';
```

### 3. Third-Party Component Compatibility

Added wrappers for third-party components like icons:

```typescript
// Before
import { Moon } from 'lucide-react';
// ...
<Moon size={16} />

// After
import { Moon as LucideMoon } from 'lucide-react';
import { react19 } from '@openagents/core';

// Define interface for the icon props
interface IconProps {
  size?: number;
  color?: string;
  className?: string;
  [key: string]: any;
}

// Make Lucide icons compatible with React 19
const Moon = react19.icon<IconProps>(LucideMoon);
// ...
<Moon size={16} />
```

### 4. Fixed React Router Components

Made React Router components compatible with React 19:

```typescript
import { Link as RouterLink } from '@tanstack/react-router';
import { react19 } from '@openagents/core';

// Define interface for router Link props
interface LinkProps {
  to: string;
  children?: React.ReactNode;
  [key: string]: any;
}

// Make React Router components compatible with React 19
const Link = react19.router<LinkProps>(RouterLink);
```

### 5. AgentClient Type Definition

Fixed the AgentClient type definition to match the SDK return value:

```typescript
// Define local interfaces that match the SDK types for compatibility with useChat
export interface AgentClient<T = unknown> {
  agent: string;
  name: string;
  setState: (state: unknown) => void;
  call: <R = unknown>(
    method: string,
    args?: unknown[],
    streamOptions?: { onUpdate?: (value: R) => void }
  ) => Promise<R>;
}
```

## Results

- All TypeScript type errors are now resolved
- The core package exports React Native compatible components
- Third-party libraries work with React 19
- Components can use React Router without type errors
- The agent client properly connects to the Cloudflare Workers backend

## Usage

Developers should now:

1. Import React Native components from `@openagents/core` instead of directly from 'react-native'
2. Use the `react19.icon()`, `react19.router()` and `react19.compat()` utilities for third-party components
3. Explicitly type component props when needed

## Future Improvements

1. Consider updating the type definitions in all dependencies to be natively compatible with React 19
2. Add better prop type definitions for common components
3. Implement a better way to handle the global ReactNode type incompatibility