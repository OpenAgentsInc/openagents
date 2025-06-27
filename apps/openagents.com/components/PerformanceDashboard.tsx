'use client'

import React, { useState, useEffect } from 'react'
import { Text, cx } from '@arwes/react'
import { 
  Activity, 
  Zap, 
  Monitor, 
  TrendingUp, 
  AlertTriangle,
  CheckCircle,
  Info,
  RefreshCw,
  BarChart3,
  Clock
} from 'lucide-react'
import { 
  performanceMonitor, 
  getMemoryUsage, 
  getNetworkInfo, 
  getPerformanceRecommendations,
  analyzeBundleSize
} from '@/lib/performance'

interface PerformanceMetrics {
  webVitals: Record<string, { avg: number, min: number, max: number, count: number }>
  memory: Record<string, number> | null
  network: Record<string, any> | null
  recommendations: string[]
}

export function PerformanceDashboard({ className = '' }: { className?: string }) {
  const [metrics, setMetrics] = useState<PerformanceMetrics>({
    webVitals: {},
    memory: null,
    network: null,
    recommendations: []
  })
  const [isCollapsed, setIsCollapsed] = useState(true)
  const [lastUpdated, setLastUpdated] = useState<Date>(new Date())

  const updateMetrics = () => {
    if (!performanceMonitor) return

    const webVitals = performanceMonitor.getMetrics()
    const memory = getMemoryUsage()
    const network = getNetworkInfo()
    const recommendations = getPerformanceRecommendations(webVitals)

    setMetrics({
      webVitals,
      memory,
      network,
      recommendations
    })
    setLastUpdated(new Date())
  }

  useEffect(() => {
    // Initial metrics load
    updateMetrics()

    // Auto-refresh every 30 seconds
    const interval = setInterval(updateMetrics, 30000)
    return () => clearInterval(interval)
  }, [])

  useEffect(() => {
    // Analyze bundle size on mount (development only)
    if (process.env.NODE_ENV === 'development') {
      analyzeBundleSize()
    }
  }, [])

  const getScoreColor = (metric: string, value: number) => {
    switch (metric) {
      case 'LCP':
        if (value <= 2500) return 'text-green-400'
        if (value <= 4000) return 'text-yellow-400'
        return 'text-red-400'
      case 'FID':
        if (value <= 100) return 'text-green-400'
        if (value <= 300) return 'text-yellow-400'
        return 'text-red-400'
      case 'CLS':
        if (value <= 0.1) return 'text-green-400'
        if (value <= 0.25) return 'text-yellow-400'
        return 'text-red-400'
      default:
        return 'text-cyan-400'
    }
  }

  const getScoreIcon = (metric: string, value: number) => {
    const thresholds = {
      LCP: [2500, 4000],
      FID: [100, 300],
      CLS: [0.1, 0.25]
    }

    const threshold = thresholds[metric as keyof typeof thresholds]
    if (!threshold) return <Info className="w-4 h-4" />

    if (value <= threshold[0]) return <CheckCircle className="w-4 h-4 text-green-400" />
    if (value <= threshold[1]) return <AlertTriangle className="w-4 h-4 text-yellow-400" />
    return <AlertTriangle className="w-4 h-4 text-red-400" />
  }

  const formatMetricValue = (metric: string, value: number) => {
    switch (metric) {
      case 'LCP':
      case 'FID':
        return `${value.toFixed(0)}ms`
      case 'CLS':
        return value.toFixed(3)
      default:
        return value.toFixed(2)
    }
  }

  // Only show in development or when explicitly enabled
  if (process.env.NODE_ENV === 'production') {
    return null
  }

  return (
    <div className={cx(
      'fixed bottom-4 right-4 z-50 max-w-md',
      'bg-black/90 border border-cyan-500/30 rounded-lg backdrop-blur-sm',
      'transition-all duration-300',
      className
    )}>
      {/* Header */}
      <div 
        className="flex items-center justify-between p-3 cursor-pointer"
        onClick={() => setIsCollapsed(!isCollapsed)}
      >
        <div className="flex items-center gap-2">
          <Activity className="w-4 h-4 text-cyan-400" />
          <Text className="text-sm font-medium text-cyan-300 font-sans">
            Performance Monitor
          </Text>
        </div>
        
        <div className="flex items-center gap-2">
          <button
            onClick={(e) => {
              e.stopPropagation()
              updateMetrics()
            }}
            className="p-1 hover:bg-cyan-500/20 rounded transition-colors"
            title="Refresh metrics"
          >
            <RefreshCw className="w-3 h-3 text-cyan-400" />
          </button>
          
          <div className={cx(
            'w-3 h-3 transform transition-transform',
            isCollapsed ? 'rotate-0' : 'rotate-180'
          )}>
            ▼
          </div>
        </div>
      </div>

      {/* Content */}
      {!isCollapsed && (
        <div className="px-3 pb-3 space-y-4">
          {/* Web Vitals */}
          <div>
            <Text className="text-xs text-cyan-400 mb-2 font-sans uppercase tracking-wide">
              Core Web Vitals
            </Text>
            <div className="grid grid-cols-3 gap-2">
              {Object.entries(metrics.webVitals).map(([metric, data]) => (
                <div key={metric} className="text-center p-2 bg-black/50 rounded border border-gray-700/30">
                  <div className="flex items-center justify-center gap-1 mb-1">
                    {getScoreIcon(metric, data.avg)}
                    <Text className="text-xs text-gray-400 font-mono">{metric}</Text>
                  </div>
                  <Text className={cx('text-sm font-bold font-mono', getScoreColor(metric, data.avg))}>
                    {formatMetricValue(metric, data.avg)}
                  </Text>
                  <Text className="text-xs text-gray-500 font-mono">
                    {data.count} samples
                  </Text>
                </div>
              ))}
            </div>
          </div>

          {/* Memory Usage */}
          {metrics.memory && (
            <div>
              <Text className="text-xs text-cyan-400 mb-2 font-sans uppercase tracking-wide">
                Memory Usage
              </Text>
              <div className="grid grid-cols-2 gap-2">
                <div className="p-2 bg-black/50 rounded border border-gray-700/30">
                  <Text className="text-xs text-gray-400 font-sans">Used</Text>
                  <Text className="text-sm font-bold text-cyan-300 font-mono">
                    {metrics.memory.usedJSHeapSize}MB
                  </Text>
                </div>
                <div className="p-2 bg-black/50 rounded border border-gray-700/30">
                  <Text className="text-xs text-gray-400 font-sans">Total</Text>
                  <Text className="text-sm font-bold text-cyan-300 font-mono">
                    {metrics.memory.totalJSHeapSize}MB
                  </Text>
                </div>
              </div>
            </div>
          )}

          {/* Network Info */}
          {metrics.network && (
            <div>
              <Text className="text-xs text-cyan-400 mb-2 font-sans uppercase tracking-wide">
                Network
              </Text>
              <div className="p-2 bg-black/50 rounded border border-gray-700/30">
                <div className="flex justify-between items-center">
                  <Text className="text-xs text-gray-400 font-sans">Connection</Text>
                  <Text className="text-sm text-cyan-300 font-mono">
                    {metrics.network.effectiveType || 'Unknown'}
                  </Text>
                </div>
                {metrics.network.downlink && (
                  <div className="flex justify-between items-center">
                    <Text className="text-xs text-gray-400 font-sans">Speed</Text>
                    <Text className="text-sm text-cyan-300 font-mono">
                      {metrics.network.downlink} Mbps
                    </Text>
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Recommendations */}
          {metrics.recommendations.length > 0 && (
            <div>
              <Text className="text-xs text-cyan-400 mb-2 font-sans uppercase tracking-wide">
                Recommendations
              </Text>
              <div className="space-y-1">
                {metrics.recommendations.slice(0, 3).map((rec, index) => (
                  <div key={index} className="p-2 bg-yellow-500/10 border border-yellow-500/20 rounded">
                    <Text className="text-xs text-yellow-300 font-sans">{rec}</Text>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Last Updated */}
          <div className="pt-2 border-t border-gray-700/30">
            <div className="flex items-center justify-between">
              <Text className="text-xs text-gray-500 font-sans">
                Last updated: {lastUpdated.toLocaleTimeString()}
              </Text>
              <div className="flex items-center gap-1">
                <Clock className="w-3 h-3 text-gray-500" />
                <Text className="text-xs text-gray-500 font-sans">
                  {process.env.NODE_ENV}
                </Text>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}