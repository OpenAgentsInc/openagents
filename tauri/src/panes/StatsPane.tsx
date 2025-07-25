import React, { useState, useEffect } from "react";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import { BarChart, Clock, Target, Trophy, TrendingUp, Loader2, RefreshCw, Eye } from "lucide-react";

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
  sessionBasedAPM: number;
  allTimeAPM: number;
  currentSessionAPM: number;
  totalSessions: number;
  totalMessages: number;
  totalToolUses: number;
  totalDuration: number;
  skillTier: string;
  tierColor: string;
  toolUsage: ToolUsage[];
  recentSessions: APMSession[];
  productivityByTime: {
    morning: number;
    afternoon: number;
    evening: number;
    night: number;
  };
}

interface StatsPaneProps {
  // Props will be passed from PaneManager
}

const getSkillTier = (apm: number): { tier: string; color: string; emoji: string } => {
  if (apm >= 200) return { tier: "Elite", color: "text-purple-400", emoji: "🟣" };
  if (apm >= 100) return { tier: "Professional", color: "text-red-400", emoji: "🔴" };
  if (apm >= 50) return { tier: "Productive", color: "text-orange-400", emoji: "🟠" };
  if (apm >= 25) return { tier: "Active", color: "text-yellow-400", emoji: "🟡" };
  if (apm >= 10) return { tier: "Casual", color: "text-green-400", emoji: "🟢" };
  return { tier: "Novice", color: "text-amber-600", emoji: "🟤" };
};

const formatDuration = (minutes: number): string => {
  if (minutes < 60) return `${minutes.toFixed(1)}m`;
  const hours = Math.floor(minutes / 60);
  const mins = Math.floor(minutes % 60);
  return `${hours}h ${mins}m`;
};

export const StatsPane: React.FC<StatsPaneProps> = () => {
  const [stats, setStats] = useState<APMStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadStats = async () => {
    try {
      setLoading(true);
      setError(null);
      
      const result = await invoke('analyze_claude_conversations');
      
      if (result && typeof result === 'object' && 'success' in result) {
        const response = result as { success: boolean; data?: APMStats; error?: string };
        
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
    }
  };

  useEffect(() => {
    loadStats();
  }, []);

  const skillTierInfo = stats ? getSkillTier(stats.sessionBasedAPM) : null;

  if (loading) {
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
            <Button onClick={loadStats} variant="outline" size="sm">
              <RefreshCw className="h-4 w-4 mr-2" />
              Retry
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
        <div className="flex items-center justify-center gap-2 mb-1">
          <BarChart className="h-5 w-5" />
          <h1 className="text-xl font-bold">APM Statistics</h1>
        </div>
        <p className="text-muted-foreground text-xs">Actions Per Minute Analysis</p>
      </div>

      <Separator className="my-4" />

      <div className="flex-1 overflow-auto space-y-6">
        {/* Overview Cards */}
        <div className="grid grid-cols-2 gap-4">
          <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2 mb-2">
              <Target className="h-4 w-4 text-blue-400" />
              <span className="text-sm font-medium">Session APM</span>
            </div>
            <div className="text-2xl font-bold">{stats.sessionBasedAPM.toFixed(1)}</div>
            <div className="text-xs text-muted-foreground">Active coding time only</div>
          </div>
          
          <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2 mb-2">
              <TrendingUp className="h-4 w-4 text-purple-400" />
              <span className="text-sm font-medium">All-Time APM</span>
            </div>
            <div className="text-2xl font-bold">{stats.allTimeAPM.toFixed(3)}</div>
            <div className="text-xs text-muted-foreground">From first to last conversation</div>
          </div>
        </div>

        {/* Current Session Card */}
        <div className="bg-card rounded-lg border p-4">
          <div className="flex items-center gap-2 mb-2">
            <Clock className="h-4 w-4 text-orange-400" />
            <span className="text-sm font-medium">Current Session APM</span>
          </div>
          <div className="text-2xl font-bold">{stats.currentSessionAPM.toFixed(1)}</div>
          <div className="text-xs text-muted-foreground">Most recent session</div>
        </div>

        {/* Skill Tier */}
        {skillTierInfo && (
          <div className="bg-card rounded-lg border p-4">
            <div className="flex items-center gap-2 mb-2">
              <Trophy className="h-4 w-4 text-yellow-400" />
              <span className="text-sm font-medium">Skill Tier</span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-lg">{skillTierInfo.emoji}</span>
              <span className={`text-xl font-bold ${skillTierInfo.color}`}>
                {skillTierInfo.tier}
              </span>
              <span className="text-sm text-muted-foreground">
                (Session APM: {stats.sessionBasedAPM.toFixed(1)})
              </span>
            </div>
          </div>
        )}

        {/* Top Tools */}
        <div className="bg-card rounded-lg border p-4">
          <h3 className="font-semibold mb-3">Top Tools</h3>
          <div className="space-y-2">
            {stats.toolUsage.slice(0, 5).map((tool, index) => (
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
              <span className="text-sm font-medium">{stats.productivityByTime.morning.toFixed(1)} APM</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Afternoon (12-18)</span>
              <span className="text-sm font-medium">{stats.productivityByTime.afternoon.toFixed(1)} APM</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Evening (18-24)</span>
              <span className="text-sm font-medium">{stats.productivityByTime.evening.toFixed(1)} APM</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Night (0-6)</span>
              <span className="text-sm font-medium">{stats.productivityByTime.night.toFixed(1)} APM</span>
            </div>
          </div>
        </div>

        {/* Recent Sessions */}
        <div className="bg-card rounded-lg border p-4">
          <h3 className="font-semibold mb-3">Recent Sessions</h3>
          <div className="space-y-2 max-h-48 overflow-auto">
            {stats.recentSessions.slice(0, 10).map((session) => (
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
              <div className="font-medium">{stats.totalSessions}</div>
            </div>
            <div>
              <div className="text-muted-foreground">Total Messages</div>
              <div className="font-medium">{stats.totalMessages.toLocaleString()}</div>
            </div>
            <div>
              <div className="text-muted-foreground">Total Tools</div>
              <div className="font-medium">{stats.totalToolUses.toLocaleString()}</div>
            </div>
            <div>
              <div className="text-muted-foreground">Total Time</div>
              <div className="font-medium">{formatDuration(stats.totalDuration)}</div>
            </div>
          </div>
        </div>
      </div>

      {/* Footer */}
      <div className="mt-4 pt-4 border-t border-border">
        <Button onClick={loadStats} variant="outline" size="sm" className="w-full">
          <RefreshCw className="h-4 w-4 mr-2" />
          Refresh Stats
        </Button>
      </div>
    </div>
  );
};