/**
 * React 19 Compatible Convex Provider Wrapper
 * 
 * This file provides a React 19 compatible wrapper for ConvexProvider
 * that has FunctionComponent types which aren't directly compatible
 * with React 19's stricter JSX component type checking.
 */

import React from 'react'
import { ConvexProvider as _ConvexProvider } from 'convex/react'
import type { ConvexReactClient } from 'convex/react'

interface ConvexProviderProps {
  client: ConvexReactClient
  children?: React.ReactNode
}

// Wrapper that is properly typed for React 19
export const ConvexProvider: React.FC<ConvexProviderProps> = ({ client, children }) => {
  return React.createElement(_ConvexProvider as React.ComponentType<ConvexProviderProps>, { client, children })
}