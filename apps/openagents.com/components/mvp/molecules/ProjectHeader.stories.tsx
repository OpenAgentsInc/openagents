import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { StatusBadge } from '../atoms/StatusBadge.stories'
import { ModelBadge } from '../atoms/ModelBadge.stories'
import { DeploymentUrl } from '../atoms/DeploymentUrl.stories'

// Icon components
const RocketIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M9 11L7 21L12 17L17 21L15 11M12 2C12 2 17 4 17 11L12 13L7 11C7 4 12 2 12 2Z" />
  </svg>
)

const SettingsIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="3" />
    <path d="M12 1v6M12 17v6M4.22 4.22l4.24 4.24M15.54 15.54l4.24 4.24M1 12h6M17 12h6M4.22 19.78l4.24-4.24M15.54 8.46l4.24-4.24" />
  </svg>
)

const MoreIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="1" />
    <circle cx="12" cy="5" r="1" />
    <circle cx="12" cy="19" r="1" />
  </svg>
)

const ShareIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8" />
    <polyline points="16 6 12 2 8 6" />
    <line x1="12" y1="2" x2="12" y2="15" />
  </svg>
)

const SaveIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
    <polyline points="17 21 17 13 7 13 7 21" />
    <polyline points="7 3 7 8 15 8" />
  </svg>
)

const PlayIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polygon points="5 3 19 12 5 21 5 3" />
  </svg>
)

// ProjectHeader component
export interface ProjectHeaderProps {
  projectName?: string
  status?: 'idle' | 'generating' | 'deploying' | 'deployed' | 'error'
  deploymentUrl?: string
  currentModel?: string
  currentProvider?: string
  lastSaved?: Date
  showDeployButton?: boolean
  showActions?: boolean
  showModelInfo?: boolean
  showUrl?: boolean
  isDeploying?: boolean
  animated?: boolean
  className?: string
  onDeploy?: () => void
  onSave?: () => void
  onShare?: () => void
  onSettings?: () => void
  onPreview?: () => void
}

export const ProjectHeader = ({
  projectName = 'My Project',
  status = 'idle',
  deploymentUrl,
  currentModel = 'claude-3-sonnet',
  currentProvider = 'anthropic',
  lastSaved,
  showDeployButton = true,
  showActions = true,
  showModelInfo = true,
  showUrl = true,
  isDeploying = false,
  animated = true,
  className = '',
  onDeploy,
  onSave,
  onShare,
  onSettings,
  onPreview
}: ProjectHeaderProps) => {
  const [active, setActive] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const formatTime = (date: Date): string => {
    return date.toLocaleTimeString('en-US', {
      hour12: true,
      hour: 'numeric',
      minute: '2-digit'
    })
  }

  const getDeployButtonConfig = () => {
    if (isDeploying || status === 'deploying') {
      return {
        text: 'Deploying...',
        icon: RocketIcon,
        disabled: true,
        className: 'bg-cyan-500/20 text-cyan-300 border-cyan-500/50 animate-pulse'
      }
    }
    
    if (status === 'deployed' && deploymentUrl) {
      return {
        text: 'Redeploy',
        icon: RocketIcon,
        disabled: false,
        className: 'bg-green-500/20 text-green-300 border-green-500/50 hover:bg-green-500/30'
      }
    }
    
    return {
      text: 'Deploy',
      icon: RocketIcon,
      disabled: false,
      className: 'bg-cyan-500/20 text-cyan-300 border-cyan-500/50 hover:bg-cyan-500/30'
    }
  }

  const deployConfig = getDeployButtonConfig()

  const headerContent = (
    <div
      className={cx(
        'flex items-center justify-between px-6 py-4',
        'bg-black border-b border-cyan-500/20',
        'shadow-lg shadow-cyan-500/10',
        className
      )}
    >
      {/* Left Section - Project Info */}
      <div className="flex items-center gap-4">
        <div>
          <Text as="h1" className="text-xl font-medium text-white">
            {projectName}
          </Text>
          <div className="flex items-center gap-3 mt-1">
            <StatusBadge
              status={status === 'deployed' ? 'deployed' : status === 'deploying' ? 'deploying' : status === 'generating' ? 'generating' : status === 'error' ? 'error' : 'idle'}
              size="small"
              animated={false}
            />
            
            {showModelInfo && (
              <ModelBadge
                model={currentModel}
                provider={currentProvider as any}
                variant="outline"
                size="small"
                animated={false}
              />
            )}
            
            {lastSaved && (
              <span className="text-xs text-gray-500">
                Saved {formatTime(lastSaved)}
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Center Section - Deployment URL */}
      {showUrl && deploymentUrl && status === 'deployed' && (
        <div className="flex-1 max-w-md mx-4">
          <DeploymentUrl
            url={deploymentUrl}
            status="active"
            animated={false}
            onClick={() => window.open(deploymentUrl, '_blank')}
          />
        </div>
      )}

      {/* Right Section - Actions */}
      <div className="flex items-center gap-3">
        {/* Action Buttons */}
        {showActions && (
          <div className="flex items-center gap-1">
            <button
              onClick={onSave}
              className="p-2 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer"
              title="Save project"
            >
              <SaveIcon className="w-4 h-4" />
            </button>
            
            {deploymentUrl && (
              <button
                onClick={onPreview}
                className="p-2 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer"
                title="Preview site"
              >
                <PlayIcon className="w-4 h-4" />
              </button>
            )}
            
            <button
              onClick={onShare}
              className="p-2 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer"
              title="Share project"
            >
              <ShareIcon className="w-4 h-4" />
            </button>
            
            <button
              onClick={onSettings}
              className="p-2 text-gray-400 hover:text-cyan-300 transition-colors cursor-pointer"
              title="Project settings"
            >
              <SettingsIcon className="w-4 h-4" />
            </button>
          </div>
        )}

        {/* Deploy Button */}
        {showDeployButton && (
          <button
            onClick={onDeploy}
            disabled={deployConfig.disabled}
            className={cx(
              'inline-flex items-center gap-2 px-4 py-2 border rounded font-medium transition-all duration-200 cursor-pointer',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              deployConfig.className
            )}
          >
            <deployConfig.icon className="w-4 h-4" />
            {deployConfig.text}
          </button>
        )}
      </div>
    </div>
  )

  if (!animated) {
    return headerContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', -20, 0]]}>
          {headerContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/ProjectHeader',
  component: ProjectHeader,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Project header with deployment controls, status indicators, and action buttons. Main navigation for project workspace.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    projectName: {
      control: 'text',
      description: 'Name of the project'
    },
    status: {
      control: 'select',
      options: ['idle', 'generating', 'deploying', 'deployed', 'error'],
      description: 'Current project status'
    },
    deploymentUrl: {
      control: 'text',
      description: 'URL of deployed application'
    },
    currentModel: {
      control: 'text',
      description: 'Current AI model'
    },
    currentProvider: {
      control: 'select',
      options: ['anthropic', 'openai', 'cloudflare', 'openrouter', 'custom'],
      description: 'Current AI provider'
    },
    lastSaved: {
      control: 'date',
      description: 'Last save timestamp'
    },
    showDeployButton: {
      control: 'boolean',
      description: 'Show deploy button'
    },
    showActions: {
      control: 'boolean',
      description: 'Show action buttons'
    },
    showModelInfo: {
      control: 'boolean',
      description: 'Show model badge'
    },
    showUrl: {
      control: 'boolean',
      description: 'Show deployment URL'
    },
    isDeploying: {
      control: 'boolean',
      description: 'Show deploying state'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof ProjectHeader>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    projectName: 'Bitcoin Puns Website'
  }
}

export const Deploying: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    status: 'deploying',
    isDeploying: true
  }
}

export const Deployed: Story = {
  args: {
    projectName: 'Bitcoin Puns Website',
    status: 'deployed',
    deploymentUrl: 'https://bitcoin-puns.openagents.dev',
    lastSaved: new Date(Date.now() - 300000)
  }
}

export const Generating: Story = {
  args: {
    projectName: 'E-commerce Platform',
    status: 'generating',
    currentModel: 'claude-3-opus',
    currentProvider: 'anthropic',
    lastSaved: new Date(Date.now() - 60000)
  }
}

export const WithError: Story = {
  args: {
    projectName: 'Failed Deployment',
    status: 'error',
    lastSaved: new Date(Date.now() - 120000)
  }
}

export const MinimalHeader: Story = {
  args: {
    projectName: 'Simple Project',
    showActions: false,
    showModelInfo: false,
    showDeployButton: false
  }
}

export const DifferentModel: Story = {
  args: {
    projectName: 'Landing Page',
    currentModel: 'llama-3-8b-instruct',
    currentProvider: 'cloudflare',
    status: 'deployed',
    deploymentUrl: 'https://landing-page.openagents.dev'
  }
}

export const LongProjectName: Story = {
  args: {
    projectName: 'Enterprise E-commerce Platform with Advanced Analytics and Real-time Dashboard',
    status: 'deployed',
    deploymentUrl: 'https://enterprise-ecommerce-platform.openagents.dev'
  }
}

export const RecentlySaved: Story = {
  args: {
    projectName: 'Portfolio Website',
    status: 'idle',
    lastSaved: new Date(Date.now() - 30000),
    currentModel: 'claude-3-haiku',
    currentProvider: 'anthropic'
  }
}

export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    const [status, setStatus] = useState<'idle' | 'generating' | 'deploying' | 'deployed' | 'error'>('idle')
    const [deploymentUrl, setDeploymentUrl] = useState<string>()
    const [lastSaved, setLastSaved] = useState<Date>()
    const [message, setMessage] = useState('')

    const handleDeploy = () => {
      setStatus('deploying')
      setMessage('Starting deployment...')
      
      setTimeout(() => {
        setStatus('deployed')
        setDeploymentUrl('https://bitcoin-puns.openagents.dev')
        setMessage('Deployment successful!')
        setTimeout(() => setMessage(''), 3000)
      }, 3000)
    }

    const handleSave = () => {
      setLastSaved(new Date())
      setMessage('Project saved!')
      setTimeout(() => setMessage(''), 2000)
    }

    const handleShare = () => {
      navigator.clipboard?.writeText(deploymentUrl || 'https://openagents.com/project/bitcoin-puns')
      setMessage('Project link copied to clipboard!')
      setTimeout(() => setMessage(''), 2000)
    }

    const handleSettings = () => {
      setMessage('Opening project settings...')
      setTimeout(() => setMessage(''), 2000)
    }

    const handlePreview = () => {
      setMessage('Opening site preview...')
      setTimeout(() => setMessage(''), 2000)
    }

    const resetDemo = () => {
      setStatus('idle')
      setDeploymentUrl(undefined)
      setLastSaved(undefined)
      setMessage('')
    }

    return (
      <div className="space-y-4">
        <ProjectHeader
          projectName="Interactive Demo"
          status={status}
          deploymentUrl={deploymentUrl}
          lastSaved={lastSaved}
          onDeploy={handleDeploy}
          onSave={handleSave}
          onShare={handleShare}
          onSettings={handleSettings}
          onPreview={handlePreview}
        />
        
        {message && (
          <div className="text-center p-4 bg-cyan-500/10 border border-cyan-500/30 rounded text-cyan-300">
            {message}
          </div>
        )}
        
        <div className="text-center">
          <button
            onClick={resetDemo}
            className="px-4 py-2 bg-gray-500/20 text-gray-300 border border-gray-500/50 rounded hover:bg-gray-500/30 transition-colors cursor-pointer"
          >
            Reset Demo
          </button>
        </div>
      </div>
    )
  }
}

export const MultipleStates: Story = {
  args: {},
  render: () => (
    <div className="space-y-4">
      <ProjectHeader
        projectName="Idle Project"
        status="idle"
      />
      
      <ProjectHeader
        projectName="Generating Code"
        status="generating"
      />
      
      <ProjectHeader
        projectName="Deploying to Cloud"
        status="deploying"
        isDeploying={true}
      />
      
      <ProjectHeader
        projectName="Live Website"
        status="deployed"
        deploymentUrl="https://bitcoin-puns.openagents.dev"
        lastSaved={new Date(Date.now() - 120000)}
      />
      
      <ProjectHeader
        projectName="Failed Deployment"
        status="error"
        lastSaved={new Date(Date.now() - 300000)}
      />
    </div>
  )
}

export const Playground: Story = {
  args: {
    projectName: 'My Project',
    status: 'idle',
    currentModel: 'claude-3-sonnet',
    currentProvider: 'anthropic',
    showDeployButton: true,
    showActions: true,
    showModelInfo: true,
    showUrl: true,
    animated: true
  }
}