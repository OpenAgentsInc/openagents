import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { DeploymentStage } from '../molecules/DeploymentStage.stories'
import { StatusBadge } from '../atoms/StatusBadge.stories'
import { DeploymentUrl } from '../atoms/DeploymentUrl.stories'

// Icon components
const RocketIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M9 11L7 21L12 17L17 21L15 11M12 2C12 2 17 4 17 11L12 13L7 11C7 4 12 2 12 2Z" />
  </svg>
)

const ClockIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="10" />
    <polyline points="12 6 12 12 16 14" />
  </svg>
)

const CheckIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="20 6 9 17 4 12" />
  </svg>
)

const ExternalLinkIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
    <polyline points="15 3 21 3 21 9" />
    <line x1="10" y1="14" x2="21" y2="3" />
  </svg>
)

// Stage interface
interface DeploymentStageData {
  id: string
  title: string
  description?: string
  status: 'pending' | 'running' | 'complete' | 'error' | 'skipped'
  duration?: number
  startTime?: Date
  endTime?: Date
  error?: string
  logs?: string[]
}

// DeploymentProgress component
export interface DeploymentProgressProps {
  projectName?: string
  stages?: DeploymentStageData[]
  overallStatus?: 'pending' | 'running' | 'complete' | 'error' | 'cancelled'
  deploymentUrl?: string
  startTime?: Date
  totalDuration?: number
  showLogs?: boolean
  showTimeline?: boolean
  animated?: boolean
  className?: string
  onStageRetry?: (stageId: string) => void
  onViewLogs?: (stageId: string) => void
  onCancel?: () => void
  onVisitSite?: (url: string) => void
}

export const DeploymentProgress = ({
  projectName = 'My Project',
  stages = [],
  overallStatus = 'pending',
  deploymentUrl,
  startTime,
  totalDuration,
  showLogs = true,
  showTimeline = true,
  animated = true,
  className = '',
  onStageRetry,
  onViewLogs,
  onCancel,
  onVisitSite
}: DeploymentProgressProps) => {
  const [active, setActive] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 200)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const completedStages = stages.filter(stage => stage.status === 'complete').length
  const totalStages = stages.length
  const progressPercent = totalStages > 0 ? (completedStages / totalStages) * 100 : 0

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

  const getOverallStatusConfig = () => {
    switch (overallStatus) {
      case 'running':
        return {
          icon: RocketIcon,
          color: 'text-cyan-400',
          bgColor: 'bg-cyan-500/10',
          borderColor: 'border-cyan-500/30',
          label: 'Deploying'
        }
      case 'complete':
        return {
          icon: CheckIcon,
          color: 'text-green-400',
          bgColor: 'bg-green-500/10',
          borderColor: 'border-green-500/30',
          label: 'Deployed'
        }
      case 'error':
        return {
          icon: ClockIcon,
          color: 'text-red-400',
          bgColor: 'bg-red-500/10',
          borderColor: 'border-red-500/30',
          label: 'Failed'
        }
      case 'cancelled':
        return {
          icon: ClockIcon,
          color: 'text-gray-400',
          bgColor: 'bg-gray-500/10',
          borderColor: 'border-gray-500/30',
          label: 'Cancelled'
        }
      default:
        return {
          icon: ClockIcon,
          color: 'text-gray-400',
          bgColor: 'bg-gray-500/10',
          borderColor: 'border-gray-500/30',
          label: 'Pending'
        }
    }
  }

  const statusConfig = getOverallStatusConfig()
  const StatusIcon = statusConfig.icon

  const progressContent = (
    <div
      className={cx(
        'bg-black border border-cyan-500/30 rounded-lg overflow-hidden',
        'shadow-lg shadow-cyan-500/20',
        className
      )}
    >
      {/* Header */}
      <div className={cx(
        'px-6 py-4 border-b border-cyan-500/20',
        statusConfig.bgColor
      )}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className={cx(
              'flex items-center justify-center w-10 h-10 rounded-lg',
              statusConfig.bgColor,
              statusConfig.borderColor,
              'border'
            )}>
              <StatusIcon className={statusConfig.color} />
            </div>
            
            <div>
              <Text as="h2" className="text-xl font-medium text-white">
                {projectName}
              </Text>
              <div className="flex items-center gap-2 mt-1">
                <StatusBadge
                  status={overallStatus === 'running' ? 'deploying' : overallStatus === 'complete' ? 'deployed' : overallStatus === 'error' ? 'error' : 'idle'}
                  size="small"
                  animated={false}
                />
                <span className="text-sm text-gray-400">
                  {statusConfig.label}
                </span>
              </div>
            </div>
          </div>
          
          <div className="text-right">
            {startTime && (
              <div className="text-sm text-gray-400">
                Started: {formatTime(startTime)}
              </div>
            )}
            {totalDuration && (
              <div className="text-sm text-gray-300">
                Duration: {formatDuration(totalDuration)}
              </div>
            )}
            {overallStatus === 'running' && onCancel && (
              <button
                onClick={onCancel}
                className="mt-2 px-3 py-1 text-xs bg-red-500/20 text-red-300 border border-red-500/50 rounded hover:bg-red-500/30 transition-colors cursor-pointer"
              >
                Cancel
              </button>
            )}
          </div>
        </div>

        {/* Progress Bar */}
        {showTimeline && (
          <div className="mt-4">
            <div className="flex items-center justify-between text-sm text-gray-400 mb-2">
              <span>Progress</span>
              <span>{completedStages}/{totalStages} stages</span>
            </div>
            <div className="w-full bg-gray-700 rounded-full h-2">
              <div
                className={cx(
                  'h-2 rounded-full transition-all duration-500',
                  overallStatus === 'complete' ? 'bg-green-400' :
                  overallStatus === 'error' ? 'bg-red-400' :
                  'bg-cyan-400'
                )}
                style={{ width: `${progressPercent}%` }}
              />
            </div>
          </div>
        )}
      </div>

      {/* Deployment URL */}
      {deploymentUrl && overallStatus === 'complete' && (
        <div className="px-6 py-4 bg-green-500/5 border-b border-green-500/20">
          <div className="flex items-center justify-between">
            <div>
              <Text className="text-sm text-gray-400 mb-1">Deployment URL</Text>
              <DeploymentUrl
                url={deploymentUrl}
                status="active"
                animated={false}
              />
            </div>
            <button
              onClick={() => onVisitSite?.(deploymentUrl)}
              className="flex items-center gap-2 px-4 py-2 bg-green-500/20 text-green-300 border border-green-500/50 rounded hover:bg-green-500/30 transition-colors cursor-pointer"
            >
              <span>Visit Site</span>
              <ExternalLinkIcon className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {/* Stages */}
      <div className="p-6">
        <div className="space-y-4">
          {stages.map((stage, index) => (
            <div key={stage.id} style={{ animationDelay: `${index * 100}ms` }}>
              <DeploymentStage
                {...stage}
                showLogs={showLogs}
                animated={false}
                onRetry={onStageRetry}
                onViewLogs={onViewLogs}
              />
            </div>
          ))}
        </div>

        {stages.length === 0 && (
          <div className="text-center py-8 text-gray-500">
            <ClockIcon className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <Text>Waiting for deployment to start...</Text>
          </div>
        )}
      </div>
    </div>
  )

  if (!animated) {
    return progressContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.7, exit: 0.4 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', 40, 0]]}>
          {progressContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Organisms/DeploymentProgress',
  component: DeploymentProgress,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Complete deployment progress visualization showing multiple stages, overall status, and deployment URL. Used to track the entire deployment pipeline.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    projectName: {
      control: 'text',
      description: 'Name of the project being deployed'
    },
    stages: {
      control: 'object',
      description: 'Array of deployment stages'
    },
    overallStatus: {
      control: 'select',
      options: ['pending', 'running', 'complete', 'error', 'cancelled'],
      description: 'Overall deployment status'
    },
    deploymentUrl: {
      control: 'text',
      description: 'URL of deployed application'
    },
    totalDuration: {
      control: 'number',
      description: 'Total deployment duration in milliseconds'
    },
    showLogs: {
      control: 'boolean',
      description: 'Allow viewing stage logs'
    },
    showTimeline: {
      control: 'boolean',
      description: 'Show progress timeline'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof DeploymentProgress>

export default meta
type Story = StoryObj<typeof meta>

// Sample stages for stories
const sampleStages: DeploymentStageData[] = [
  {
    id: 'checkout',
    title: 'Checkout Code',
    description: 'Fetching latest code from repository',
    status: 'complete',
    duration: 2100,
    startTime: new Date(Date.now() - 120000),
    endTime: new Date(Date.now() - 118000)
  },
  {
    id: 'install',
    title: 'Install Dependencies',
    description: 'Installing npm packages',
    status: 'complete',
    duration: 12400,
    startTime: new Date(Date.now() - 118000),
    endTime: new Date(Date.now() - 106000)
  },
  {
    id: 'build',
    title: 'Build Application',
    description: 'Compiling and bundling assets',
    status: 'complete',
    duration: 8500,
    startTime: new Date(Date.now() - 106000),
    endTime: new Date(Date.now() - 98000)
  },
  {
    id: 'test',
    title: 'Run Tests',
    description: 'Execute test suite',
    status: 'complete',
    duration: 5200,
    startTime: new Date(Date.now() - 98000),
    endTime: new Date(Date.now() - 93000)
  },
  {
    id: 'deploy',
    title: 'Deploy to Production',
    description: 'Upload to Cloudflare Workers',
    status: 'complete',
    duration: 3800,
    startTime: new Date(Date.now() - 93000),
    endTime: new Date(Date.now() - 89000)
  }
]

// Stories
export const Default: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    overallStatus: 'pending'
  }
}

export const InProgress: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    stages: [
      { ...sampleStages[0] },
      { ...sampleStages[1] },
      {
        ...sampleStages[2],
        status: 'running',
        endTime: undefined,
        duration: undefined
      },
      { ...sampleStages[3], status: 'pending' },
      { ...sampleStages[4], status: 'pending' }
    ],
    overallStatus: 'running',
    startTime: new Date(Date.now() - 120000)
  }
}

export const Completed: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    stages: sampleStages,
    overallStatus: 'complete',
    deploymentUrl: 'https://bitcoin-puns.openagents.dev',
    startTime: new Date(Date.now() - 120000),
    totalDuration: 32100
  }
}

export const WithError: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    stages: [
      { ...sampleStages[0] },
      { ...sampleStages[1] },
      {
        ...sampleStages[2],
        status: 'error',
        error: 'TypeScript compilation failed: Cannot find module \'@types/bitcoin\'',
        duration: 3200
      },
      { ...sampleStages[3], status: 'skipped' },
      { ...sampleStages[4], status: 'skipped' }
    ],
    overallStatus: 'error',
    startTime: new Date(Date.now() - 120000),
    totalDuration: 17700
  }
}

export const WithLogs: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    stages: [
      {
        ...sampleStages[0],
        logs: [
          'Initializing git repository',
          'Fetching from origin',
          'Checking out main branch',
          'Repository ready'
        ]
      },
      {
        ...sampleStages[1],
        logs: [
          'Reading package.json',
          'Resolving dependencies',
          'Installing 42 packages',
          'Running post-install scripts',
          'Dependencies installed successfully'
        ]
      },
      { ...sampleStages[2] },
      { ...sampleStages[3] },
      { ...sampleStages[4] }
    ],
    overallStatus: 'complete',
    deploymentUrl: 'https://bitcoin-puns.openagents.dev',
    startTime: new Date(Date.now() - 120000),
    totalDuration: 32100,
    showLogs: true
  }
}

export const LongDeployment: Story = {
  args: {
    projectName: 'E-commerce Platform',
    stages: [
      {
        id: 'checkout',
        title: 'Checkout Code',
        description: 'Fetching source code',
        status: 'complete',
        duration: 3200
      },
      {
        id: 'install',
        title: 'Install Dependencies',
        description: 'Installing packages and dependencies',
        status: 'complete',
        duration: 45600
      },
      {
        id: 'build-frontend',
        title: 'Build Frontend',
        description: 'Compiling React application',
        status: 'complete',
        duration: 28900
      },
      {
        id: 'build-backend',
        title: 'Build Backend',
        description: 'Compiling API services',
        status: 'complete',
        duration: 15400
      },
      {
        id: 'test-unit',
        title: 'Unit Tests',
        description: 'Running unit test suite',
        status: 'complete',
        duration: 12300
      },
      {
        id: 'test-integration',
        title: 'Integration Tests',
        description: 'Running integration tests',
        status: 'complete',
        duration: 18700
      },
      {
        id: 'deploy-staging',
        title: 'Deploy to Staging',
        description: 'Deploy to staging environment',
        status: 'complete',
        duration: 8900
      },
      {
        id: 'smoke-tests',
        title: 'Smoke Tests',
        description: 'Running smoke tests on staging',
        status: 'complete',
        duration: 6500
      },
      {
        id: 'deploy-production',
        title: 'Deploy to Production',
        description: 'Deploy to production environment',
        status: 'complete',
        duration: 12100
      }
    ],
    overallStatus: 'complete',
    deploymentUrl: 'https://ecommerce-platform.openagents.dev',
    startTime: new Date(Date.now() - 300000),
    totalDuration: 151600
  }
}

export const MinimalView: Story = {
  args: {
    projectName: 'Simple App',
    stages: [
      {
        id: 'build',
        title: 'Build',
        status: 'complete',
        duration: 5400
      },
      {
        id: 'deploy',
        title: 'Deploy',
        status: 'complete',
        duration: 2100
      }
    ],
    overallStatus: 'complete',
    deploymentUrl: 'https://simple-app.openagents.dev',
    showTimeline: false,
    showLogs: false
  }
}

export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    const [status, setStatus] = useState<'pending' | 'running' | 'complete' | 'error'>('pending')
    const [currentStage, setCurrentStage] = useState(0)
    const [stages, setStages] = useState<DeploymentStageData[]>([
      {
        id: 'checkout',
        title: 'Checkout Code',
        description: 'Fetching repository',
        status: 'pending'
      },
      {
        id: 'install',
        title: 'Install Dependencies',
        description: 'Installing packages',
        status: 'pending'
      },
      {
        id: 'build',
        title: 'Build Application',
        description: 'Compiling assets',
        status: 'pending'
      },
      {
        id: 'deploy',
        title: 'Deploy to Production',
        description: 'Uploading to Cloudflare',
        status: 'pending'
      }
    ])
    const [deploymentUrl, setDeploymentUrl] = useState<string>()

    const startDeployment = () => {
      setStatus('running')
      setCurrentStage(0)
      
      // Simulate stage progression
      const stageTimings = [2000, 3000, 2500, 1500]
      let delay = 0
      
      stages.forEach((stage, index) => {
        delay += stageTimings[index]
        
        setTimeout(() => {
          setStages(prev => prev.map((s, i) => ({
            ...s,
            status: i === index ? 'running' : i < index ? 'complete' : 'pending'
          })))
          setCurrentStage(index)
        }, delay - stageTimings[index])
        
        setTimeout(() => {
          setStages(prev => prev.map((s, i) => ({
            ...s,
            status: i <= index ? 'complete' : 'pending'
          })))
          
          if (index === stages.length - 1) {
            setStatus('complete')
            setDeploymentUrl('https://bitcoin-puns.openagents.dev')
          }
        }, delay)
      })
    }

    const resetDeployment = () => {
      setStatus('pending')
      setCurrentStage(0)
      setDeploymentUrl(undefined)
      setStages(prev => prev.map(s => ({ ...s, status: 'pending' })))
    }

    return (
      <div className="space-y-4">
        <DeploymentProgress
          projectName="Interactive Demo"
          stages={stages}
          overallStatus={status}
          deploymentUrl={deploymentUrl}
          startTime={status !== 'pending' ? new Date(Date.now() - 30000) : undefined}
        />
        
        <div className="flex gap-2 justify-center">
          <button
            onClick={startDeployment}
            disabled={status === 'running'}
            className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {status === 'running' ? 'Deploying...' : 'Start Deployment'}
          </button>
          
          <button
            onClick={resetDeployment}
            className="px-4 py-2 bg-gray-500/20 text-gray-300 border border-gray-500/50 rounded hover:bg-gray-500/30 transition-colors cursor-pointer"
          >
            Reset
          </button>
        </div>
      </div>
    )
  }
}

export const Playground: Story = {
  args: {
    projectName: 'My Project',
    stages: sampleStages,
    overallStatus: 'complete',
    deploymentUrl: 'https://my-project.openagents.dev',
    showLogs: true,
    showTimeline: true,
    animated: true
  }
}