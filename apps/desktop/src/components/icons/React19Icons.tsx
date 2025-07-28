/**
 * React 19 Compatible Icon Wrappers
 * 
 * This file provides React 19 compatible wrappers for Lucide React icons
 * that have ForwardRefExoticComponent types which aren't directly compatible
 * with React 19's stricter JSX component type checking.
 */

import React from 'react'
import {
  BarChart,
  Clock,
  TrendingUp,
  Loader2,
  RefreshCw,
  Eye,
  X,
  PanelLeftIcon,
  XIcon,
} from 'lucide-react'
import type { LucideProps } from 'lucide-react'

// Icon wrappers that use type assertion for React 19 compatibility
export const BarChartIcon: React.FC<LucideProps> = (props) => React.createElement(BarChart as React.ComponentType<LucideProps>, props)
export const ClockIcon: React.FC<LucideProps> = (props) => React.createElement(Clock as React.ComponentType<LucideProps>, props)
export const TrendingUpIcon: React.FC<LucideProps> = (props) => React.createElement(TrendingUp as React.ComponentType<LucideProps>, props)
export const LoaderIcon: React.FC<LucideProps> = (props) => React.createElement(Loader2 as React.ComponentType<LucideProps>, props)
export const RefreshIcon: React.FC<LucideProps> = (props) => React.createElement(RefreshCw as React.ComponentType<LucideProps>, props)
export const EyeIcon: React.FC<LucideProps> = (props) => React.createElement(Eye as React.ComponentType<LucideProps>, props)

// Close icon wrappers (for different import patterns)
export const CloseIcon: React.FC<LucideProps> = (props) => React.createElement(X as React.ComponentType<LucideProps>, props)
export const CloseIconAlt: React.FC<LucideProps> = (props) => React.createElement(XIcon as React.ComponentType<LucideProps>, props)

// Panel toggle icon
export const PanelToggleIcon: React.FC<LucideProps> = (props) => React.createElement(PanelLeftIcon as React.ComponentType<LucideProps>, props)