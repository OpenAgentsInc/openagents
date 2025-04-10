/**
 * React 19 compatibility utilities
 *
 * This file provides utilities to fix React Native component compatibility issues
 * with React 19. In React 19, the ReactNode type definition changed which caused
 * type errors with React Native components.
 */
import React, { ComponentType, ForwardRefExoticComponent, PropsWithoutRef, RefAttributes } from 'react';

/**
 * React 19 compatibility declarations
 *
 * This provides utility functions to make React Native components work with React 19's
 * updated ReactNode type definition. React 19 removed bigint from ReactNode and also
 * changed some type definitions which created compatibility issues with existing components.
 *
 * This utility provides:
 * 1. createReactComponent - A function to create React 19 compatible components
 * 2. Pre-wrapped common React Native components
 * 3. react19 - A namespace with utility functions for third-party component libraries
 */

// Instead of directly modifying the ReactNode type, we use a type assertion approach
// This avoids the 'duplicate identifier' error

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

// Re-exports for components that need React 19 compatibility
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

// Create a placeholder/fallback Markdown component for web
export const Markdown = createReactComponent(
  (props: any) => {
    // Simple fallback that just renders the children or markdown text as plain text
    return React.createElement('div', {
      className: 'markdown-content',
      style: { whiteSpace: 'pre-wrap' }
    }, props.children || props.content || '');
  }
);
