import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, cx } from '@arwes/react'

// Icon components
const CopyIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
  </svg>
)

const CheckIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="20 6 9 17 4 12" />
  </svg>
)

// CopyButton component
export interface CopyButtonProps {
  text: string
  label?: string
  successLabel?: string
  variant?: 'icon' | 'text' | 'both'
  size?: 'small' | 'medium' | 'large'
  color?: 'cyan' | 'yellow' | 'green' | 'red' | 'purple'
  position?: 'left' | 'right'
  animated?: boolean
  disabled?: boolean
  className?: string
  onCopy?: (text: string) => void
  onError?: (error: Error) => void
}

export const CopyButton = ({
  text,
  label = 'Copy',
  successLabel = 'Copied!',
  variant = 'both',
  size = 'medium',
  color = 'cyan',
  position = 'right',
  animated = true,
  disabled = false,
  className = '',
  onCopy,
  onError
}: CopyButtonProps) => {
  const [active, setActive] = useState(false)
  const [copied, setCopied] = useState(false)
  const [error, setError] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const handleCopy = async () => {
    if (disabled || copied) return

    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      setError(false)
      onCopy?.(text)
      
      setTimeout(() => {
        setCopied(false)
      }, 2000)
    } catch (err) {
      setError(true)
      onError?.(err as Error)
      console.error('Failed to copy:', err)
      
      setTimeout(() => {
        setError(false)
      }, 2000)
    }
  }

  const colorClasses = {
    cyan: {
      bg: 'bg-cyan-500/20',
      hover: 'hover:bg-cyan-500/30',
      border: 'border-cyan-500/50',
      text: 'text-cyan-300',
      success: 'text-green-400 border-green-500/50',
      error: 'text-red-400 border-red-500/50'
    },
    yellow: {
      bg: 'bg-yellow-500/20',
      hover: 'hover:bg-yellow-500/30',
      border: 'border-yellow-500/50',
      text: 'text-yellow-300',
      success: 'text-green-400 border-green-500/50',
      error: 'text-red-400 border-red-500/50'
    },
    green: {
      bg: 'bg-green-500/20',
      hover: 'hover:bg-green-500/30',
      border: 'border-green-500/50',
      text: 'text-green-300',
      success: 'text-green-400 border-green-500/50',
      error: 'text-red-400 border-red-500/50'
    },
    red: {
      bg: 'bg-red-500/20',
      hover: 'hover:bg-red-500/30',
      border: 'border-red-500/50',
      text: 'text-red-300',
      success: 'text-green-400 border-green-500/50',
      error: 'text-red-400 border-red-500/50'
    },
    purple: {
      bg: 'bg-purple-500/20',
      hover: 'hover:bg-purple-500/30',
      border: 'border-purple-500/50',
      text: 'text-purple-300',
      success: 'text-green-400 border-green-500/50',
      error: 'text-red-400 border-red-500/50'
    }
  }

  const sizeClasses = {
    small: {
      padding: variant === 'icon' ? 'p-1' : 'px-2 py-1',
      text: 'text-xs',
      icon: 'w-3 h-3',
      gap: 'gap-1'
    },
    medium: {
      padding: variant === 'icon' ? 'p-1.5' : 'px-3 py-1.5',
      text: 'text-sm',
      icon: 'w-4 h-4',
      gap: 'gap-1.5'
    },
    large: {
      padding: variant === 'icon' ? 'p-2' : 'px-4 py-2',
      text: 'text-base',
      icon: 'w-5 h-5',
      gap: 'gap-2'
    }
  }

  const colors = colorClasses[color]
  const sizes = sizeClasses[size]

  const currentLabel = copied ? successLabel : error ? 'Error!' : label
  const CurrentIcon = copied ? CheckIcon : CopyIcon

  const buttonContent = (
    <button
      onClick={handleCopy}
      disabled={disabled}
      className={cx(
        'inline-flex items-center justify-center rounded border transition-all duration-200',
        colors.bg,
        colors.border,
        colors.text,
        !disabled && colors.hover,
        !disabled && 'cursor-pointer',
        disabled && 'opacity-50 cursor-not-allowed',
        // Keep original color when copied - don't change to success color
        error && colors.error,
        sizes.padding,
        sizes.gap,
        'active:scale-95',
        className
      )}
    >
      {variant !== 'text' && position === 'left' && (
        <CurrentIcon className={cx(sizes.icon, 'flex-shrink-0')} />
      )}
      
      {variant !== 'icon' && (
        <span className={cx(sizes.text, 'font-medium')}>
          {currentLabel}
        </span>
      )}
      
      {variant !== 'text' && position === 'right' && (
        <CurrentIcon className={cx(sizes.icon, 'flex-shrink-0')} />
      )}
    </button>
  )

  if (!animated) {
    return buttonContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.3, exit: 0.2 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['scale', 0.8, 1]]}>
          {buttonContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Atoms/CopyButton',
  component: CopyButton,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Copy to clipboard button with visual feedback. Shows success/error states and supports multiple variants and sizes.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    text: {
      control: 'text',
      description: 'Text to copy to clipboard'
    },
    label: {
      control: 'text',
      description: 'Button label text'
    },
    successLabel: {
      control: 'text',
      description: 'Label shown when copy succeeds'
    },
    variant: {
      control: 'select',
      options: ['icon', 'text', 'both'],
      description: 'Visual variant of the button'
    },
    size: {
      control: 'select',
      options: ['small', 'medium', 'large'],
      description: 'Size of the button'
    },
    color: {
      control: 'select',
      options: ['cyan', 'yellow', 'green', 'red', 'purple'],
      description: 'Color theme'
    },
    position: {
      control: 'select',
      options: ['left', 'right'],
      description: 'Icon position when variant is "both"'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    },
    disabled: {
      control: 'boolean',
      description: 'Disable the button'
    }
  }
} satisfies Meta<typeof CopyButton>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    text: 'Hello, OpenAgents!'
  }
}

export const VariantShowcase: Story = {
  render: () => (
    <div className="space-y-4">
      <div className="flex items-center gap-4">
        <CopyButton text="Icon only" variant="icon" />
        <span className="text-gray-400 text-sm">Icon variant</span>
      </div>
      <div className="flex items-center gap-4">
        <CopyButton text="Text only" variant="text" />
        <span className="text-gray-400 text-sm">Text variant</span>
      </div>
      <div className="flex items-center gap-4">
        <CopyButton text="Icon and text" variant="both" />
        <span className="text-gray-400 text-sm">Both variant</span>
      </div>
    </div>
  )
}

export const SizeComparison: Story = {
  render: () => (
    <div className="space-y-4">
      <div className="flex items-center gap-4">
        <CopyButton text="Small button" size="small" />
        <CopyButton text="Small button" size="small" variant="icon" />
      </div>
      <div className="flex items-center gap-4">
        <CopyButton text="Medium button" size="medium" />
        <CopyButton text="Medium button" size="medium" variant="icon" />
      </div>
      <div className="flex items-center gap-4">
        <CopyButton text="Large button" size="large" />
        <CopyButton text="Large button" size="large" variant="icon" />
      </div>
    </div>
  )
}

export const ColorVariants: Story = {
  render: () => (
    <div className="space-y-2">
      <CopyButton text="Cyan theme" color="cyan" />
      <CopyButton text="Yellow theme" color="yellow" />
      <CopyButton text="Green theme" color="green" />
      <CopyButton text="Red theme" color="red" />
      <CopyButton text="Purple theme" color="purple" />
    </div>
  )
}

export const IconPosition: Story = {
  render: () => (
    <div className="space-y-4">
      <CopyButton text="Icon on left" position="left" />
      <CopyButton text="Icon on right" position="right" />
    </div>
  )
}

export const CustomLabels: Story = {
  render: () => (
    <div className="space-y-4">
      <CopyButton 
        text="npm install @openagents/sdk" 
        label="Copy command"
        successLabel="Command copied!"
      />
      <CopyButton 
        text="https://openagents.dev" 
        label="Copy URL"
        successLabel="URL copied!"
      />
      <CopyButton 
        text="hello@openagents.com" 
        label="Copy email"
        successLabel="Email copied!"
      />
    </div>
  )
}

export const DisabledState: Story = {
  render: () => (
    <div className="space-y-4">
      <CopyButton text="Enabled button" disabled={false} />
      <CopyButton text="Disabled button" disabled={true} />
      <CopyButton text="Disabled icon" variant="icon" disabled={true} />
    </div>
  )
}

export const InContext: Story = {
  render: () => (
    <div className="space-y-6">
      <div className="p-4 bg-gray-900/50 rounded border border-gray-700">
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-cyan-300">API Key</h3>
          <CopyButton 
            text="sk-1234567890abcdef" 
            variant="icon" 
            size="small" 
          />
        </div>
        <code className="text-xs text-gray-400 font-mono">sk-1234567890abcdef</code>
      </div>
      
      <div className="p-4 bg-gray-900/50 rounded border border-gray-700">
        <div className="mb-2">
          <h3 className="text-cyan-300 mb-1">Installation</h3>
          <div className="flex items-center gap-2">
            <code className="text-sm text-gray-300 font-mono flex-1">
              npm install @openagents/sdk
            </code>
            <CopyButton 
              text="npm install @openagents/sdk" 
              label="Copy"
              size="small"
            />
          </div>
        </div>
      </div>
    </div>
  )
}

export const InteractiveDemo: Story = {
  render: () => {
    const [copyCount, setCopyCount] = useState(0)
    const [lastCopied, setLastCopied] = useState('')
    
    return (
      <div className="space-y-4 text-center">
        <div className="space-y-2">
          <CopyButton 
            text="Click me to copy this text!"
            onCopy={(text) => {
              setCopyCount(c => c + 1)
              setLastCopied(text)
            }}
          />
        </div>
        
        {copyCount > 0 && (
          <div className="text-sm text-gray-400">
            <p>Copied {copyCount} time{copyCount !== 1 ? 's' : ''}</p>
            <p className="text-xs mt-1">Last copied: "{lastCopied}"</p>
          </div>
        )}
      </div>
    )
  }
}

export const ErrorHandling: Story = {
  render: () => {
    // Simulate clipboard API not available
    const [error, setError] = useState('')
    
    return (
      <div className="space-y-4">
        <p className="text-gray-400 text-sm">
          This demo simulates error handling when clipboard API fails
        </p>
        <CopyButton 
          text="Test error handling"
          onError={(err) => {
            setError(err.message)
            setTimeout(() => setError(''), 3000)
          }}
        />
        {error && (
          <p className="text-red-400 text-sm">Error: {error}</p>
        )}
      </div>
    )
  }
}

export const Playground: Story = {
  args: {
    text: 'Playground text to copy',
    label: 'Copy',
    successLabel: 'Copied!',
    variant: 'both',
    size: 'medium',
    color: 'cyan',
    position: 'right',
    animated: true,
    disabled: false
  }
}