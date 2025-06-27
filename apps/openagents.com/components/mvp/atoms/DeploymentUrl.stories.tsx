import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'

// Icon components
const LinkIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
    <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
  </svg>
)

const ExternalLinkIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
    <polyline points="15 3 21 3 21 9" />
    <line x1="10" y1="14" x2="21" y2="3" />
  </svg>
)

const CheckIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="20 6 9 17 4 12" />
  </svg>
)

const CopyIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
  </svg>
)

// DeploymentUrl component
export interface DeploymentUrlProps {
  url: string
  status?: 'pending' | 'active' | 'error'
  showCopyButton?: boolean
  showExternalLink?: boolean
  truncate?: boolean
  size?: 'small' | 'medium' | 'large'
  animated?: boolean
  className?: string
  onCopy?: (url: string) => void
  onClick?: (url: string) => void
}

export const DeploymentUrl = ({
  url = '',
  status = 'active',
  showCopyButton = true,
  showExternalLink = true,
  truncate = true,
  size = 'medium',
  animated = true,
  className = '',
  onCopy,
  onClick
}: DeploymentUrlProps) => {
  const [active, setActive] = useState(false)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 200)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation()
    try {
      await navigator.clipboard.writeText(url)
      setCopied(true)
      onCopy?.(url)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy:', err)
    }
  }

  const handleClick = () => {
    if (onClick) {
      onClick(url)
    } else if (showExternalLink && status === 'active') {
      window.open(url, '_blank', 'noopener,noreferrer')
    }
  }

  const statusColors = {
    pending: {
      bg: 'bg-gray-500/10',
      border: 'border-gray-500/30',
      text: 'text-gray-400',
      hover: 'hover:bg-gray-500/20',
      glow: ''
    },
    active: {
      bg: 'bg-cyan-500/10',
      border: 'border-cyan-500/30',
      text: 'text-cyan-300',
      hover: 'hover:bg-cyan-500/20 hover:border-cyan-500/50',
      glow: 'shadow-lg shadow-cyan-500/20'
    },
    error: {
      bg: 'bg-red-500/10',
      border: 'border-red-500/30',
      text: 'text-red-400',
      hover: 'hover:bg-red-500/20',
      glow: ''
    }
  }

  const sizeClasses = {
    small: {
      padding: 'px-3 py-1',
      text: 'text-xs',
      icon: 'w-3 h-3',
      gap: 'gap-1.5'
    },
    medium: {
      padding: 'px-4 py-2',
      text: 'text-sm',
      icon: 'w-4 h-4',
      gap: 'gap-2'
    },
    large: {
      padding: 'px-5 py-2.5',
      text: 'text-base',
      icon: 'w-5 h-5',
      gap: 'gap-2.5'
    }
  }

  const colors = statusColors[status]
  const sizes = sizeClasses[size]

  const displayUrl = truncate && url && url.length > 50 
    ? url.substring(0, 47) + '...' 
    : url

  const content = (
    <div
      className={cx(
        'group relative inline-flex items-center rounded border transition-all duration-300 cursor-pointer',
        colors.bg,
        colors.border,
        colors.text,
        colors.hover,
        status === 'active' && colors.glow,
        sizes.padding,
        sizes.gap,
        className
      )}
      onClick={handleClick}
    >
      <LinkIcon className={cx(sizes.icon, 'flex-shrink-0')} />
      
      <Text 
        as="span" 
        manager={animated && status === 'active' ? 'sequence' : undefined}
        className={cx(
          sizes.text,
          'font-mono',
          truncate && 'truncate',
          'flex-1'
        )}
      >
        {displayUrl}
      </Text>

      {showExternalLink && status === 'active' && (
        <ExternalLinkIcon 
          className={cx(
            sizes.icon,
            'flex-shrink-0 opacity-0 group-hover:opacity-100 transition-opacity'
          )} 
        />
      )}

      {showCopyButton && (
        <button
          onClick={handleCopy}
          className={cx(
            'flex-shrink-0 p-1 rounded transition-all duration-200 cursor-pointer',
            'hover:bg-white/10',
            copied ? 'text-green-400' : ''
          )}
          title="Copy URL"
        >
          {copied ? (
            <CheckIcon className={sizes.icon} />
          ) : (
            <CopyIcon className={cx(sizes.icon, 'opacity-60 hover:opacity-100')} />
          )}
        </button>
      )}

      {/* Tooltip on hover */}
      {truncate && url && url.length > 50 && (
        <div className="absolute bottom-full left-0 mb-2 px-3 py-1 bg-gray-900 text-cyan-300 text-xs rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-10">
          {url}
          <div className="absolute top-full left-4 w-0 h-0 border-l-4 border-r-4 border-t-4 border-transparent border-t-gray-900" />
        </div>
      )}
    </div>
  )

  if (!animated || status !== 'active') {
    return content
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['x', -20, 0]]}>
          {content}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Atoms/DeploymentUrl',
  component: DeploymentUrl,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Clickable deployment URL component with copy functionality and status indicators. Shows deployed application URLs with appropriate styling and interactions.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    url: {
      control: 'text',
      description: 'The deployment URL to display'
    },
    status: {
      control: 'select',
      options: ['pending', 'active', 'error'],
      description: 'Current status of the deployment'
    },
    showCopyButton: {
      control: 'boolean',
      description: 'Show copy to clipboard button'
    },
    showExternalLink: {
      control: 'boolean',
      description: 'Show external link icon and enable click to open'
    },
    truncate: {
      control: 'boolean',
      description: 'Truncate long URLs with ellipsis'
    },
    size: {
      control: 'select',
      options: ['small', 'medium', 'large'],
      description: 'Size variant of the component'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof DeploymentUrl>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    url: 'https://bitcoin-puns-xyz.openagents.dev'
  }
}

export const StatusVariants: Story = {
  render: () => (
    <div className="space-y-4">
      <div>
        <p className="text-gray-400 text-sm mb-2">Pending deployment:</p>
        <DeploymentUrl 
          url="https://my-app.openagents.dev" 
          status="pending" 
        />
      </div>
      <div>
        <p className="text-gray-400 text-sm mb-2">Active deployment:</p>
        <DeploymentUrl 
          url="https://my-app.openagents.dev" 
          status="active" 
        />
      </div>
      <div>
        <p className="text-gray-400 text-sm mb-2">Failed deployment:</p>
        <DeploymentUrl 
          url="https://my-app.openagents.dev" 
          status="error" 
        />
      </div>
    </div>
  )
}

export const SizeVariants: Story = {
  render: () => (
    <div className="space-y-4">
      <DeploymentUrl 
        url="https://small-app.openagents.dev" 
        size="small" 
      />
      <DeploymentUrl 
        url="https://medium-app.openagents.dev" 
        size="medium" 
      />
      <DeploymentUrl 
        url="https://large-app.openagents.dev" 
        size="large" 
      />
    </div>
  )
}

export const LongUrl: Story = {
  args: {
    url: 'https://my-super-long-application-name-that-exceeds-fifty-characters.openagents.dev/path/to/resource'
  }
}

export const NoTruncate: Story = {
  args: {
    url: 'https://my-super-long-application-name-that-exceeds-fifty-characters.openagents.dev',
    truncate: false
  }
}

export const MinimalMode: Story = {
  args: {
    url: 'https://minimal.openagents.dev',
    showCopyButton: false,
    showExternalLink: false
  }
}

export const CopyDemo: Story = {
  args: {
    url: 'https://example.com'
  },
  render: () => {
    const [message, setMessage] = useState('')
    
    return (
      <div className="space-y-4">
        <DeploymentUrl 
          url="https://copy-demo.openagents.dev"
          onCopy={(url) => {
            setMessage(`Copied: ${url}`)
            setTimeout(() => setMessage(''), 3000)
          }}
        />
        {message && (
          <p className="text-green-400 text-sm animate-pulse">{message}</p>
        )}
      </div>
    )
  }
}

export const MultipleUrls: Story = {
  render: () => (
    <div className="space-y-3">
      <div className="p-4 bg-gray-900/50 rounded">
        <h3 className="text-cyan-300 mb-3">Your Deployed Applications</h3>
        <div className="space-y-2">
          <DeploymentUrl url="https://bitcoin-puns.openagents.dev" />
          <DeploymentUrl url="https://weather-app.openagents.dev" />
          <DeploymentUrl url="https://task-manager.openagents.dev" />
          <DeploymentUrl url="https://blog-platform.openagents.dev" status="pending" />
          <DeploymentUrl url="https://failed-deploy.openagents.dev" status="error" />
        </div>
      </div>
    </div>
  )
}

export const AnimationSequence: Story = {
  args: {
    url: 'https://example.com'
  },
  render: () => {
    const [key, setKey] = useState(0)
    
    return (
      <div className="space-y-4">
        <div key={key} className="space-y-2">
          {['app-1', 'app-2', 'app-3'].map((app, index) => (
            <div key={app} style={{ animationDelay: `${index * 200}ms` }}>
              <DeploymentUrl 
                url={`https://${app}.openagents.dev`}
                animated={true}
              />
            </div>
          ))}
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

export const Playground: Story = {
  args: {
    url: 'https://playground-app.openagents.dev',
    status: 'active',
    showCopyButton: true,
    showExternalLink: true,
    truncate: true,
    size: 'medium',
    animated: true
  }
}