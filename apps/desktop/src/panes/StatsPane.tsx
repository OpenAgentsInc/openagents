import React, { useState, useEffect } from "react";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import { BarChart, Clock, TrendingUp, Loader2, RefreshCw, Eye } from "lucide-react";
import { HistoricalAPMChart } from "@/components/charts/HistoricalAPMChart";

interface ToolUsage {
  name: string;
  count: number;
  percentage: number;
  category: string;
}

interface APMSession {
  id: string;
  project: string;
  apm: number;
  duration: number;
  messageCount: number;
  toolCount: number;
  timestamp: string;
}

interface APMStats {
  apm1h: number;
  apm6h: number;
  apm1d: number;
  apm1w: number;
  apm1m: number;
  apmLifetime: number;
  totalSessions: number;
  totalMessages: number;
  totalToolUses: number;
  totalDuration: number;
  toolUsage: ToolUsage[];
  recentSessions: APMSession[];
  productivityByTime: {
    morning: number;
    afternoon: number;
    evening: number;
    night: number;
  };
}

interface CombinedAPMStats extends APMStats {
  cliStats: APMStats;
  sdkStats: APMStats;
}

interface StatsPaneProps {
  // Props will be passed from PaneManager
}

// Removed skill tier functionality

const formatDuration = (minutes: number): string => {
  if (minutes < 60) return `${minutes.toFixed(1)}m`;
  const hours = Math.floor(minutes / 60);
  const mins = Math.floor(minutes % 60);
  return `${hours}h ${mins}m`;
};

export const StatsPane: React.FC<StatsPaneProps> = () => {
  const [stats, setStats] = useState<CombinedAPMStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<'combined' | 'cli' | 'sdk'>('combined');

  const loadStats = async (isRefresh = false) => {
    try {
      if (isRefresh) {
        setRefreshing(true);
      } else {
        setLoading(true);
      }
      setError(null);
      
      const result = await invoke('analyze_combined_conversations');
      
      if (result && typeof result === 'object' && 'success' in result) {
        const response = result as { success: boolean; data?: CombinedAPMStats; error?: string };
        
        if (response.success && response.data) {
          setStats(response.data);
        } else {
          setError(response.error || 'Failed to load APM statistics');
        }
      } else {
        setError('Invalid response format');
      }
    } catch (err) {
      console.error('Error loading APM stats:', err);
      setError(err instanceof Error ? err.message : 'Failed to load statistics');
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  useEffect(() => {
    loadStats();
  }, []);

  // Auto-refresh every 10 seconds
  useEffect(() => {
    const interval = setInterval(() => {
      if (stats) { // Only auto-refresh if we have existing stats
        loadStats(true);
      }
    }, 10000);

    return () => clearInterval(interval);
  }, [stats]);

  // Helper function to get current view's stats
  const getCurrentStats = (): APMStats | null => {
    if (!stats) return null;
    
    switch (viewMode) {
      case 'cli':
        return stats.cliStats;
      case 'sdk':
        return stats.sdkStats;
      case 'combined':
      default:
        return stats;
    }
  };

  const currentStats = getCurrentStats();

  // Helper to get view display name
  const getViewDisplayName = (mode: string) => {
    switch (mode) {
      case 'cli':
        return 'CLI Only';
      case 'sdk':
        return 'SDK Only';
      case 'combined':
      default:
        return 'Combined';
    }
  };

  if (loading && !stats) {
    return (
      <div className="flex flex-col h-full">
        <div className="flex items-center justify-center h-full">
          <div className="text-center">
            <Loader2 className="h-8 w-8 animate-spin mx-auto mb-4 text-muted-foreground" />
            <p className="text-muted-foreground">Analyzing conversations...</p>
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col h-full">
        <div className="text-center select-none mb-4">
          <h1 className="text-xl font-bold mb-1">APM Statistics</h1>
          <p className="text-muted-foreground text-xs">Actions Per Minute Analysis</p>
        </div>
        
        <Separator className="my-4" />
        
        <div className="flex items-center justify-center h-full">
          <div className="text-center">
            <p className="text-red-400 mb-4">{error}</p>
            <Button onClick={() => loadStats(false)} variant="outline" size="sm" disabled={refreshing}>
              <RefreshCw className={`h-4 w-4 mr-2 ${refreshing ? 'animate-spin' : ''}`} />
              {refreshing ? 'Retrying...' : 'Retry'}
            </Button>
          </div>
        </div>
      </div>
    );
  }

  if (!stats) {
    return (
      <div className="flex flex-col h-full">
        <div className="text-center select-none mb-4">
          <h1 className="text-xl font-bold mb-1">APM Statistics</h1>
          <p className="text-muted-foreground text-xs">Actions Per Minute Analysis</p>
        </div>
        
        <Separator className="my-4" />
        
        <div className="flex items-center justify-center h-full">
          <p className="text-muted-foreground">No data available</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="text-center select-none mb-4">
        <div className="flex items-center justify-center gap-2 mb-2">
          <BarChart className="h-5 w-5" />
          <h1 className="text-xl font-bold">APM Statistics</h1>
        </div>
        <p className="text-muted-foreground text-xs mb-3">Actions Per Minute Analysis</p>
        
        {/* View Mode Switcher */}
        <div className="flex items-center justify-center gap-1">
          {(['combined', 'cli', 'sdk'] as const).map((mode) => (
            <Button
              key={mode}
              variant={viewMode === mode ? "default" : "outline"}
              size="sm"
              onClick={() => setViewMode(mode)}
              className="text-xs h-7 px-2"
            >
              {getViewDisplayName(mode)}
            </Button>
          ))}
        </div>
      </div>

      <Separator className="my-4" />

      <div className="flex-1 overflow-auto space-y-6">
        {/* Time Window APM Cards */}
        <div className="grid grid-cols-3 gap-3">
          <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2 mb-2">
              <Clock className="h-4 w-4 text-red-400" />
              <span className="text-sm font-medium">1 Hour</span>
            </div>
            <div className="text-xl font-bold">{currentStats?.apm1h.toFixed(1) || '0.0'}</div>
            <div className="text-xs text-muted-foreground">APM</div>
          </div>
          
          <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2 mb-2">
              <Clock className="h-4 w-4 text-orange-400" />
              <span className="text-sm font-medium">6 Hours</span>
            </div>
            <div className="text-xl font-bold">{currentStats?.apm6h.toFixed(2) || '0.00'}</div>
            <div className="text-xs text-muted-foreground">APM</div>
          </div>

          <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2 mb-2">
              <Clock className="h-4 w-4 text-yellow-400" />
              <span className="text-sm font-medium">1 Day</span>
            </div>
            <div className="text-xl font-bold">{currentStats?.apm1d.toFixed(3) || '0.000'}</div>
            <div className="text-xs text-muted-foreground">APM</div>
          </div>

          <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2 mb-2">
              <Clock className="h-4 w-4 text-green-400" />
              <span className="text-sm font-medium">1 Week</span>
            </div>
            <div className="text-xl font-bold">{currentStats?.apm1w.toFixed(3) || '0.000'}</div>
            <div className="text-xs text-muted-foreground">APM</div>
          </div>

          <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2 mb-2">
              <Clock className="h-4 w-4 text-blue-400" />
              <span className="text-sm font-medium">1 Month</span>
            </div>
            <div className="text-xl font-bold">{currentStats?.apm1m.toFixed(3) || '0.000'}</div>
            <div className="text-xs text-muted-foreground">APM</div>
          </div>

          <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2 mb-2">
              <TrendingUp className="h-4 w-4 text-purple-400" />
              <span className="text-sm font-medium">Lifetime</span>
            </div>
            <div className="text-xl font-bold">{currentStats?.apmLifetime.toFixed(3) || '0.000'}</div>
            <div className="text-xs text-muted-foreground">APM</div>
          </div>
        </div>

        {/* Time Window Explanations */}
        <div className="bg-card rounded-lg border p-4">
          <h3 className="font-semibold mb-3 text-sm">Time Window Explanations</h3>
          <div className="space-y-2 text-xs text-muted-foreground">
            <div className="flex items-start gap-2">
              <span className="text-red-400 font-medium min-w-0 flex-shrink-0">1 Hour:</span>
              <span>Actions per minute over the last 60 minutes</span>
            </div>
            <div className="flex items-start gap-2">
              <span className="text-orange-400 font-medium min-w-0 flex-shrink-0">6 Hours:</span>
              <span>Actions per minute over the last 6 hours</span>
            </div>
            <div className="flex items-start gap-2">
              <span className="text-yellow-400 font-medium min-w-0 flex-shrink-0">1 Day:</span>
              <span>Actions per minute over the last 24 hours</span>
            </div>
            <div className="flex items-start gap-2">
              <span className="text-green-400 font-medium min-w-0 flex-shrink-0">1 Week:</span>
              <span>Actions per minute over the last 7 days</span>
            </div>
            <div className="flex items-start gap-2">
              <span className="text-blue-400 font-medium min-w-0 flex-shrink-0">1 Month:</span>
              <span>Actions per minute over the last 30 days</span>
            </div>
            <div className="flex items-start gap-2">
              <span className="text-purple-400 font-medium min-w-0 flex-shrink-0">Lifetime:</span>
              <span>Actions per minute from your first to most recent conversation</span>
            </div>
          </div>
        </div>

        {/* Historical APM Chart */}
        <HistoricalAPMChart viewMode={viewMode} />

        {/* Top Tools */}
        <div className="bg-card rounded-lg border p-4">
          <h3 className="font-semibold mb-3">Top Tools</h3>
          <div className="space-y-2">
            {(currentStats?.toolUsage || []).slice(0, 5).map((tool, index) => (
              <div key={tool.name} className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="text-xs bg-muted px-2 py-1 rounded">
                    #{index + 1}
                  </span>
                  <span className="text-sm font-medium">{tool.name}</span>
                </div>
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <span>{tool.count}</span>
                  <span>({tool.percentage.toFixed(1)}%)</span>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Productivity by Time */}
        <div className="bg-card rounded-lg border p-4">
          <div className="flex items-center gap-2 mb-3">
            <Clock className="h-4 w-4" />
            <h3 className="font-semibold">Productivity by Time</h3>
          </div>
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-sm">Morning (6-12)</span>
              <span className="text-sm font-medium">{currentStats?.productivityByTime.morning.toFixed(1) || '0.0'} APM</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Afternoon (12-18)</span>
              <span className="text-sm font-medium">{currentStats?.productivityByTime.afternoon.toFixed(1) || '0.0'} APM</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Evening (18-24)</span>
              <span className="text-sm font-medium">{currentStats?.productivityByTime.evening.toFixed(1) || '0.0'} APM</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Night (0-6)</span>
              <span className="text-sm font-medium">{currentStats?.productivityByTime.night.toFixed(1) || '0.0'} APM</span>
            </div>
          </div>
        </div>

        {/* Recent Sessions */}
        <div className="bg-card rounded-lg border p-4">
          <h3 className="font-semibold mb-3">Recent Sessions</h3>
          <div className="space-y-2 max-h-48 overflow-auto">
            {(currentStats?.recentSessions || []).slice(0, 10).map((session) => (
              <div key={session.id} className="flex items-center justify-between text-sm border-b border-border/50 pb-2">
                <div className="flex-1 min-w-0">
                  <div className="font-medium truncate">{session.project}</div>
                  <div className="text-muted-foreground text-xs">
                    {formatDuration(session.duration)} • {session.messageCount} messages • {session.toolCount} tools
                  </div>
                </div>
                <div className="flex items-center gap-2 ml-2">
                  <span className="font-medium">{session.apm.toFixed(1)}</span>
                  <Button 
                    variant="ghost" 
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={() => {/* TODO: Implement session detail view */}}
                  >
                    <Eye className="h-3 w-3" />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Summary Stats */}
        <div className="bg-card rounded-lg border p-4">
          <h3 className="font-semibold mb-3">Summary</h3>
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <div className="text-muted-foreground">Total Sessions</div>
              <div className="font-medium">{currentStats?.totalSessions || 0}</div>
            </div>
            <div>
              <div className="text-muted-foreground">Total Messages</div>
              <div className="font-medium">{(currentStats?.totalMessages || 0).toLocaleString()}</div>
            </div>
            <div>
              <div className="text-muted-foreground">Total Tools</div>
              <div className="font-medium">{(currentStats?.totalToolUses || 0).toLocaleString()}</div>
            </div>
            <div>
              <div className="text-muted-foreground">Total Time</div>
              <div className="font-medium">{formatDuration(currentStats?.totalDuration || 0)}</div>
            </div>
          </div>
        </div>

        {/* Breakdown Comparison (only show in combined mode) */}
        {viewMode === 'combined' && stats && (
          <div className="bg-card rounded-lg border p-4">
            <h3 className="font-semibold mb-3">CLI vs SDK Breakdown</h3>
            <div className="space-y-4">
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div className="font-medium text-muted-foreground">Metric</div>
                <div className="font-medium text-blue-600">CLI Only</div>
                <div className="font-medium text-green-600">SDK Only</div>
              </div>
              
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div>Sessions</div>
                <div>{stats.cliStats.totalSessions}</div>
                <div>{stats.sdkStats.totalSessions}</div>
              </div>
              
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div>Messages</div>
                <div>{stats.cliStats.totalMessages.toLocaleString()}</div>
                <div>{stats.sdkStats.totalMessages.toLocaleString()}</div>
              </div>
              
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div>Tool Uses</div>
                <div>{stats.cliStats.totalToolUses.toLocaleString()}</div>
                <div>{stats.sdkStats.totalToolUses.toLocaleString()}</div>
              </div>
              
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div>1 Hour APM</div>
                <div>{stats.cliStats.apm1h.toFixed(1)}</div>
                <div>{stats.sdkStats.apm1h.toFixed(1)}</div>
              </div>
              
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div>1 Day APM</div>
                <div>{stats.cliStats.apm1d.toFixed(3)}</div>
                <div>{stats.sdkStats.apm1d.toFixed(3)}</div>
              </div>
              
              <div className="grid grid-cols-3 gap-4 text-sm">
                <div>Lifetime APM</div>
                <div>{stats.cliStats.apmLifetime.toFixed(3)}</div>
                <div>{stats.sdkStats.apmLifetime.toFixed(3)}</div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="mt-4 pt-4 border-t border-border">
        <Button 
          onClick={() => loadStats(false)} 
          variant="outline" 
          size="sm" 
          className="w-full" 
          disabled={refreshing}
        >
          <RefreshCw className={`h-4 w-4 mr-2 ${refreshing ? 'animate-spin' : ''}`} />
          {refreshing ? 'Refreshing Stats...' : 'Refresh Stats'}
        </Button>
      </div>
    </div>
  );
};