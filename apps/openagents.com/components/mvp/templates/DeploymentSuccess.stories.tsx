import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { DeploymentUrl } from '../atoms/DeploymentUrl.stories'
import { CopyButton } from '../atoms/CopyButton.stories'
import { StatusBadge } from '../atoms/StatusBadge.stories'

// Icon components
const CheckIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="20 6 9 17 4 12" />
  </svg>
)

const RocketIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M9 11L7 21L12 17L17 21L15 11M12 2C12 2 17 4 17 11L12 13L7 11C7 4 12 2 12 2Z" />
  </svg>
)

const ExternalLinkIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
    <polyline points="15 3 21 3 21 9" />
    <line x1="10" y1="14" x2="21" y2="3" />
  </svg>
)

const ShareIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8" />
    <polyline points="16 6 12 2 8 6" />
    <line x1="12" y1="2" x2="12" y2="15" />
  </svg>
)

const CodeIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="16 18 22 12 16 6" />
    <polyline points="8 6 2 12 8 18" />
  </svg>
)

const DownloadIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
    <polyline points="7 10 12 15 17 10" />
    <line x1="12" y1="15" x2="12" y2="3" />
  </svg>
)

const RefreshIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="23 4 23 10 17 10" />
    <polyline points="1 20 1 14 7 14" />
    <path d="M20.49 9A9 9 0 0 0 5.64 5.64L1 10m22 4l-4.64 4.36A9 9 0 0 1 3.51 15" />
  </svg>
)

const StarIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
  </svg>
)

// Deployment stats interface
interface DeploymentStats {
  filesGenerated: number
  linesOfCode: number
  deploymentTime: number
  regions: string[]
  buildSize: string
}

// DeploymentSuccess component
export interface DeploymentSuccessProps {
  projectName?: string
  deploymentUrl?: string
  stats?: DeploymentStats
  showCelebration?: boolean
  showActions?: boolean
  showStats?: boolean
  showNextSteps?: boolean
  animated?: boolean
  className?: string
  onVisitSite?: (url: string) => void
  onShareProject?: () => void
  onDownloadCode?: () => void
  onNewProject?: () => void
  onCustomize?: () => void
}

export const DeploymentSuccess = ({
  projectName = 'My Project',
  deploymentUrl = 'https://my-project.openagents.dev',
  stats = {
    filesGenerated: 5,
    linesOfCode: 287,
    deploymentTime: 45,
    regions: ['US', 'EU', 'ASIA'],
    buildSize: '24.6 KB'
  },
  showCelebration = true,
  showActions = true,
  showStats = true,
  showNextSteps = true,
  animated = true,
  className = '',
  onVisitSite,
  onShareProject,
  onDownloadCode,
  onNewProject,
  onCustomize
}: DeploymentSuccessProps) => {
  const [active, setActive] = useState(false)
  const [celebrationComplete, setCelebrationComplete] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 200)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  useEffect(() => {
    if (showCelebration && active) {
      const timer = setTimeout(() => setCelebrationComplete(true), 2000)
      return () => clearTimeout(timer)
    }
  }, [showCelebration, active])

  const formatDuration = (seconds: number): string => {
    if (seconds < 60) return `${seconds}s`
    const minutes = Math.floor(seconds / 60)
    const remainingSeconds = seconds % 60
    return `${minutes}m ${remainingSeconds}s`
  }

  const successContent = (
    <div
      className={cx(
        'flex flex-col bg-black border border-green-500/30 rounded-lg overflow-hidden',
        'shadow-lg shadow-green-500/20',
        className
      )}
    >
      {/* Celebration Header */}
      {showCelebration && (
        <div className="relative bg-gradient-to-r from-green-500/10 via-cyan-500/10 to-green-500/10 border-b border-green-500/20 p-8 text-center overflow-hidden">
          {/* Animated Background */}
          <div className="absolute inset-0 opacity-20">
            <div className="absolute top-4 left-4 animate-bounce" style={{ animationDelay: '0s' }}>
              <StarIcon className="text-yellow-400" />
            </div>
            <div className="absolute top-8 right-8 animate-bounce" style={{ animationDelay: '0.5s' }}>
              <RocketIcon className="text-cyan-400" />
            </div>
            <div className="absolute bottom-4 left-1/4 animate-bounce" style={{ animationDelay: '1s' }}>
              <StarIcon className="text-yellow-400" />
            </div>
            <div className="absolute bottom-8 right-1/4 animate-bounce" style={{ animationDelay: '1.5s' }}>
              <RocketIcon className="text-cyan-400" />
            </div>
          </div>

          <div className="relative z-10">
            <div className="mb-6">
              <div className={cx(
                'inline-flex items-center justify-center w-20 h-20 rounded-full',
                'bg-green-500/20 border border-green-500/30 mb-4',
                celebrationComplete && 'animate-pulse'
              )}>
                <CheckIcon className="text-green-400" />
              </div>
            </div>

            <Text as="h1" className="text-3xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-green-400 via-cyan-400 to-green-400 mb-4">
              🎉 Deployment Successful!
            </Text>

            <Text className="text-xl text-gray-300 mb-6">
              Your {projectName} is now live and accessible worldwide
            </Text>

            <StatusBadge
              status="deployed"
              size="medium"
              animated={false}
            />
          </div>
        </div>
      )}

      {/* Deployment URL */}
      <div className="p-6 border-b border-cyan-500/20 bg-cyan-500/5">
        <div className="text-center">
          <Text className="text-sm text-gray-400 mb-3">Your live website</Text>
          <div className="max-w-md mx-auto">
            <DeploymentUrl
              url={deploymentUrl}
              status="active"
              animated={false}
              onVisit={onVisitSite}
            />
          </div>
        </div>
      </div>

      {/* Primary Actions */}
      {showActions && (
        <div className="p-6 border-b border-cyan-500/20">
          <div className="flex flex-wrap items-center justify-center gap-4">
            <button
              onClick={() => onVisitSite?.(deploymentUrl)}
              className="inline-flex items-center gap-2 px-6 py-3 bg-green-500/20 text-green-300 border border-green-500/50 rounded hover:bg-green-500/30 transition-colors cursor-pointer font-medium"
            >
              <ExternalLinkIcon className="w-5 h-5" />
              Visit Your Site
            </button>

            <button
              onClick={onShareProject}
              className="inline-flex items-center gap-2 px-6 py-3 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors cursor-pointer"
            >
              <ShareIcon className="w-5 h-5" />
              Share Project
            </button>

            <button
              onClick={onDownloadCode}
              className="inline-flex items-center gap-2 px-4 py-3 bg-gray-500/20 text-gray-300 border border-gray-500/50 rounded hover:bg-gray-500/30 transition-colors cursor-pointer"
            >
              <DownloadIcon className="w-5 h-5" />
              Download Code
            </button>
          </div>
        </div>
      )}

      {/* Deployment Stats */}
      {showStats && (
        <div className="p-6 border-b border-cyan-500/20">
          <Text as="h3" className="text-lg font-medium text-cyan-300 mb-4 text-center">
            Deployment Statistics
          </Text>
          
          <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
            <div className="text-center p-4 bg-black/30 rounded border border-gray-600">
              <div className="text-2xl font-bold text-green-400 mb-1">
                {stats.filesGenerated}
              </div>
              <div className="text-sm text-gray-400">Files Generated</div>
            </div>
            
            <div className="text-center p-4 bg-black/30 rounded border border-gray-600">
              <div className="text-2xl font-bold text-cyan-400 mb-1">
                {stats.linesOfCode.toLocaleString()}
              </div>
              <div className="text-sm text-gray-400">Lines of Code</div>
            </div>
            
            <div className="text-center p-4 bg-black/30 rounded border border-gray-600">
              <div className="text-2xl font-bold text-yellow-400 mb-1">
                {formatDuration(stats.deploymentTime)}
              </div>
              <div className="text-sm text-gray-400">Deploy Time</div>
            </div>
            
            <div className="text-center p-4 bg-black/30 rounded border border-gray-600">
              <div className="text-2xl font-bold text-purple-400 mb-1">
                {stats.regions.length}
              </div>
              <div className="text-sm text-gray-400">Global Regions</div>
            </div>
            
            <div className="text-center p-4 bg-black/30 rounded border border-gray-600">
              <div className="text-2xl font-bold text-orange-400 mb-1">
                {stats.buildSize}
              </div>
              <div className="text-sm text-gray-400">Bundle Size</div>
            </div>
            
            <div className="text-center p-4 bg-black/30 rounded border border-gray-600">
              <div className="text-2xl font-bold text-green-400 mb-1">
                100%
              </div>
              <div className="text-sm text-gray-400">Uptime</div>
            </div>
          </div>
        </div>
      )}

      {/* Next Steps */}
      {showNextSteps && (
        <div className="p-6">
          <Text as="h3" className="text-lg font-medium text-cyan-300 mb-4 text-center">
            What's Next?
          </Text>
          
          <div className="space-y-3">
            <button
              onClick={onCustomize}
              className="w-full flex items-center gap-3 p-4 bg-black/30 border border-gray-600 rounded hover:border-cyan-500/50 hover:bg-cyan-500/5 transition-all cursor-pointer"
            >
              <CodeIcon className="w-5 h-5 text-cyan-400" />
              <div className="text-left">
                <div className="font-medium text-gray-200">Customize Your Site</div>
                <div className="text-sm text-gray-400">Make changes and redeploy instantly</div>
              </div>
            </button>
            
            <button
              onClick={onNewProject}
              className="w-full flex items-center gap-3 p-4 bg-black/30 border border-gray-600 rounded hover:border-green-500/50 hover:bg-green-500/5 transition-all cursor-pointer"
            >
              <RocketIcon className="w-5 h-5 text-green-400" />
              <div className="text-left">
                <div className="font-medium text-gray-200">Start New Project</div>
                <div className="text-sm text-gray-400">Build something else amazing</div>
              </div>
            </button>
            
            <div className="flex gap-3">
              <CopyButton
                text={deploymentUrl}
                label="Copy URL"
                variant="text"
                size="small"
                animated={false}
              />
              
              <CopyButton
                text={`Check out my new website built with OpenAgents: ${deploymentUrl}`}
                label="Copy Share Text"
                variant="text"
                size="small"
                animated={false}
              />
            </div>
          </div>
        </div>
      )}
    </div>
  )

  if (!animated) {
    return successContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.6 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['scale', 0.9, 1]]}>
          {successContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Templates/DeploymentSuccess',
  component: DeploymentSuccess,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Celebration screen shown after successful deployment. Provides deployment statistics, next actions, and sharing options.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    projectName: {
      control: 'text',
      description: 'Name of the deployed project'
    },
    deploymentUrl: {
      control: 'text',
      description: 'URL of the deployed site'
    },
    stats: {
      control: 'object',
      description: 'Deployment statistics'
    },
    showCelebration: {
      control: 'boolean',
      description: 'Show celebration header'
    },
    showActions: {
      control: 'boolean',
      description: 'Show primary action buttons'
    },
    showStats: {
      control: 'boolean',
      description: 'Show deployment statistics'
    },
    showNextSteps: {
      control: 'boolean',
      description: 'Show next steps section'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof DeploymentSuccess>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    projectName: 'Bitcoin Puns Website'
  }
}

export const LargeProject: Story = {
  args: {
    projectName: 'E-commerce Platform',
    deploymentUrl: 'https://ecommerce-platform.openagents.dev',
    stats: {
      filesGenerated: 24,
      linesOfCode: 1847,
      deploymentTime: 127,
      regions: ['US-EAST', 'US-WEST', 'EU-CENTRAL', 'ASIA-PACIFIC'],
      buildSize: '156.8 KB'
    }
  }
}

export const QuickDeploy: Story = {
  args: {
    projectName: 'Simple Landing Page',
    deploymentUrl: 'https://landing.openagents.dev',
    stats: {
      filesGenerated: 3,
      linesOfCode: 89,
      deploymentTime: 18,
      regions: ['US', 'EU'],
      buildSize: '12.4 KB'
    }
  }
}

export const MinimalView: Story = {
  args: {
    projectName: 'My Website',
    showCelebration: false,
    showStats: false,
    showNextSteps: false
  }
}

export const NoActions: Story = {
  args: {
    projectName: 'Read-only Project',
    showActions: false
  }
}

export const ComprehensiveStats: Story = {
  args: {
    projectName: 'Full-stack Application',
    stats: {
      filesGenerated: 47,
      linesOfCode: 3254,
      deploymentTime: 248,
      regions: ['US-EAST-1', 'US-WEST-1', 'EU-WEST-1', 'EU-CENTRAL-1', 'ASIA-SE-1', 'ASIA-NE-1'],
      buildSize: '342.7 KB'
    }
  }
}

export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    const [message, setMessage] = useState('')

    const handleVisitSite = (url: string) => {
      setMessage(`Opening ${url} in new tab`)
      setTimeout(() => setMessage(''), 3000)
    }

    const handleShareProject = () => {
      setMessage('Project shared to social media!')
      setTimeout(() => setMessage(''), 3000)
    }

    const handleDownloadCode = () => {
      setMessage('Downloading project code as ZIP file...')
      setTimeout(() => setMessage(''), 3000)
    }

    const handleNewProject = () => {
      setMessage('Starting new project...')
      setTimeout(() => setMessage(''), 3000)
    }

    const handleCustomize = () => {
      setMessage('Opening project editor...')
      setTimeout(() => setMessage(''), 3000)
    }

    return (
      <div className="space-y-4">
        <DeploymentSuccess
          projectName="Interactive Demo"
          onVisitSite={handleVisitSite}
          onShareProject={handleShareProject}
          onDownloadCode={handleDownloadCode}
          onNewProject={handleNewProject}
          onCustomize={handleCustomize}
        />
        
        {message && (
          <div className="text-center p-4 bg-cyan-500/10 border border-cyan-500/30 rounded text-cyan-300">
            {message}
          </div>
        )}
      </div>
    )
  }
}

export const DifferentProjects: Story = {
  args: {},
  render: () => (
    <div className="space-y-8">
      <DeploymentSuccess
        projectName="Personal Blog"
        deploymentUrl="https://my-blog.openagents.dev"
        stats={{
          filesGenerated: 8,
          linesOfCode: 456,
          deploymentTime: 32,
          regions: ['US', 'EU'],
          buildSize: '67.2 KB'
        }}
        showCelebration={false}
      />
      
      <DeploymentSuccess
        projectName="Portfolio Website"
        deploymentUrl="https://portfolio.openagents.dev"
        stats={{
          filesGenerated: 12,
          linesOfCode: 623,
          deploymentTime: 41,
          regions: ['US', 'EU', 'ASIA'],
          buildSize: '89.4 KB'
        }}
        showCelebration={false}
      />
      
      <DeploymentSuccess
        projectName="Company Website"
        deploymentUrl="https://company.openagents.dev"
        stats={{
          filesGenerated: 31,
          linesOfCode: 2134,
          deploymentTime: 156,
          regions: ['US-EAST', 'US-WEST', 'EU-CENTRAL', 'ASIA-PACIFIC'],
          buildSize: '287.6 KB'
        }}
        showCelebration={false}
      />
    </div>
  )
}

export const Playground: Story = {
  args: {
    projectName: 'My Project',
    deploymentUrl: 'https://my-project.openagents.dev',
    stats: {
      filesGenerated: 5,
      linesOfCode: 287,
      deploymentTime: 45,
      regions: ['US', 'EU', 'ASIA'],
      buildSize: '24.6 KB'
    },
    showCelebration: true,
    showActions: true,
    showStats: true,
    showNextSteps: true,
    animated: true
  }
}