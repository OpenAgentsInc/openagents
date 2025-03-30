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
 * updated ReactNode type definition. React 19 removed bigint from ReactNode.
 */

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