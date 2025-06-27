import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx, FrameCorners } from '@arwes/react'

// HeroCallToAction component
export interface HeroCallToActionProps {
  primaryText?: string
  countdownText?: string
  benefitBullets?: string[]
  showCountdown?: boolean
  countdownDuration?: number
  variant?: 'default' | 'urgent' | 'special'
  size?: 'medium' | 'large' | 'huge'
  onClick?: () => void
  onCountdownComplete?: () => void
  animated?: boolean
  pulseAnimation?: boolean
  glowIntensity?: 'low' | 'medium' | 'high'
  className?: string
}

export const HeroCallToAction = ({
  primaryText = 'Build Yours in 60 Seconds',
  countdownText = 'Start building in',
  benefitBullets = [
    '1000 free AI operations',
    'Deploy to global edge network',
    'No credit card required'
  ],
  showCountdown = true,
  countdownDuration = 60,
  variant = 'default',
  size = 'large',
  onClick,
  onCountdownComplete,
  animated = true,
  pulseAnimation = true,
  glowIntensity = 'medium',
  className = ''
}: HeroCallToActionProps) => {
  const [active, setActive] = useState(false)
  const [countdown, setCountdown] = useState(countdownDuration)
  const [isHovered, setIsHovered] = useState(false)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 300)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  useEffect(() => {
    if (!showCountdown || countdown <= 0) return

    const timer = setInterval(() => {
      setCountdown(prev => {
        if (prev <= 1) {
          onCountdownComplete?.()
          return countdownDuration // Reset
        }
        return prev - 1
      })
    }, 1000)

    return () => clearInterval(timer)
  }, [showCountdown, countdown, countdownDuration, onCountdownComplete])

  const variantStyles = {
    default: {
      bg: 'bg-cyan-500/20',
      border: 'border-cyan-500/50',
      text: 'text-cyan-300',
      hoverBg: 'hover:bg-cyan-500/30',
      shadowColor: 'cyan'
    },
    urgent: {
      bg: 'bg-yellow-500/20',
      border: 'border-yellow-500/50',
      text: 'text-yellow-300',
      hoverBg: 'hover:bg-yellow-500/30',
      shadowColor: 'yellow'
    },
    special: {
      bg: 'bg-purple-500/20',
      border: 'border-purple-500/50',
      text: 'text-purple-300',
      hoverBg: 'hover:bg-purple-500/30',
      shadowColor: 'purple'
    }
  }

  const sizeStyles = {
    medium: {
      padding: 'px-6 py-3',
      fontSize: 'text-lg',
      bulletSize: 'text-sm',
      gap: 'gap-3'
    },
    large: {
      padding: 'px-8 py-4',
      fontSize: 'text-xl',
      bulletSize: 'text-base',
      gap: 'gap-4'
    },
    huge: {
      padding: 'px-10 py-6',
      fontSize: 'text-2xl',
      bulletSize: 'text-lg',
      gap: 'gap-5'
    }
  }

  const glowStyles = {
    low: '10px',
    medium: '20px',
    high: '40px'
  }

  const currentVariant = variantStyles[variant]
  const currentSize = sizeStyles[size]
  const glowSize = glowStyles[glowIntensity]

  const buttonContent = (
    <button
      onClick={onClick}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      className={cx(
        'relative group transition-all duration-300',
        currentSize.padding,
        currentVariant.bg,
        currentVariant.border,
        currentVariant.hoverBg,
        'border-2 rounded-lg',
        pulseAnimation && 'animate-pulse',
        className
      )}
      style={{
        boxShadow: isHovered 
          ? `0 0 ${glowSize} ${currentVariant.shadowColor === 'cyan' ? '#00ffff' : currentVariant.shadowColor === 'yellow' ? '#ffff00' : '#ff00ff'}80`
          : `0 0 ${parseInt(glowSize) / 2}px ${currentVariant.shadowColor === 'cyan' ? '#00ffff' : currentVariant.shadowColor === 'yellow' ? '#ffff00' : '#ff00ff'}40`
      }}
    >
      {/* Main Text */}
      <div className={cx('font-bold', currentSize.fontSize, currentVariant.text)}>
        <Text manager={animated ? 'decipher' : undefined}>
          {primaryText}
        </Text>
      </div>

      {/* Countdown */}
      {showCountdown && (
        <div className="mt-2 text-center">
          <Text 
            as="span" 
            className={cx('font-mono opacity-80', currentVariant.text)}
            style={{ fontSize: '0.8em' }}
          >
            {countdownText} {countdown}s
          </Text>
        </div>
      )}

      {/* Benefit Bullets */}
      {benefitBullets.length > 0 && (
        <div className={cx('mt-4 space-y-1', currentSize.bulletSize)}>
          {benefitBullets.map((bullet, index) => (
            <div 
              key={index}
              className={cx('flex items-center', currentSize.gap)}
              style={{ animationDelay: `${index * 100}ms` }}
            >
              <svg 
                className="w-4 h-4 flex-shrink-0" 
                fill="currentColor" 
                viewBox="0 0 20 20"
              >
                <path 
                  fillRule="evenodd" 
                  d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" 
                  clipRule="evenodd" 
                />
              </svg>
              <Text as="span" className="opacity-90">
                {bullet}
              </Text>
            </div>
          ))}
        </div>
      )}

      {/* Corner Frames for extra emphasis */}
      {variant === 'special' && (
        <FrameCorners
          className="absolute inset-0 pointer-events-none"
          style={{
            '--arwes-frames-line-color': 'hsla(300, 75%, 50%, 0.5)'
          } as React.CSSProperties}
        />
      )}

      {/* Hover Effect Overlay */}
      <div 
        className={cx(
          'absolute inset-0 rounded-lg opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none',
          currentVariant.bg
        )}
        style={{
          background: `radial-gradient(circle at center, ${
            currentVariant.shadowColor === 'cyan' ? 'rgba(0, 255, 255, 0.1)' : 
            currentVariant.shadowColor === 'yellow' ? 'rgba(255, 255, 0, 0.1)' : 
            'rgba(255, 0, 255, 0.1)'
          } 0%, transparent 70%)`
        }}
      />
    </button>
  )

  if (!animated) {
    return buttonContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated
          animated={[
            ['opacity', 0, 1],
            ['scale', 0.9, 1]
          ]}
        >
          {buttonContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// GitHub Sign In variant
export const GitHubSignInCTA = ({
  onClick,
  className = ''
}: {
  onClick?: () => void
  className?: string
}) => {
  return (
    <HeroCallToAction
      primaryText="Sign in with GitHub to Start"
      benefitBullets={[
        'No email/password needed',
        'Your code stays private',
        'Cancel anytime'
      ]}
      showCountdown={false}
      variant="default"
      size="large"
      onClick={onClick}
      className={className}
      glowIntensity="high"
    />
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Atoms/HeroCallToAction',
  component: HeroCallToAction,
  parameters: {
    layout: 'centered',
    backgrounds: {
      default: 'dark'
    },
    docs: {
      description: {
        component: 'High-impact call-to-action button optimized for conversion psychology. Features countdown timer, benefit bullets, and attention-grabbing animations. Critical for homepage conversion.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    primaryText: {
      control: 'text',
      description: 'Main CTA text'
    },
    countdownText: {
      control: 'text',
      description: 'Text before countdown timer'
    },
    benefitBullets: {
      control: 'object',
      description: 'List of benefits shown below CTA'
    },
    showCountdown: {
      control: 'boolean',
      description: 'Show countdown timer'
    },
    countdownDuration: {
      control: { type: 'number', min: 10, max: 120, step: 10 },
      description: 'Countdown duration in seconds'
    },
    variant: {
      control: 'select',
      options: ['default', 'urgent', 'special'],
      description: 'Visual variant'
    },
    size: {
      control: 'select',
      options: ['medium', 'large', 'huge'],
      description: 'Button size'
    },
    pulseAnimation: {
      control: 'boolean',
      description: 'Enable pulse animation'
    },
    glowIntensity: {
      control: 'select',
      options: ['low', 'medium', 'high'],
      description: 'Glow effect intensity'
    }
  }
} satisfies Meta<typeof HeroCallToAction>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const Urgent: Story = {
  args: {
    variant: 'urgent',
    primaryText: 'Limited Time: Build Free Today',
    countdownDuration: 30,
    glowIntensity: 'high'
  }
}

export const Special: Story = {
  args: {
    variant: 'special',
    primaryText: 'Exclusive Launch Access',
    benefitBullets: [
      'Be among the first 1000 users',
      'Lifetime benefits',
      'Priority support'
    ],
    glowIntensity: 'high'
  }
}

export const GitHubSignIn: Story = {
  render: () => <GitHubSignInCTA onClick={() => console.log('Sign in clicked')} />
}

export const NoCountdown: Story = {
  args: {
    showCountdown: false,
    primaryText: 'Start Building Now'
  }
}

export const HugeSize: Story = {
  args: {
    size: 'huge',
    primaryText: 'Deploy Your First App',
    glowIntensity: 'high'
  }
}

export const MinimalBenefits: Story = {
  args: {
    benefitBullets: ['Free forever'],
    showCountdown: false,
    size: 'medium'
  }
}

export const WithCallback: Story = {
  render: () => {
    const [status, setStatus] = useState('Waiting for countdown...')
    
    return (
      <div className="space-y-6">
        <HeroCallToAction
          onClick={() => setStatus('Button clicked!')}
          onCountdownComplete={() => setStatus('Countdown completed!')}
          countdownDuration={10}
        />
        <p className="text-cyan-300">{status}</p>
      </div>
    )
  }
}

export const MultipleVariants: Story = {
  render: () => (
    <div className="space-y-6">
      <HeroCallToAction variant="default" />
      <HeroCallToAction variant="urgent" primaryText="Last Chance!" />
      <HeroCallToAction variant="special" primaryText="VIP Access" />
    </div>
  )
}

export const NoPulse: Story = {
  args: {
    pulseAnimation: false,
    glowIntensity: 'low'
  }
}

export const Playground: Story = {
  args: {
    primaryText: 'Build Yours in 60 Seconds',
    countdownText: 'Start building in',
    benefitBullets: [
      '1000 free AI operations',
      'Deploy to global edge network', 
      'No credit card required'
    ],
    showCountdown: true,
    countdownDuration: 60,
    variant: 'default',
    size: 'large',
    pulseAnimation: true,
    glowIntensity: 'medium'
  }
}