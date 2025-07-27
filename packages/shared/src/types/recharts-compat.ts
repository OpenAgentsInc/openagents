/**
 * Recharts React 19 Compatibility Module
 * 
 * This module provides centralized utilities for Recharts React 19 compatibility.
 * Due to TypeScript module boundary limitations with type assertions, this module
 * provides helper functions and documentation for proper Recharts usage with React 19.
 * 
 * @see https://github.com/recharts/recharts/issues/3615
 * @see https://github.com/recharts/recharts/issues/3584
 * 
 * Uses inline type assertions per component file (confirmed working approach)
 */

import React from 'react';

/**
 * Utility function to create React 19 compatible Recharts component
 * Usage in component files:
 * 
 * ```tsx
 * import { LineChart, Line } from 'recharts';
 * import { createRechartsComponent } from '@openagents/shared/types/recharts-compat';
 * 
 * const RechartsLineChart = createRechartsComponent(LineChart);
 * const RechartsLine = createRechartsComponent(Line);
 * ```
 */
export function createRechartsComponent<T>(component: T): React.ComponentType<any> {
  return component as unknown as React.ComponentType<any>;
}

/**
 * Recommended inline pattern for React 19 compatibility
 * Copy this pattern into component files that use Recharts:
 * 
 * ```tsx
 * import {
 *   LineChart,
 *   Line,
 *   XAxis,
 *   YAxis,
 *   CartesianGrid,
 *   Tooltip,
 *   ResponsiveContainer,
 *   Legend,
 * } from 'recharts';
 * 
 * // Type assertions for Recharts components to resolve React 19 compatibility issues
 * const RechartsLineChart = LineChart as unknown as React.ComponentType<any>;
 * const RechartsLine = Line as unknown as React.ComponentType<any>;
 * const RechartsXAxis = XAxis as unknown as React.ComponentType<any>;
 * const RechartsYAxis = YAxis as unknown as React.ComponentType<any>;
 * const RechartsCartesianGrid = CartesianGrid as unknown as React.ComponentType<any>;
 * const RechartsTooltip = Tooltip as unknown as React.ComponentType<any>;
 * const RechartsResponsiveContainer = ResponsiveContainer as unknown as React.ComponentType<any>;
 * const RechartsLegend = Legend as unknown as React.ComponentType<any>;
 * ```
 */
export const RECHARTS_INLINE_PATTERN_EXAMPLE = `
// Recommended pattern for each Recharts component file:
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend } from 'recharts';

const RechartsLineChart = LineChart as unknown as React.ComponentType<any>;
const RechartsLine = Line as unknown as React.ComponentType<any>;
const RechartsXAxis = XAxis as unknown as React.ComponentType<any>;
const RechartsYAxis = YAxis as unknown as React.ComponentType<any>;
const RechartsCartesianGrid = CartesianGrid as unknown as React.ComponentType<any>;
const RechartsTooltip = Tooltip as unknown as React.ComponentType<any>;
const RechartsResponsiveContainer = ResponsiveContainer as unknown as React.ComponentType<any>;
const RechartsLegend = Legend as unknown as React.ComponentType<any>;
`;

/**
 * Type definitions for common Recharts components
 * Use these for proper TypeScript intellisense while maintaining React 19 compatibility
 */
export type RechartsCompatComponent<T = any> = React.ComponentType<T>;

/**
 * Re-export original Recharts types for advanced usage
 * Use these when you need access to the original TypeScript interfaces
 */
export type {
  LineChart as OriginalLineChart,
  Line as OriginalLine,
  XAxis as OriginalXAxis,
  YAxis as OriginalYAxis,
  CartesianGrid as OriginalCartesianGrid,
  Tooltip as OriginalTooltip,
  ResponsiveContainer as OriginalResponsiveContainer,
  Legend as OriginalLegend,
} from 'recharts';

/**
 * Compatibility information for developers
 */
export const RECHARTS_COMPAT_INFO = {
  version: 'recharts@2.x',
  reactVersion: 'React 19',
  status: 'Type compatibility workaround active',
  issue: 'https://github.com/recharts/recharts/issues/3615',
  expectedResolution: 'When Recharts releases React 19 compatible types',
} as const;