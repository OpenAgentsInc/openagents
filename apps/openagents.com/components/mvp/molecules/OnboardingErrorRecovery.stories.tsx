import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx, FrameKranox } from '@arwes/react'

// Types
export interface ErrorRecoveryOption {
  id: string
  title: string
  description: string
  action: string
  icon: React.ReactNode
  recommended?: boolean
}

export interface OnboardingErrorRecoveryProps {
  errorType?: 'generation' | 'deployment' | 'network' | 'quota' | 'unknown'
  errorMessage?: string
  remainingQuota?: number
  resetTime?: Date
  fallbackOptions?: ErrorRecoveryOption[]
  onRetry?: () => void
  onFallback?: (option: ErrorRecoveryOption) => void
  onContactSupport?: () => void
  showQuotaInfo?: boolean
  animated?: boolean
  className?: string
}

// Default fallback options
const getDefaultFallbacks = (errorType: string): ErrorRecoveryOption[] => {
  const baseOptions: ErrorRecoveryOption[] = [
    {
      id: 'template',
      title: 'Try a Template Instead',
      description: 'Deploy a pre-built template that\'s guaranteed to work',
      action: 'Browse Templates',
      icon: (
        <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
        </svg>
      ),
      recommended: true
    },
    {
      id: 'simpler',
      title: 'Try a Simpler Prompt',
      description: 'Sometimes less complex requests work better',
      action: 'Simplify & Retry',
      icon: (
        <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
        </svg>
      )
    },
    {
      id: 'support',
      title: 'Get Help',
      description: 'Our team is here to help you succeed',
      action: 'Contact Support',
      icon: (
        <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M18.364 5.636l-3.536 3.536m0 5.656l3.536 3.536M9.172 9.172L5.636 5.636m3.536 9.192l-3.536 3.536M21 12a9 9 0 11-18 0 9 9 0 0118 0zm-5 0a4 4 0 11-8 0 4 4 0 018 0z" />
        </svg>
      )
    }
  ]

  if (errorType === 'generation') {
    return [
      baseOptions[0], // Template
      baseOptions[1], // Simpler
      {
        id: 'examples',
        title: 'View Example Prompts',
        description: 'See what prompts work best',
        action: 'Show Examples',
        icon: (
          <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
          </svg>
        )
      }
    ]
  }

  return baseOptions
}

// Encouraging messages based on error type
const getEncouragingMessage = (errorType: string): string => {
  const messages: Record<string, string> = {
    generation: "Don't worry! The AI sometimes needs a clearer prompt. Let's try a different approach.",
    deployment: "Deployment hiccups happen to the best of us. We've got backup options!",
    network: "Looks like a connection issue. These usually resolve quickly.",
    quota: "You've been building amazing things! Let's explore other options.",
    unknown: "Something unexpected happened, but we'll get you building in no time!"
  }
  return messages[errorType] || messages.unknown
}

export const OnboardingErrorRecovery = ({
  errorType = 'generation',
  errorMessage = 'The AI encountered an issue generating your code',
  remainingQuota = 1000,
  resetTime = new Date(Date.now() + 24 * 60 * 60 * 1000),
  fallbackOptions = getDefaultFallbacks(errorType),
  onRetry,
  onFallback,
  onContactSupport,
  showQuotaInfo = true,
  animated = true,
  className = ''
}: OnboardingErrorRecoveryProps) => {
  const [active, setActive] = useState(false)
  const [selectedOption, setSelectedOption] = useState<string>()

  React.useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  const handleOptionClick = (option: ErrorRecoveryOption) => {
    setSelectedOption(option.id)
    if (option.id === 'support') {
      onContactSupport?.()
    } else {
      onFallback?.(option)
    }
  }

  const formatResetTime = (date: Date): string => {
    const hours = Math.floor((date.getTime() - Date.now()) / (1000 * 60 * 60))
    if (hours < 1) return 'less than an hour'
    if (hours === 1) return '1 hour'
    if (hours < 24) return `${hours} hours`
    return `${Math.floor(hours / 24)} days`
  }

  const content = (
    <div className={cx('space-y-6', className)}>
      {/* Error Header */}
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', -20, 0]]}>
          <div className="text-center space-y-3">
            <div className="inline-flex items-center justify-center w-16 h-16 bg-yellow-500/20 border border-yellow-500/50 rounded-full">
              <svg className="w-8 h-8 text-yellow-400" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
              </svg>
            </div>
            
            <div>
              <Text as="h2" className="text-2xl font-bold text-white mb-2">
                Oops! Let's Try Something Else
              </Text>
              <Text as="p" className="text-lg text-gray-300">
                {getEncouragingMessage(errorType)}
              </Text>
            </div>
          </div>
        </Animated>
      </Animator>

      {/* Error Details */}
      <Animator active={active} duration={{ delay: 0.2 }}>
        <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
          <div className="relative">
            <FrameKranox
              style={{
                '--arwes-frames-bg-color': 'hsla(0, 50%, 20%, 0.3)',
                '--arwes-frames-line-color': 'hsla(0, 50%, 50%, 0.5)'
              } as React.CSSProperties}
            />
            <div className="relative p-4">
              <div className="flex items-start gap-3">
              <svg className="w-5 h-5 text-red-400 mt-0.5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
              </svg>
              <div className="flex-1">
                <Text as="p" className="text-red-300 font-medium mb-1">
                  Error Details
                </Text>
                <Text as="p" className="text-gray-400 text-sm">
                  {errorMessage}
                </Text>
              </div>
              </div>
            </div>
          </div>
        </Animated>
      </Animator>

      {/* Quota Info */}
      {showQuotaInfo && remainingQuota > 0 && (
        <Animator active={active} duration={{ delay: 0.3 }}>
          <Animated animated={[['opacity', 0, 1]]}>
            <div className="bg-cyan-500/10 border border-cyan-500/30 rounded-lg p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <svg className="w-5 h-5 text-cyan-400" fill="currentColor" viewBox="0 0 20 20">
                    <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clipRule="evenodd" />
                  </svg>
                  <Text as="span" className="text-cyan-300 font-medium">
                    Good news! You still have {remainingQuota} operations available
                  </Text>
                </div>
                <Text as="span" className="text-gray-400 text-sm">
                  Resets in {formatResetTime(resetTime)}
                </Text>
              </div>
            </div>
          </Animated>
        </Animator>
      )}

      {/* Fallback Options */}
      <div className="space-y-4">
        <Text as="h3" className="text-lg font-bold text-white text-center">
          Here's what you can do:
        </Text>
        
        <div className="grid gap-3">
          {fallbackOptions.map((option, index) => (
            <Animator key={option.id} active={active} duration={{ delay: 0.4 + index * 0.1 }}>
              <Animated animated={[['opacity', 0, 1], ['x', -20, 0]]}>
                <button
                  onClick={() => handleOptionClick(option)}
                  className={cx(
                    'w-full p-4 border rounded-lg transition-all duration-300 group',
                    'hover:shadow-lg',
                    selectedOption === option.id
                      ? 'bg-cyan-500/20 border-cyan-500/50'
                      : 'bg-gray-900/30 border-gray-700/50 hover:bg-gray-800/50 hover:border-cyan-500/30'
                  )}
                >
                  <div className="flex items-start gap-4">
                    <div className={cx(
                      'p-2 rounded-lg transition-colors',
                      option.recommended ? 'bg-cyan-500/20 text-cyan-400' : 'bg-gray-800 text-gray-400',
                      'group-hover:text-cyan-300'
                    )}>
                      {option.icon}
                    </div>
                    <div className="flex-1 text-left">
                      <div className="flex items-center gap-2 mb-1">
                        <Text as="h4" className="font-bold text-white">
                          {option.title}
                        </Text>
                        {option.recommended && (
                          <span className="px-2 py-1 bg-cyan-500/20 text-cyan-300 text-xs rounded-full font-medium">
                            RECOMMENDED
                          </span>
                        )}
                      </div>
                      <Text as="p" className="text-gray-400 text-sm mb-2">
                        {option.description}
                      </Text>
                      <Text as="span" className="text-cyan-400 text-sm font-medium group-hover:underline">
                        {option.action} →
                      </Text>
                    </div>
                  </div>
                </button>
              </Animated>
            </Animator>
          ))}
        </div>
      </div>

      {/* Retry Button */}
      {onRetry && (
        <Animator active={active} duration={{ delay: 0.6 }}>
          <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
            <div className="text-center pt-4">
              <button
                onClick={onRetry}
                className="inline-flex items-center gap-2 px-6 py-2 text-gray-400 hover:text-white transition-colors"
              >
                <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                Try Again with Same Prompt
              </button>
            </div>
          </Animated>
        </Animator>
      )}
    </div>
  )

  if (!animated) return content

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      {content}
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/OnboardingErrorRecovery',
  component: OnboardingErrorRecovery,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Specialized error recovery component for onboarding flows. Maintains user confidence and momentum by providing clear fallback options when things go wrong.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    errorType: {
      control: 'select',
      options: ['generation', 'deployment', 'network', 'quota', 'unknown'],
      description: 'Type of error encountered'
    },
    errorMessage: {
      control: 'text',
      description: 'Detailed error message'
    },
    remainingQuota: {
      control: { type: 'number', min: 0, max: 1000 },
      description: 'Remaining AI operations'
    },
    showQuotaInfo: {
      control: 'boolean',
      description: 'Show quota information'
    }
  }
} satisfies Meta<typeof OnboardingErrorRecovery>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const DeploymentError: Story = {
  args: {
    errorType: 'deployment',
    errorMessage: 'Failed to build container: dependency resolution failed'
  }
}

export const NetworkError: Story = {
  args: {
    errorType: 'network',
    errorMessage: 'Connection to Cloudflare API timed out',
    fallbackOptions: [
      {
        id: 'retry',
        title: 'Retry Connection',
        description: 'Try connecting again',
        action: 'Retry Now',
        icon: <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" /></svg>,
        recommended: true
      },
      {
        id: 'status',
        title: 'Check Service Status',
        description: 'View platform status page',
        action: 'View Status',
        icon: <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" /></svg>
      }
    ]
  }
}

export const QuotaError: Story = {
  args: {
    errorType: 'quota',
    errorMessage: 'You\'ve used all 1000 free operations this month',
    remainingQuota: 0,
    showQuotaInfo: false,
    fallbackOptions: [
      {
        id: 'upgrade',
        title: 'Upgrade to Pro',
        description: 'Get 10,000 operations per month',
        action: 'View Plans',
        icon: <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>,
        recommended: true
      },
      {
        id: 'wait',
        title: 'Wait for Reset',
        description: 'Your quota resets tomorrow',
        action: 'Set Reminder',
        icon: <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
      }
    ]
  }
}

export const LowQuota: Story = {
  args: {
    remainingQuota: 50,
    errorMessage: 'Generation failed due to complexity. Consider simplifying your request.'
  }
}

export const WithCallbacks: Story = {
  render: () => {
    const [action, setAction] = useState<string>()
    
    return (
      <div className="space-y-6">
        <OnboardingErrorRecovery
          onRetry={() => setAction('Retry clicked')}
          onFallback={(option) => setAction(`Selected: ${option.title}`)}
          onContactSupport={() => setAction('Contact support clicked')}
        />
        {action && (
          <div className="text-center">
            <Text as="p" className="text-cyan-300">
              {action}
            </Text>
          </div>
        )}
      </div>
    )
  }
}

export const NoAnimation: Story = {
  args: {
    animated: false
  }
}

export const CustomError: Story = {
  args: {
    errorType: 'unknown',
    errorMessage: 'An unexpected error occurred while processing your request',
    fallbackOptions: [
      {
        id: 'reload',
        title: 'Reload Page',
        description: 'Start fresh with a page reload',
        action: 'Reload',
        icon: <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" /></svg>
      }
    ]
  }
}

export const Playground: Story = {
  args: {
    errorType: 'generation',
    errorMessage: 'The AI encountered an issue generating your code',
    remainingQuota: 1000,
    showQuotaInfo: true
  }
}