import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'

// Icon components
const BrainIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M12 2C8 2 4.5 5 4.5 9c0 1.5.5 3 1.5 4v5l3 2 3-2 3 2 3-2v-5c1-1 1.5-2.5 1.5-4 0-4-3.5-7-7.5-7z" />
    <path d="M9 10h.01M15 10h.01M9.5 15.5s1.5 1.5 3 1.5 3-1.5 3-1.5" />
  </svg>
)

const SparklesIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M12 3L13.5 8.5L19 10L13.5 11.5L12 17L10.5 11.5L5 10L10.5 8.5L12 3Z" />
    <path d="M5 3L5.75 5.25L8 6L5.75 6.75L5 9L4.25 6.75L2 6L4.25 5.25L5 3Z" opacity="0.5" />
    <path d="M19 15L19.75 17.25L22 18L19.75 18.75L19 21L18.25 18.75L16 18L18.25 17.25L19 15Z" opacity="0.5" />
  </svg>
)

const ZapIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
  </svg>
)

const RocketIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M9 11L7 21L12 17L17 21L15 11M12 2C12 2 17 4 17 11L12 13L7 11C7 4 12 2 12 2Z" />
  </svg>
)

// ModelBadge component
export interface ModelBadgeProps {
  model: string
  provider?: 'cloudflare' | 'openrouter' | 'openai' | 'anthropic' | 'custom'
  showIcon?: boolean
  size?: 'small' | 'medium' | 'large'
  variant?: 'solid' | 'outline' | 'ghost'
  animated?: boolean
  truncate?: boolean
  maxLength?: number
  className?: string
  onClick?: () => void
}

export const ModelBadge = ({
  model = '',
  provider = 'cloudflare',
  showIcon = true,
  size = 'medium',
  variant = 'solid',
  animated = true,
  truncate = true,
  maxLength = 30,
  className = '',
  onClick
}: ModelBadgeProps) => {
  const [active, setActive] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 150)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const providerConfig = {
    cloudflare: {
      icon: SparklesIcon,
      color: 'yellow',
      name: 'Cloudflare AI'
    },
    openrouter: {
      icon: SparklesIcon,
      color: 'cyan',
      name: 'OpenRouter'
    },
    openai: {
      icon: BrainIcon,
      color: 'green',
      name: 'OpenAI'
    },
    anthropic: {
      icon: RocketIcon,
      color: 'purple',
      name: 'Anthropic'
    },
    custom: {
      icon: SparklesIcon,
      color: 'cyan',
      name: 'Custom'
    }
  }

  const colorStyles = {
    yellow: {
      solid: 'bg-yellow-500/20 border-yellow-500/50 text-yellow-300',
      outline: 'bg-transparent border-yellow-500/50 text-yellow-300',
      ghost: 'bg-transparent border-transparent text-yellow-300 hover:bg-yellow-500/10'
    },
    cyan: {
      solid: 'bg-cyan-500/20 border-cyan-500/50 text-cyan-300',
      outline: 'bg-transparent border-cyan-500/50 text-cyan-300',
      ghost: 'bg-transparent border-transparent text-cyan-300 hover:bg-cyan-500/10'
    },
    green: {
      solid: 'bg-green-500/20 border-green-500/50 text-green-300',
      outline: 'bg-transparent border-green-500/50 text-green-300',
      ghost: 'bg-transparent border-transparent text-green-300 hover:bg-green-500/10'
    },
    purple: {
      solid: 'bg-purple-500/20 border-purple-500/50 text-purple-300',
      outline: 'bg-transparent border-purple-500/50 text-purple-300',
      ghost: 'bg-transparent border-transparent text-purple-300 hover:bg-purple-500/10'
    }
  }

  const sizeClasses = {
    small: {
      padding: 'px-2 py-0.5',
      text: 'text-xs',
      icon: 'w-3 h-3',
      gap: 'gap-1'
    },
    medium: {
      padding: 'px-3 py-1',
      text: 'text-sm',
      icon: 'w-4 h-4',
      gap: 'gap-1.5'
    },
    large: {
      padding: 'px-4 py-1.5',
      text: 'text-base',
      icon: 'w-5 h-5',
      gap: 'gap-2'
    }
  }

  const config = providerConfig[provider]
  const Icon = config.icon
  const colorStyle = colorStyles[config.color][variant]
  const sizes = sizeClasses[size]

  const displayModel = truncate && model && model.length > maxLength
    ? model.substring(0, maxLength - 3) + '...'
    : model

  const badgeContent = (
    <div
      className={cx(
        'inline-flex items-center rounded-md border transition-all duration-200',
        colorStyle,
        sizes.padding,
        sizes.gap,
        onClick && 'cursor-pointer hover:scale-105 active:scale-95',
        className
      )}
      onClick={onClick}
      title={`${config.name}: ${model}`}
    >
      {showIcon && (
        <Icon className={cx(sizes.icon, 'flex-shrink-0')} />
      )}
      <Text
        as="span"
        manager={animated ? 'decipher' : undefined}
        className={cx(sizes.text, 'font-mono font-medium')}
      >
        {displayModel}
      </Text>
    </div>
  )

  if (!animated) {
    return badgeContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.4, exit: 0.2 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', -10, 0]]}>
          {badgeContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Atoms/ModelBadge',
  component: ModelBadge,
  parameters: {
    layout: 'centered',
    docs: {
      description: {
        component: 'Badge component displaying AI model information with provider-specific theming and icons.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    model: {
      control: 'text',
      description: 'Model name to display'
    },
    provider: {
      control: 'select',
      options: ['cloudflare', 'openrouter', 'openai', 'anthropic', 'custom'],
      description: 'AI provider'
    },
    showIcon: {
      control: 'boolean',
      description: 'Show provider icon'
    },
    size: {
      control: 'select',
      options: ['small', 'medium', 'large'],
      description: 'Badge size'
    },
    variant: {
      control: 'select',
      options: ['solid', 'outline', 'ghost'],
      description: 'Visual variant'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    },
    truncate: {
      control: 'boolean',
      description: 'Truncate long model names'
    },
    maxLength: {
      control: 'number',
      description: 'Maximum character length before truncation'
    }
  }
} satisfies Meta<typeof ModelBadge>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    model: 'llama-3-8b-instruct'
  }
}

export const AllProviders: Story = {
  render: () => (
    <div className="space-y-3">
      <div className="flex items-center gap-4">
        <ModelBadge model="llama-3-8b-instruct" provider="cloudflare" />
        <span className="text-gray-400 text-sm">Cloudflare Workers AI</span>
      </div>
      <div className="flex items-center gap-4">
        <ModelBadge model="claude-3-opus" provider="openrouter" />
        <span className="text-gray-400 text-sm">OpenRouter</span>
      </div>
      <div className="flex items-center gap-4">
        <ModelBadge model="gpt-4-turbo" provider="openai" />
        <span className="text-gray-400 text-sm">OpenAI</span>
      </div>
      <div className="flex items-center gap-4">
        <ModelBadge model="claude-3-sonnet" provider="anthropic" />
        <span className="text-gray-400 text-sm">Anthropic</span>
      </div>
      <div className="flex items-center gap-4">
        <ModelBadge model="custom-model-v2" provider="custom" />
        <span className="text-gray-400 text-sm">Custom Provider</span>
      </div>
    </div>
  )
}

export const SizeVariants: Story = {
  render: () => (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <ModelBadge model="llama-3-8b" size="small" />
        <ModelBadge model="llama-3-8b" size="medium" />
        <ModelBadge model="llama-3-8b" size="large" />
      </div>
    </div>
  )
}

export const VariantStyles: Story = {
  render: () => (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <ModelBadge model="gpt-4" provider="openai" variant="solid" />
        <ModelBadge model="gpt-4" provider="openai" variant="outline" />
        <ModelBadge model="gpt-4" provider="openai" variant="ghost" />
      </div>
    </div>
  )
}

export const LongModelNames: Story = {
  args: {
    model: 'example-model'
  },
  render: () => (
    <div className="space-y-3">
      <ModelBadge 
        model="meta-llama-3-70b-instruct-awq-int4" 
        provider="cloudflare"
      />
      <ModelBadge 
        model="anthropic/claude-3-opus-20240229-gcp" 
        provider="openrouter"
      />
      <ModelBadge 
        model="super-long-model-name-that-definitely-needs-truncation-v2-final" 
        provider="custom"
        maxLength={40}
      />
    </div>
  )
}

export const NoTruncation: Story = {
  args: {
    model: 'meta-llama-3-70b-instruct-awq-int4',
    truncate: false
  }
}

export const NoIcon: Story = {
  render: () => (
    <div className="space-y-3">
      <ModelBadge model="llama-3-8b" showIcon={false} />
      <ModelBadge model="claude-3-opus" provider="openrouter" showIcon={false} />
      <ModelBadge model="gpt-4" provider="openai" showIcon={false} />
    </div>
  )
}

export const Interactive: Story = {
  args: {
    model: 'example-model'
  },
  render: () => {
    const [selected, setSelected] = useState('llama-3-8b-instruct')
    const models = [
      { name: 'llama-3-8b-instruct', provider: 'cloudflare' as const },
      { name: 'llama-3-70b-instruct', provider: 'cloudflare' as const },
      { name: 'claude-3-opus', provider: 'openrouter' as const },
      { name: 'gpt-4-turbo', provider: 'openai' as const }
    ]
    
    return (
      <div className="space-y-4">
        <p className="text-gray-400 text-sm">Click to select a model:</p>
        <div className="flex flex-wrap gap-2">
          {models.map(({ name, provider }) => (
            <ModelBadge
              key={name}
              model={name}
              provider={provider}
              variant={selected === name ? 'solid' : 'outline'}
              onClick={() => setSelected(name)}
            />
          ))}
        </div>
        <p className="text-cyan-300 text-sm">
          Selected: <span className="font-mono">{selected}</span>
        </p>
      </div>
    )
  }
}

export const InContext: Story = {
  args: {
    model: 'example-model'
  },
  render: () => (
    <div className="space-y-4">
      <div className="p-4 bg-gray-900/50 rounded border border-gray-700">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-cyan-300">Chat Session</h3>
          <ModelBadge model="llama-3-8b-instruct" size="small" />
        </div>
        <p className="text-gray-300 text-sm">
          Using Cloudflare Workers AI for fast, edge-based inference.
        </p>
      </div>
      
      <div className="p-4 bg-gray-900/50 rounded border border-gray-700">
        <div className="mb-2">
          <span className="text-gray-400 text-sm">Current model:</span>
        </div>
        <div className="flex items-center gap-3">
          <ModelBadge model="claude-3-opus" provider="openrouter" />
          <button className="text-xs text-cyan-300 hover:text-cyan-200">
            Change model →
          </button>
        </div>
      </div>
    </div>
  )
}

export const ModelGrid: Story = {
  args: {
    model: 'example-model'
  },
  render: () => (
    <div className="grid grid-cols-2 gap-3 p-6 bg-black/50 rounded">
      <ModelBadge model="llama-3-8b" provider="cloudflare" variant="outline" />
      <ModelBadge model="llama-3-70b" provider="cloudflare" variant="outline" />
      <ModelBadge model="mistral-7b" provider="cloudflare" variant="outline" />
      <ModelBadge model="phi-2" provider="cloudflare" variant="outline" />
      <ModelBadge model="codellama-7b" provider="cloudflare" variant="outline" />
      <ModelBadge model="gemma-7b" provider="cloudflare" variant="outline" />
    </div>
  )
}

export const AnimationDemo: Story = {
  args: {
    model: 'example-model'
  },
  render: () => {
    const [key, setKey] = useState(0)
    
    return (
      <div className="space-y-4">
        <div key={key} className="space-y-2">
          {['cloudflare', 'openrouter', 'openai', 'anthropic'].map((provider, index) => (
            <div key={provider} style={{ animationDelay: `${index * 100}ms` }}>
              <ModelBadge 
                model={`model-${index + 1}`}
                provider={provider as any}
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
    model: 'llama-3-8b-instruct',
    provider: 'cloudflare',
    showIcon: true,
    size: 'medium',
    variant: 'solid',
    animated: true,
    truncate: true,
    maxLength: 30
  }
}