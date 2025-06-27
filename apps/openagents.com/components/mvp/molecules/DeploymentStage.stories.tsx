import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { StatusBadge } from '../atoms/StatusBadge.stories'
import { LoadingSpinner } from '../atoms/LoadingSpinner.stories'

// Icon components
const CheckIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="20 6 9 17 4 12" />
  </svg>
)

const XIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <line x1="18" y1="6" x2="6" y2="18" />
    <line x1="6" y1="6" x2="18" y2="18" />
  </svg>
)

const ClockIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="10" />
    <polyline points="12 6 12 12 16 14" />
  </svg>
)

const PlayIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polygon points="5 3 19 12 5 21 5 3" />
  </svg>
)

const BuildIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
  </svg>
)

const UploadIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
    <polyline points="7 10 12 15 17 10" />
    <line x1="12" y1="15" x2="12" y2="3" />
  </svg>
)

const RocketIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M9 11L7 21L12 17L17 21L15 11M12 2C12 2 17 4 17 11L12 13L7 11C7 4 12 2 12 2Z" />
  </svg>
)

// DeploymentStage component
export interface DeploymentStageProps {
  id: string
  title: string
  description?: string
  status: 'pending' | 'running' | 'complete' | 'error' | 'skipped'
  duration?: number
  startTime?: Date
  endTime?: Date
  error?: string
  logs?: string[]
  showLogs?: boolean
  showDuration?: boolean
  animated?: boolean
  className?: string
  onRetry?: (id: string) => void
  onViewLogs?: (id: string) => void
}

export const DeploymentStage = ({
  id,
  title,
  description,
  status,
  duration,
  startTime,
  endTime,
  error,
  logs = [],
  showLogs = false,
  showDuration = true,
  animated = true,
  className = '',
  onRetry,
  onViewLogs
}: DeploymentStageProps) => {
  const [active, setActive] = useState(false)
  const [expanded, setExpanded] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 150)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const stageConfig = {
    pending: {
      icon: ClockIcon,
      bgColor: 'bg-gray-500/10',
      borderColor: 'border-gray-500/30',
      textColor: 'text-gray-300',
      iconBg: 'bg-gray-500/20',
      iconColor: 'text-gray-400'
    },
    running: {
      icon: PlayIcon,
      bgColor: 'bg-cyan-500/10',
      borderColor: 'border-cyan-500/30',
      textColor: 'text-cyan-300',
      iconBg: 'bg-cyan-500/20',
      iconColor: 'text-cyan-400'
    },
    complete: {
      icon: CheckIcon,
      bgColor: 'bg-green-500/10',
      borderColor: 'border-green-500/30',
      textColor: 'text-green-300',
      iconBg: 'bg-green-500/20',
      iconColor: 'text-green-400'
    },
    error: {
      icon: XIcon,
      bgColor: 'bg-red-500/10',
      borderColor: 'border-red-500/30',
      textColor: 'text-red-300',
      iconBg: 'bg-red-500/20',
      iconColor: 'text-red-400'
    },
    skipped: {
      icon: ClockIcon,
      bgColor: 'bg-gray-500/5',
      borderColor: 'border-gray-500/20',
      textColor: 'text-gray-500',
      iconBg: 'bg-gray-500/10',
      iconColor: 'text-gray-600'
    }
  }

  const config = stageConfig[status] || stageConfig.pending
  const Icon = config?.icon || ClockIcon

  const formatDuration = (ms: number): string => {
    if (ms < 1000) return `${ms}ms`
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`
    return `${(ms / 60000).toFixed(1)}m`
  }

  const formatTime = (date: Date): string => {
    return date.toLocaleTimeString('en-US', {
      hour12: false,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    })
  }

  const calculateDuration = (): number | undefined => {
    if (duration) return duration
    if (startTime && endTime) return endTime.getTime() - startTime.getTime()
    if (startTime && status === 'running') return Date.now() - startTime.getTime()
    return undefined
  }

  const stageContent = (
    <div
      className={cx(
        'rounded-lg border p-4 transition-all duration-300',
        config.bgColor,
        config.borderColor,
        status === 'running' && 'shadow-lg shadow-cyan-500/20',
        className
      )}
    >
      <div className="flex items-start gap-3">
        {/* Status Icon */}
        <div
          className={cx(
            'flex items-center justify-center w-10 h-10 rounded-lg flex-shrink-0',
            config.iconBg
          )}
        >
          {status === 'running' ? (
            <LoadingSpinner size="small" color="cyan" variant="circle" />
          ) : (
            <Icon className={config.iconColor} />
          )}
        </div>

        {/* Content */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <Text
              as="h3"
              manager={animated ? 'decipher' : undefined}
              className={cx('font-medium', config.textColor)}
            >
              {title}
            </Text>
            
            <StatusBadge
              status={status === 'running' ? 'generating' : status === 'complete' ? 'deployed' : status === 'error' ? 'error' : 'idle'}
              size="small"
              animated={false}
            />

            {showDuration && calculateDuration() && (
              <span className="text-xs text-gray-500 ml-auto">
                {formatDuration(calculateDuration()!)}
              </span>
            )}
          </div>

          {description && (
            <p className="text-sm text-gray-400 mt-1">{description}</p>
          )}

          {/* Timestamps */}
          {(startTime || endTime) && (
            <div className="flex items-center gap-4 mt-2 text-xs text-gray-500">
              {startTime && (
                <span>Started: {formatTime(startTime)}</span>
              )}
              {endTime && (
                <span>Completed: {formatTime(endTime)}</span>
              )}
            </div>
          )}

          {/* Error Message */}
          {status === 'error' && error && (
            <div className="mt-3 p-3 bg-red-500/10 border border-red-500/30 rounded text-red-400 text-sm">
              <strong>Error:</strong> {error}
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center gap-2 mt-3">
            {status === 'error' && onRetry && (
              <button
                onClick={() => onRetry(id)}
                className="px-3 py-1 text-xs bg-red-500/20 text-red-300 border border-red-500/50 rounded hover:bg-red-500/30 transition-colors cursor-pointer"
              >
                Retry
              </button>
            )}
            
            {logs.length > 0 && onViewLogs && (
              <button
                onClick={() => {
                  setExpanded(!expanded)
                  onViewLogs(id)
                }}
                className="px-3 py-1 text-xs bg-gray-500/20 text-gray-300 border border-gray-500/50 rounded hover:bg-gray-500/30 transition-colors cursor-pointer"
              >
                {expanded ? 'Hide' : 'View'} Logs ({logs.length})
              </button>
            )}
          </div>

          {/* Logs */}
          {showLogs && expanded && logs.length > 0 && (
            <div className="mt-3 p-3 bg-black/50 border border-gray-700 rounded font-mono text-xs">
              <div className="max-h-32 overflow-y-auto space-y-1">
                {logs.map((log, index) => (
                  <div key={index} className="text-gray-300">
                    <span className="text-gray-600">[{index + 1}]</span> {log}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )

  if (!animated) {
    return stageContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['x', -20, 0]]}>
          {stageContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/DeploymentStage',
  component: DeploymentStage,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Individual deployment stage component showing status, duration, and logs for each step in the deployment process.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    id: {
      control: 'text',
      description: 'Unique identifier for the stage'
    },
    title: {
      control: 'text',
      description: 'Stage title'
    },
    description: {
      control: 'text',
      description: 'Optional stage description'
    },
    status: {
      control: 'select',
      options: ['pending', 'running', 'complete', 'error', 'skipped'],
      description: 'Current stage status'
    },
    duration: {
      control: 'number',
      description: 'Stage duration in milliseconds'
    },
    error: {
      control: 'text',
      description: 'Error message (when status is error)'
    },
    showLogs: {
      control: 'boolean',
      description: 'Enable log viewing'
    },
    showDuration: {
      control: 'boolean',
      description: 'Show stage duration'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof DeploymentStage>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    id: 'build',
    title: 'Building Application',
    description: 'Compiling TypeScript and bundling assets',
    status: 'running'
  }
}

export const AllStatuses: Story = {
  args: {
    id: 'example',
    title: 'Example Stage',
    status: 'pending'
  },
  render: () => (
    <div className="space-y-4">
      <DeploymentStage
        id="pending"
        title="Initialize Build"
        description="Preparing build environment"
        status="pending"
      />
      <DeploymentStage
        id="running"
        title="Building Application"
        description="Compiling and bundling code"
        status="running"
        startTime={new Date(Date.now() - 30000)}
      />
      <DeploymentStage
        id="complete"
        title="Deploy to Cloudflare"
        description="Uploading to edge network"
        status="complete"
        duration={5420}
        startTime={new Date(Date.now() - 60000)}
        endTime={new Date(Date.now() - 55000)}
      />
      <DeploymentStage
        id="error"
        title="Run Tests"
        description="Execute test suite"
        status="error"
        error="TypeError: Cannot read property 'map' of undefined at line 42"
        duration={1250}
      />
      <DeploymentStage
        id="skipped"
        title="Deploy to Production"
        description="Deploy to production environment"
        status="skipped"
      />
    </div>
  )
}

export const WithLogs: Story = {
  args: {
    id: 'build-logs',
    title: 'Building Application',
    description: 'Compiling TypeScript and bundling assets',
    status: 'complete',
    duration: 15420,
    showLogs: true,
    logs: [
      'Installing dependencies...',
      'Found 42 packages to install',
      'Compiling TypeScript files...',
      'src/index.ts compiled successfully',
      'src/components/App.tsx compiled successfully',
      'Running Webpack bundler...',
      'Asset optimization complete',
      'Build completed successfully in 15.4s'
    ]
  }
}

export const ErrorWithRetry: Story = {
  args: {
    id: 'example',
    title: 'Example Stage',
    status: 'pending'
  },
  render: () => {
    const [status, setStatus] = useState<'error' | 'running' | 'complete'>('error')
    const [retryCount, setRetryCount] = useState(0)
    
    const handleRetry = () => {
      setRetryCount(count => count + 1)
      setStatus('running')
      
      setTimeout(() => {
        setStatus(retryCount < 2 ? 'error' : 'complete')
      }, 2000)
    }
    
    return (
      <div className="space-y-4">
        <DeploymentStage
          id="deploy"
          title="Deploy to Cloudflare"
          description="Uploading application files"
          status={status}
          error={status === 'error' ? `Network timeout (attempt ${retryCount + 1})` : undefined}
          onRetry={handleRetry}
        />
        
        {retryCount > 0 && (
          <p className="text-sm text-gray-400">
            Retry attempts: {retryCount}
          </p>
        )}
      </div>
    )
  }
}

export const DeploymentPipeline: Story = {
  args: {
    id: 'example',
    title: 'Example Stage',
    status: 'pending'
  },
  render: () => {
    const stages = [
      {
        id: 'checkout',
        title: 'Checkout Code',
        description: 'Fetching latest code from repository',
        status: 'complete' as const,
        duration: 2100
      },
      {
        id: 'install',
        title: 'Install Dependencies',
        description: 'Installing npm packages',
        status: 'complete' as const,
        duration: 12400
      },
      {
        id: 'build',
        title: 'Build Application',
        description: 'Compiling and bundling assets',
        status: 'running' as const,
        startTime: new Date(Date.now() - 8000)
      },
      {
        id: 'test',
        title: 'Run Tests',
        description: 'Execute test suite',
        status: 'pending' as const
      },
      {
        id: 'deploy',
        title: 'Deploy to Production',
        description: 'Upload to Cloudflare Workers',
        status: 'pending' as const
      }
    ]
    
    return (
      <div className="space-y-3">
        <h3 className="text-cyan-300 text-lg mb-4">Bitcoin Puns Deployment</h3>
        {stages.map((stage, index) => (
          <div key={stage.id} style={{ animationDelay: `${index * 100}ms` }}>
            <DeploymentStage {...stage} />
          </div>
        ))}
      </div>
    )
  }
}

export const WithTimestamps: Story = {
  args: {
    id: 'deploy-timestamps',
    title: 'Deploy to Production',
    description: 'Uploading to Cloudflare Workers',
    status: 'complete',
    startTime: new Date(Date.now() - 120000),
    endTime: new Date(Date.now() - 110000),
    duration: 10000
  }
}

export const InteractiveLogs: Story = {
  args: {
    id: 'example',
    title: 'Example Stage',
    status: 'pending'
  },
  render: () => {
    const [expanded, setExpanded] = useState(false)
    
    return (
      <DeploymentStage
        id="build-interactive"
        title="Build Application"
        description="Compiling and optimizing"
        status="complete"
        duration={8500}
        showLogs={true}
        logs={[
          'Starting build process...',
          'Loading configuration from package.json',
          'Resolving dependencies...',
          'Compiling src/index.ts',
          'Compiling src/components/App.tsx',
          'Compiling src/utils/bitcoin-puns.ts',
          'Running Webpack optimization',
          'Minifying JavaScript bundle',
          'Optimizing CSS assets',
          'Generating source maps',
          'Build completed successfully'
        ]}
        onViewLogs={(id) => {
          console.log('Viewing logs for:', id)
          setExpanded(!expanded)
        }}
      />
    )
  }
}

export const VariousDurations: Story = {
  args: {
    id: 'example',
    title: 'Example Stage',
    status: 'pending'
  },
  render: () => (
    <div className="space-y-4">
      <DeploymentStage
        id="fast"
        title="Quick Task"
        status="complete"
        duration={750}
      />
      <DeploymentStage
        id="medium"
        title="Medium Task"
        status="complete"
        duration={5420}
      />
      <DeploymentStage
        id="slow"
        title="Long Task"
        status="complete"
        duration={87500}
      />
      <DeploymentStage
        id="very-slow"
        title="Very Long Task"
        status="complete"
        duration={245000}
      />
    </div>
  )
}

export const Playground: Story = {
  args: {
    id: 'playground',
    title: 'Playground Stage',
    description: 'Test different configurations',
    status: 'running',
    duration: undefined,
    showLogs: false,
    showDuration: true,
    animated: true
  }
}