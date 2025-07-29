import React, { useEffect, useState, useCallback } from "react";
import { useQuery } from 'convex/react';
import { api } from '../../convex/_generated/api';
import { TrendingUpIcon, TrendingDownIcon, ActivityIcon, ClockIcon } from "../icons/React19Icons";

interface RealtimeAPMDisplayProps {
  /** Device ID for tracking */
  deviceId?: string;
  /** Show trend indicator */
  showTrend?: boolean;
  /** Show session details */
  showDetails?: boolean;
  /** Compact mode for smaller display */
  compact?: boolean;
  /** Custom className */
  className?: string;
  /** Update callback */
  onUpdate?: (data: RealtimeAPMData) => void;
  /** Error callback */
  onError?: (error: any) => void;
}

interface RealtimeAPMData {
  currentAPM: number;
  trend: 'up' | 'down' | 'stable';
  sessionDuration: number;
  totalActions: number;
  lastUpdateTimestamp: number;
  isActive: boolean;
  deviceId: string;
  trendPercentage?: number;
}

export const RealtimeAPMDisplay: React.FC<RealtimeAPMDisplayProps> = ({
  deviceId,
  showTrend = true,
  showDetails = true,
  compact = false,
  className = "",
  onUpdate,
  onError,
}) => {
  const [isAnimating, setIsAnimating] = useState(false);

  // Query realtime APM data from Convex
  const realtimeAPMData = useQuery(
    api.confect.apm.getRealtimeAPM,
    { deviceId, includeHistory: true }
  );

  // Handle data updates
  useEffect(() => {
    try {
      if (realtimeAPMData) {
        onUpdate?.(realtimeAPMData);
        
        // Trigger animation for visual feedback
        setIsAnimating(true);
        const timer = setTimeout(() => setIsAnimating(false), 500);
        return () => clearTimeout(timer);
      }
    } catch (error) {
      onError?.(error);
    }
    // Return undefined if no cleanup needed
    return undefined;
  }, [realtimeAPMData, onUpdate, onError]);

  const getTrendIcon = (trend: 'up' | 'down' | 'stable') => {
    switch (trend) {
      case 'up':
        return <TrendingUpIcon className="w-4 h-4 text-green-500" />;
      case 'down':
        return <TrendingDownIcon className="w-4 h-4 text-red-500" />;
      default:
        return <ActivityIcon className="w-4 h-4 text-gray-400" />;
    }
  };

  const getTrendColor = (trend: 'up' | 'down' | 'stable') => {
    switch (trend) {
      case 'up':
        return 'text-green-500';
      case 'down':
        return 'text-red-500';
      default:
        return 'text-gray-400';
    }
  };

  const formatDuration = (ms: number): string => {
    const minutes = Math.floor(ms / 60000);
    const seconds = Math.floor((ms % 60000) / 1000);
    
    if (minutes > 0) {
      return `${minutes}m ${seconds}s`;
    }
    return `${seconds}s`;
  };

  const formatTimestamp = (timestamp: number): string => {
    const now = Date.now();
    const diff = now - timestamp;
    
    if (diff < 60000) {
      return 'now';
    } else if (diff < 3600000) {
      return `${Math.floor(diff / 60000)}m ago`;
    } else {
      return new Date(timestamp).toLocaleTimeString();
    }
  };

  if (!realtimeAPMData) {
    return (
      <div className={`realtime-apm-display loading ${className}`}>
        <div className={compact ? "compact-layout" : "full-layout"}>
          <div className="flex items-center gap-2">
            <div className="animate-spin w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full" />
            <span className="text-sm text-gray-400">Loading APM...</span>
          </div>
        </div>
      </div>
    );
  }

  const {
    currentAPM,
    trend,
    sessionDuration,
    totalActions,
    lastUpdateTimestamp,
    isActive,
    trendPercentage,
  } = realtimeAPMData;

  if (compact) {
    return (
      <div className={`realtime-apm-display compact ${className} ${isAnimating ? 'animate-pulse' : ''}`}>
        <div className="flex items-center gap-3 p-3 bg-gray-800/50 rounded-lg border border-gray-700/50">
          <div className="flex items-center gap-1">
            <span className="text-2xl font-bold text-white">
              {currentAPM.toFixed(1)}
            </span>
            <span className="text-xs text-gray-400 uppercase tracking-wide">APM</span>
          </div>
          
          {showTrend && (
            <div className="flex items-center gap-1">
              {getTrendIcon(trend)}
              {trendPercentage && Math.abs(trendPercentage) >= 10 && (
                <span className={`text-xs font-medium ${getTrendColor(trend)}`}>
                  {trendPercentage > 0 ? '+' : ''}{trendPercentage.toFixed(0)}%
                </span>
              )}
            </div>
          )}

          <div className={`w-2 h-2 rounded-full ${isActive ? 'bg-green-400' : 'bg-gray-500'}`} />
        </div>
      </div>
    );
  }

  return (
    <div className={`realtime-apm-display full ${className} ${isAnimating ? 'animate-pulse' : ''}`}>
      <div className="p-4 bg-gray-800/50 rounded-lg border border-gray-700/50 space-y-4">
        {/* Header */}
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-gray-300 uppercase tracking-wide">
            Current Session APM
          </h3>
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${isActive ? 'bg-green-400' : 'bg-gray-500'}`} />
            <span className="text-xs text-gray-400">
              {isActive ? 'Active' : 'Inactive'}
            </span>
          </div>
        </div>

        {/* Main APM Display */}
        <div className="flex items-center justify-between">
          <div className="flex items-baseline gap-2">
            <span className="text-4xl font-bold text-white tabular-nums">
              {currentAPM.toFixed(1)}
            </span>
            <span className="text-lg text-gray-400 font-medium">APM</span>
          </div>

          {showTrend && (
            <div className="flex items-center gap-2">
              {getTrendIcon(trend)}
              {trendPercentage && Math.abs(trendPercentage) >= 10 && (
                <div className="flex flex-col items-end">
                  <span className={`text-sm font-semibold ${getTrendColor(trend)}`}>
                    {trendPercentage > 0 ? '+' : ''}{trendPercentage.toFixed(1)}%
                  </span>
                  <span className="text-xs text-gray-500 capitalize">{trend}</span>
                </div>
              )}
            </div>
          )}
        </div>

        {/* Session Details */}
        {showDetails && (
          <div className="grid grid-cols-3 gap-4 pt-3 border-t border-gray-700/50">
            <div className="text-center">
              <div className="flex items-center justify-center gap-1 mb-1">
                <ActivityIcon className="w-3 h-3 text-blue-400" />
                <span className="text-xs text-gray-400 uppercase tracking-wide">Actions</span>
              </div>
              <span className="text-lg font-semibold text-white tabular-nums">
                {totalActions}
              </span>
            </div>

            <div className="text-center">
              <div className="flex items-center justify-center gap-1 mb-1">
                <ClockIcon className="w-3 h-3 text-purple-400" />
                <span className="text-xs text-gray-400 uppercase tracking-wide">Duration</span>
              </div>
              <span className="text-lg font-semibold text-white">
                {formatDuration(sessionDuration)}
              </span>
            </div>

            <div className="text-center">
              <div className="flex items-center justify-center gap-1 mb-1">
                <div className="w-3 h-3 rounded-full bg-gray-400" />
                <span className="text-xs text-gray-400 uppercase tracking-wide">Updated</span>
              </div>
              <span className="text-lg font-semibold text-white">
                {formatTimestamp(lastUpdateTimestamp)}
              </span>
            </div>
          </div>
        )}

        {/* Progress Indicator */}
        <div className="w-full bg-gray-700 rounded-full h-1">
          <div 
            className={`h-1 rounded-full transition-all duration-1000 ${
              trend === 'up' ? 'bg-gradient-to-r from-green-500 to-green-400' :
              trend === 'down' ? 'bg-gradient-to-r from-red-500 to-red-400' :
              'bg-gradient-to-r from-gray-500 to-gray-400'
            }`}
            style={{ 
              width: `${Math.min(100, Math.max(5, (currentAPM / 10) * 100))}%` 
            }}
          />
        </div>
      </div>
    </div>
  );
};

// Hook for easy integration with existing desktop components
export const useDesktopRealtimeAPM = (deviceId?: string) => {
  const realtimeAPMData = useQuery(
    api.confect.apm.getRealtimeAPM,
    { deviceId, includeHistory: false }
  );

  const getCurrentAPM = useCallback((): number => {
    return realtimeAPMData?.currentAPM ?? 0;
  }, [realtimeAPMData?.currentAPM]);

  const getSessionInfo = useCallback(() => {
    if (!realtimeAPMData) {
      return {
        duration: 0,
        totalActions: 0,
        apm: 0,
        isActive: false,
      };
    }

    return {
      duration: realtimeAPMData.sessionDuration,
      totalActions: realtimeAPMData.totalActions,
      apm: realtimeAPMData.currentAPM,
      isActive: realtimeAPMData.isActive,
    };
  }, [realtimeAPMData]);

  const getTrendInfo = useCallback(() => {
    if (!realtimeAPMData) {
      return {
        trend: 'stable' as const,
        percentage: 0,
      };
    }

    return {
      trend: realtimeAPMData.trend,
      percentage: realtimeAPMData.trendPercentage ?? 0,
    };
  }, [realtimeAPMData?.trend, realtimeAPMData?.trendPercentage]);

  return {
    data: realtimeAPMData,
    isLoading: realtimeAPMData === undefined,
    getCurrentAPM,
    getSessionInfo,
    getTrendInfo,
  };
};

export default RealtimeAPMDisplay;