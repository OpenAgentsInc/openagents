// @ts-nocheck - Recharts has compatibility issues with React 18+ strict types
import React, { useState, useEffect } from 'react';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend } from 'recharts';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Loader2, TrendingUp, Calendar } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

// Types matching the Rust backend
interface HistoricalAPMDataPoint {
  period: string;
  cli_apm: number;
  sdk_apm: number;
  combined_apm: number;
  total_sessions: number;
  total_messages: number;
  total_tools: number;
  average_session_duration: number;
}

interface HistoricalAPMResponse {
  data: HistoricalAPMDataPoint[];
  time_scale: string;
  date_range: [string, string];
  view_mode: string;
}

interface CommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

interface HistoricalAPMChartProps {
  viewMode: 'combined' | 'cli' | 'sdk';
  className?: string;
}

export const HistoricalAPMChart: React.FC<HistoricalAPMChartProps> = ({
  viewMode,
  className = '',
}) => {
  const [data, setData] = useState<HistoricalAPMDataPoint[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [timeScale, setTimeScale] = useState<'daily' | 'weekly' | 'monthly'>('daily');

  const loadHistoricalData = async (scale: 'daily' | 'weekly' | 'monthly') => {
    setLoading(true);
    setError(null);
    
    try {
      console.log('📊 [HISTORICAL-APM] Loading data:', { scale, viewMode });
      
      const result = await invoke<CommandResult<HistoricalAPMResponse>>('get_historical_apm_data', {
        time_scale: scale,
        view_mode: viewMode,
      });

      if (result.success && result.data) {
        // Format data for chart display
        const formattedData = result.data.data.map(point => ({
          ...point,
          // Format period for better display
          period_display: formatPeriodDisplay(point.period, scale),
        }));
        
        setData(formattedData);
        console.log('✅ [HISTORICAL-APM] Loaded', formattedData.length, 'data points');
      } else {
        const errorMsg = result.error || 'Failed to load historical APM data';
        setError(errorMsg);
        console.error('❌ [HISTORICAL-APM] Error:', errorMsg);
      }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(errorMsg);
      console.error('❌ [HISTORICAL-APM] Exception:', errorMsg);
    } finally {
      setLoading(false);
    }
  };

  // Format period for display on x-axis
  const formatPeriodDisplay = (period: string, scale: string): string => {
    switch (scale) {
      case 'daily':
        // Convert 2025-01-26 to Jan 26
        const date = new Date(period);
        return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
      case 'weekly':
        // Convert 2025-W04 to Week 4
        const weekMatch = period.match(/(\d{4})-W(\d{2})/);
        return weekMatch ? `Week ${parseInt(weekMatch[2])}` : period;
      case 'monthly':
        // Convert 2025-01 to Jan 2025
        const monthDate = new Date(period + '-01');
        return monthDate.toLocaleDateString('en-US', { month: 'short', year: 'numeric' });
      default:
        return period;
    }
  };

  // Get chart data based on view mode
  const getChartLines = () => {
    const lines: React.ReactElement[] = [];
    
    switch (viewMode) {
      case 'combined':
        lines.push(
          <Line
            key="combined"
            type="monotone"
            dataKey="combined_apm"
            stroke="#3b82f6"
            strokeWidth={2}
            dot={{ fill: '#3b82f6', strokeWidth: 2, r: 3 }}
            name="Combined APM"
          />
        );
        break;
      case 'cli':
        lines.push(
          <Line
            key="cli"
            type="monotone"
            dataKey="cli_apm"
            stroke="#10b981"
            strokeWidth={2}
            dot={{ fill: '#10b981', strokeWidth: 2, r: 3 }}
            name="CLI APM"
          />
        );
        break;
      case 'sdk':
        lines.push(
          <Line
            key="sdk"
            type="monotone"
            dataKey="sdk_apm"
            stroke="#f59e0b"
            strokeWidth={2}
            dot={{ fill: '#f59e0b', strokeWidth: 2, r: 3 }}
            name="SDK APM"
          />
        );
        break;
    }

    return lines;
  };

  // Custom tooltip for chart
  const CustomTooltip = ({ active, payload, label }: any) => {
    if (active && payload && payload.length) {
      const data = payload[0].payload;
      return (
        <div className="bg-card border border-border rounded-lg p-3 shadow-lg">
          <p className="font-medium text-sm mb-2">{label}</p>
          {payload.map((entry: any, index: number) => (
            <div key={index} className="flex items-center gap-2 text-xs">
              <div 
                className="w-2 h-2 rounded-full" 
                style={{ backgroundColor: entry.color }}
              />
              <span>{entry.name}: {entry.value.toFixed(2)}</span>
            </div>
          ))}
          <div className="mt-2 pt-2 border-t border-border text-xs text-muted-foreground">
            <div>Sessions: {data.total_sessions}</div>
            <div>Messages: {data.total_messages}</div>
            <div>Tools: {data.total_tools}</div>
          </div>
        </div>
      );
    }
    return null;
  };

  useEffect(() => {
    loadHistoricalData(timeScale);
  }, [timeScale, viewMode]);

  if (loading) {
    return (
      <Card className={`p-6 ${className}`}>
        <div className="flex items-center justify-center h-64">
          <div className="text-center">
            <Loader2 className="h-8 w-8 animate-spin mx-auto mb-4 text-muted-foreground" />
            <p className="text-muted-foreground text-sm">Loading historical data...</p>
          </div>
        </div>
      </Card>
    );
  }

  if (error) {
    return (
      <Card className={`p-6 ${className}`}>
        <div className="flex items-center justify-center h-64">
          <div className="text-center">
            <p className="text-red-400 mb-4 text-sm">{error}</p>
            <Button 
              onClick={() => loadHistoricalData(timeScale)} 
              variant="outline" 
              size="sm"
            >
              Retry
            </Button>
          </div>
        </div>
      </Card>
    );
  }

  return (
    <Card className={`p-6 ${className}`}>
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-2">
          <TrendingUp className="h-5 w-5" />
          <h3 className="font-semibold">Historical APM Trends</h3>
        </div>
        
        {/* Time Scale Selector */}
        <div className="flex items-center gap-1">
          {(['daily', 'weekly', 'monthly'] as const).map((scale) => (
            <Button
              key={scale}
              variant={timeScale === scale ? "default" : "outline"}
              size="sm"
              onClick={() => setTimeScale(scale)}
              className="text-xs h-7 px-2"
            >
              {scale.charAt(0).toUpperCase() + scale.slice(1)}
            </Button>
          ))}
        </div>
      </div>

      {/* Chart */}
      {data.length > 0 ? (
        <div className="h-64">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={data} margin={{ top: 5, right: 30, left: 20, bottom: 5 }}>
              <CartesianGrid strokeDasharray="3 3" className="opacity-30" />
              <XAxis 
                dataKey="period_display" 
                tick={{ fontSize: 12 }}
                className="text-muted-foreground"
              />
              <YAxis 
                tick={{ fontSize: 12 }}
                className="text-muted-foreground"
                label={{ value: 'APM', angle: -90, position: 'insideLeft' }}
              />
              <Tooltip content={<CustomTooltip />} />
              <Legend 
                wrapperStyle={{ fontSize: '12px' }}
                iconType="line"
              />
              {getChartLines()}
            </LineChart>
          </ResponsiveContainer>
        </div>
      ) : (
        <div className="flex items-center justify-center h-64">
          <div className="text-center">
            <Calendar className="h-8 w-8 mx-auto mb-4 text-muted-foreground" />
            <p className="text-muted-foreground text-sm">No historical data available</p>
            <p className="text-muted-foreground text-xs mt-1">
              Try a different time scale or start using Claude Code to generate data
            </p>
          </div>
        </div>
      )}

      {/* Footer */}
      {data.length > 0 && (
        <div className="mt-4 pt-4 border-t border-border">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span>
              {data.length} {timeScale === 'daily' ? 'days' : timeScale === 'weekly' ? 'weeks' : 'months'}
            </span>
            <span>
              Last updated: {new Date().toLocaleDateString()}
            </span>
          </div>
        </div>
      )}
    </Card>
  );
};