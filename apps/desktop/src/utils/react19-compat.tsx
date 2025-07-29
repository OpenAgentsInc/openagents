import React, { ReactNode } from 'react';
import { ConvexProvider, ConvexReactClient } from 'convex/react';

/**
 * React 19 compatible wrapper for ConvexProvider
 * 
 * React 19 has stricter type checking for ReactNode, and ConvexProvider's
 * return type 'ReactNode | Promise<ReactNode>' doesn't satisfy the new
 * React 19 component expectations. This wrapper ensures type compatibility.
 */
export const ConvexProviderReact19: React.FC<{ 
  client: ConvexReactClient; 
  children: ReactNode 
}> = ({ client, children }) => {
  return ConvexProvider({ client, children }) as React.ReactElement;
};