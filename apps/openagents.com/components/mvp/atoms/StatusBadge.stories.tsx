import type { Meta, StoryObj } from '@storybook/react'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'

// Icon components (simplified for demo)
const LoaderIcon = ({ className }: { className?: string }) => (
  <svg className={cx('animate-spin', className)} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor">
    <circle cx="12" cy="12" r="10" strokeWidth="2" strokeDasharray="60" strokeDashoffset="20" />
  </svg>
)

const UploadIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M12 2L12 16M12 2L7 7M12 2L17 7M3 16L3 20C3 21.1 3.9 22 5 22L19 22C20.1 22 21 21.1 21 20L21 16" />
  </svg>
)

const CheckIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="20 6 9 17 4 12" />
  </svg>
)

const XIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <line x1="18" y1="6" x2="6" y2="18" />
    <line x1="6" y1="6" x2="18" y2="18" />
  </svg>
)

const PauseIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="6" y="4" width="4" height="16" />
    <rect x="14" y="4" width="4" height="16" />
  </svg>
)

// StatusBadge component
export interface StatusBadgeProps {
  status: 'idle' | 'generating' | 'deploying' | 'deployed' | 'error' | 'paused'
  size?: 'small' | 'medium' | 'large'
  animated?: boolean
  showPulse?: boolean
  className?: string
}

export const StatusBadge = ({
  status,
  size = 'medium',
  animated = true,
  showPulse = true,
  className = ''
}: StatusBadgeProps) => {
  const [active, setActive] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const statusConfig = {
    idle: { 
      color: 'gray', 
      icon: PauseIcon, 
      text: 'Idle',
      bgColor: 'bg-gray-500/20',
      borderColor: 'border-gray-500/50',
      textColor: 'text-gray-300',
      pulse: false
    },
    generating: { 
      color: 'cyan', 
      icon: LoaderIcon, 
      text: 'Generating',
      bgColor: 'bg-cyan-500/20',
      borderColor: 'border-cyan-500/50',
      textColor: 'text-cyan-300',
      pulse: true
    },
    deploying: { 
      color: 'yellow', 
      icon: UploadIcon, 
      text: 'Deploying',
      bgColor: 'bg-yellow-500/20',
      borderColor: 'border-yellow-500/50',
      textColor: 'text-yellow-300',
      pulse: true
    },
    deployed: { 
      color: 'green', 
      icon: CheckIcon, 
      text: 'Deployed',
      bgColor: 'bg-green-500/20',
      borderColor: 'border-green-500/50',
      textColor: 'text-green-300',
      pulse: false
    },
    error: { 
      color: 'red', 
      icon: XIcon, 
      text: 'Error',
      bgColor: 'bg-red-500/20',
      borderColor: 'border-red-500/50',
      textColor: 'text-red-300',
      pulse: false
    },
    paused: { 
      color: 'purple', 
      icon: PauseIcon, 
      text: 'Paused',
      bgColor: 'bg-purple-500/20',
      borderColor: 'border-purple-500/50',
      textColor: 'text-purple-300',
      pulse: false
    }
  }

  const config = statusConfig[status] || statusConfig.idle
  const Icon = config?.icon || PauseIcon

  const sizeClasses = {
    small: 'text-xs px-2 py-0.5 gap-1',
    medium: 'text-sm px-3 py-1 gap-1.5',
    large: 'text-base px-4 py-1.5 gap-2'
  }

  const iconSizes = {
    small: 'w-3 h-3',
    medium: 'w-4 h-4',
    large: 'w-5 h-5'
  }

  const pulseAnimation = showPulse && config.pulse ? 'animate-pulse' : ''

  const content = (
    <div 
      className={cx(
        'inline-flex items-center rounded border',
        sizeClasses[size],
        config.bgColor,
        config.borderColor,
        config.textColor,
        pulseAnimation,
        'transition-all duration-300',
        className
      )}
    >
      <Icon className={iconSizes[size]} />
      <Text as="span" manager={animated ? 'decipher' : undefined}>
        {config.text}
      </Text>
    </div>
  )

  if (!animated) {
    return content
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['scale', 0.8, 1]]}>
          {content}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Atoms/StatusBadge',
  component: StatusBadge,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Status indicator badge showing different states of operations like generating, deploying, deployed, etc. with appropriate animations and colors.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    status: {
      control: 'select',
      options: ['idle', 'generating', 'deploying', 'deployed', 'error', 'paused'],
      description: 'Current status to display'
    },
    size: {
      control: 'select',
      options: ['small', 'medium', 'large'],
      description: 'Size variant of the badge'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    },
    showPulse: {
      control: 'boolean',
      description: 'Show pulse animation for active states'
    }
  }
} satisfies Meta<typeof StatusBadge>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    status: 'generating'
  }
}

export const AllStatuses: Story = {
  render: () => (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <StatusBadge status="idle" />
        <span className="text-gray-400 text-sm">Waiting for input</span>
      </div>
      <div className="flex items-center gap-2">
        <StatusBadge status="generating" />
        <span className="text-gray-400 text-sm">AI is generating code</span>
      </div>
      <div className="flex items-center gap-2">
        <StatusBadge status="deploying" />
        <span className="text-gray-400 text-sm">Deploying to Cloudflare</span>
      </div>
      <div className="flex items-center gap-2">
        <StatusBadge status="deployed" />
        <span className="text-gray-400 text-sm">Successfully deployed</span>
      </div>
      <div className="flex items-center gap-2">
        <StatusBadge status="error" />
        <span className="text-gray-400 text-sm">Deployment failed</span>
      </div>
      <div className="flex items-center gap-2">
        <StatusBadge status="paused" />
        <span className="text-gray-400 text-sm">Operation paused</span>
      </div>
    </div>
  )
}

export const SizeVariants: Story = {
  render: () => (
    <div className="space-y-4">
      <div className="flex items-center gap-4">
        <StatusBadge status="generating" size="small" />
        <StatusBadge status="generating" size="medium" />
        <StatusBadge status="generating" size="large" />
      </div>
      <div className="flex items-center gap-4">
        <StatusBadge status="deployed" size="small" />
        <StatusBadge status="deployed" size="medium" />
        <StatusBadge status="deployed" size="large" />
      </div>
    </div>
  )
}

export const AnimationDemo: Story = {
  render: () => {
    const [key, setKey] = useState(0)
    
    return (
      <div className="space-y-4">
        <div key={key} className="flex flex-wrap gap-2">
          <StatusBadge status="idle" />
          <StatusBadge status="generating" />
          <StatusBadge status="deploying" />
          <StatusBadge status="deployed" />
          <StatusBadge status="error" />
        </div>
        <button
          onClick={() => setKey(k => k + 1)}
          className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors"
        >
          Replay Animations
        </button>
      </div>
    )
  }
}

export const StatusTransition: Story = {
  render: () => {
    const statuses: StatusBadgeProps['status'][] = ['idle', 'generating', 'deploying', 'deployed']
    const [currentIndex, setCurrentIndex] = useState(0)
    
    useEffect(() => {
      if (currentIndex < statuses.length - 1) {
        const timer = setTimeout(() => {
          setCurrentIndex(i => i + 1)
        }, 2000)
        return () => clearTimeout(timer)
      }
    }, [currentIndex])
    
    return (
      <div className="space-y-4">
        <StatusBadge status={statuses[currentIndex]} />
        <p className="text-gray-400 text-sm">Simulating deployment flow...</p>
        <button
          onClick={() => setCurrentIndex(0)}
          className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors"
        >
          Restart Flow
        </button>
      </div>
    )
  }
}

export const NoPulse: Story = {
  args: {
    status: 'generating',
    showPulse: false
  }
}

export const NoAnimation: Story = {
  args: {
    status: 'deployed',
    animated: false
  }
}

export const Playground: Story = {
  args: {
    status: 'generating',
    size: 'medium',
    animated: true,
    showPulse: true
  }
}