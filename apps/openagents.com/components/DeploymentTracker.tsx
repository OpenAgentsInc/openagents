'use client'

import React, { useState, useEffect } from 'react'
import { Text, cx } from '@arwes/react'
import { 
  Rocket, 
  CheckCircle, 
  AlertCircle, 
  Loader2, 
  ExternalLink,
  Clock,
  Terminal,
  ChevronDown,
  ChevronRight,
  Activity
} from 'lucide-react'
import { useDeploymentWebSocket, MockDeploymentWebSocket, type DeploymentStatus } from '@/lib/websocket'
import { useToast } from '@/components/Toast'

interface DeploymentTrackerProps {
  deploymentId: string
  projectName: string
  onComplete?: (deploymentUrl: string) => void
  className?: string
}

export function DeploymentTracker({ 
  deploymentId, 
  projectName, 
  onComplete, 
  className = '' 
}: DeploymentTrackerProps) {
  const toast = useToast()
  const { connected, deploymentStatus, subscribeToDeployment, unsubscribeFromDeployment } = useDeploymentWebSocket()
  const [isExpanded, setIsExpanded] = useState(true)
  const [showLogs, setShowLogs] = useState(false)
  const [startTime, setStartTime] = useState<Date | null>(null)
  const [isUsingMock, setIsUsingMock] = useState(false)

  useEffect(() => {
    setStartTime(new Date())
    
    if (connected) {
      // Real WebSocket connection
      subscribeToDeployment(deploymentId)
    } else {
      // Fallback to mock for development
      setIsUsingMock(true)
      const mockWs = new MockDeploymentWebSocket(deploymentId, (status) => {
        // This would normally come through the WebSocket hook
        console.log('Mock deployment update:', status)
      })
      mockWs.start(projectName)
    }

    return () => {
      if (connected) {
        unsubscribeFromDeployment(deploymentId)
      }
    }
  }, [deploymentId, connected, subscribeToDeployment, unsubscribeFromDeployment])

  // Handle deployment completion
  useEffect(() => {
    if (deploymentStatus?.status === 'success' && deploymentStatus.deploymentUrl) {
      toast.success('Deployment Complete!', `${projectName} is now live!`, {
        action: {
          label: 'View Live',
          onClick: () => window.open(deploymentStatus.deploymentUrl, '_blank')
        }
      })
      onComplete?.(deploymentStatus.deploymentUrl)
    } else if (deploymentStatus?.status === 'error') {
      toast.error('Deployment Failed', deploymentStatus.message || 'Something went wrong during deployment')
    }
  }, [deploymentStatus, projectName, onComplete, toast])

  const getStatusIcon = () => {
    if (!deploymentStatus) return <Loader2 className="w-5 h-5 animate-spin text-cyan-400" />
    
    switch (deploymentStatus.status) {
      case 'success':
        return <CheckCircle className="w-5 h-5 text-green-400" />
      case 'error':
        return <AlertCircle className="w-5 h-5 text-red-400" />
      default:
        return <Loader2 className="w-5 h-5 animate-spin text-cyan-400" />
    }
  }

  const getStatusColor = () => {
    if (!deploymentStatus) return 'text-cyan-300'
    
    switch (deploymentStatus.status) {
      case 'success':
        return 'text-green-300'
      case 'error':
        return 'text-red-300'
      default:
        return 'text-cyan-300'
    }
  }

  const getElapsedTime = () => {
    if (!startTime) return '0s'
    
    const elapsed = Math.floor((Date.now() - startTime.getTime()) / 1000)
    const minutes = Math.floor(elapsed / 60)
    const seconds = elapsed % 60
    
    return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`
  }

  const formatProgress = (progress: number) => {
    return Math.round(progress)
  }

  return (
    <div className={cx(
      'bg-offblack border border-cyan-900/30 rounded-lg overflow-hidden',
      className
    )}>
      {/* Header */}
      <div 
        className="flex items-center justify-between p-4 cursor-pointer hover:bg-cyan-500/5 transition-colors"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <div className="flex items-center gap-3">
          {getStatusIcon()}
          <div>
            <Text className="font-medium text-cyan-300 font-sans">
              Deploying {projectName}
            </Text>
            <div className="flex items-center gap-2 text-sm">
              <Text className={cx('font-mono', getStatusColor())}>
                {deploymentStatus?.stage || 'Initializing'}
              </Text>
              <span className="text-gray-500">•</span>
              <Text className="text-gray-400 font-mono">
                {getElapsedTime()}
              </Text>
              {!connected && isUsingMock && (
                <>
                  <span className="text-gray-500">•</span>
                  <Text className="text-orange-400 text-xs font-mono">
                    DEMO MODE
                  </Text>
                </>
              )}
            </div>
          </div>
        </div>
        
        <div className="flex items-center gap-2">
          {deploymentStatus && (
            <Text className="text-sm text-gray-400 font-mono">
              {formatProgress(deploymentStatus.progress)}%
            </Text>
          )}
          {isExpanded ? (
            <ChevronDown className="w-4 h-4 text-gray-400" />
          ) : (
            <ChevronRight className="w-4 h-4 text-gray-400" />
          )}
        </div>
      </div>

      {/* Expanded Content */}
      {isExpanded && (
        <div className="border-t border-cyan-900/30">
          {/* Progress Bar */}
          <div className="p-4">
            <div className="bg-black/50 rounded-full h-2 overflow-hidden">
              <div 
                className="h-full bg-gradient-to-r from-cyan-500 to-blue-500 transition-all duration-500 ease-out"
                style={{ width: `${deploymentStatus?.progress || 0}%` }}
              />
            </div>
          </div>

          {/* Status Message */}
          {deploymentStatus?.message && (
            <div className="px-4 pb-4">
              <Text className="text-sm text-gray-300 font-sans">
                {deploymentStatus.message}
              </Text>
            </div>
          )}

          {/* Action Buttons */}
          <div className="px-4 pb-4 flex items-center gap-2">
            {deploymentStatus?.deploymentUrl && (
              <button
                onClick={() => window.open(deploymentStatus.deploymentUrl, '_blank')}
                className={cx(
                  'flex items-center gap-2 px-4 py-2',
                  'bg-green-500/20 hover:bg-green-500/30 border border-green-500/50',
                  'text-green-300 hover:text-green-200 rounded transition-all',
                  'font-sans text-sm'
                )}
              >
                <ExternalLink className="w-4 h-4" />
                View Live Site
              </button>
            )}
            
            <button
              onClick={() => setShowLogs(!showLogs)}
              className={cx(
                'flex items-center gap-2 px-4 py-2',
                'border border-gray-500/30 hover:border-gray-400/50',
                'text-gray-400 hover:text-gray-300 rounded transition-all',
                'font-sans text-sm'
              )}
            >
              <Terminal className="w-4 h-4" />
              {showLogs ? 'Hide' : 'Show'} Logs
            </button>

            <div className="ml-auto flex items-center gap-2">
              <Activity className={cx(
                'w-4 h-4',
                connected ? 'text-green-400' : 'text-gray-500'
              )} />
              <Text className="text-xs text-gray-500 font-mono">
                {connected ? 'Connected' : 'Offline'}
              </Text>
            </div>
          </div>

          {/* Logs Section */}
          {showLogs && deploymentStatus?.logs && (
            <div className="border-t border-cyan-900/30 bg-black/30">
              <div className="p-4">
                <div className="bg-black rounded border border-gray-700/30 p-3 max-h-40 overflow-y-auto">
                  <div className="space-y-1">
                    {deploymentStatus.logs.map((log, index) => (
                      <Text key={index} className="text-xs text-gray-300 font-mono">
                        {log}
                      </Text>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Debug Info (Development Only) */}
          {process.env.NODE_ENV === 'development' && (
            <div className="border-t border-cyan-900/30 bg-gray-900/20 p-3">
              <Text className="text-xs text-gray-500 font-mono">
                Debug: {deploymentId} | Status: {deploymentStatus?.status || 'pending'} | 
                WS: {connected ? 'connected' : 'disconnected'}
              </Text>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

// Simplified deployment status badge
export function DeploymentStatusBadge({ 
  status, 
  className = '' 
}: { 
  status: DeploymentStatus['status']
  className?: string 
}) {
  const getStatusConfig = () => {
    switch (status) {
      case 'success':
        return {
          icon: CheckCircle,
          color: 'text-green-400',
          bg: 'bg-green-400/10',
          border: 'border-green-400/20',
          label: 'Deployed'
        }
      case 'error':
        return {
          icon: AlertCircle,
          color: 'text-red-400',
          bg: 'bg-red-400/10',
          border: 'border-red-400/20',
          label: 'Failed'
        }
      case 'building':
      case 'deploying':
        return {
          icon: Loader2,
          color: 'text-cyan-400',
          bg: 'bg-cyan-400/10',
          border: 'border-cyan-400/20',
          label: 'Deploying'
        }
      default:
        return {
          icon: Clock,
          color: 'text-gray-400',
          bg: 'bg-gray-400/10',
          border: 'border-gray-400/20',
          label: 'Pending'
        }
    }
  }

  const config = getStatusConfig()
  const Icon = config.icon

  return (
    <div className={cx(
      'inline-flex items-center gap-2 px-3 py-1.5 rounded-full border',
      config.bg,
      config.border,
      className
    )}>
      <Icon className={cx('w-4 h-4', config.color, status === 'building' || status === 'deploying' ? 'animate-spin' : '')} />
      <Text className={cx('text-sm font-medium font-sans', config.color)}>
        {config.label}
      </Text>
    </div>
  )
}