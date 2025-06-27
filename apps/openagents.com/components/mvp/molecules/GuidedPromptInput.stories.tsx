import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect, useRef } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'

// Types
export interface PromptSuggestion {
  id: string
  text: string
  category: 'app' | 'game' | 'tool' | 'website' | 'api'
  difficulty: 'easy' | 'medium' | 'hard'
  estimatedTime: string
  popularity?: number
}

export interface GuidedPromptInputProps {
  value?: string
  onChange?: (value: string) => void
  onSubmit?: (value: string) => void
  suggestions?: PromptSuggestion[]
  placeholder?: string
  maxLength?: number
  showCharacterCount?: boolean
  showSuggestions?: boolean
  showDifficultyIndicator?: boolean
  showContextHints?: boolean
  loading?: boolean
  error?: string
  animated?: boolean
  className?: string
}

// Default suggestions based on successful patterns
const defaultSuggestions: PromptSuggestion[] = [
  {
    id: '1',
    text: 'Create a Bitcoin price tracker with live charts',
    category: 'app',
    difficulty: 'easy',
    estimatedTime: '30 seconds',
    popularity: 147
  },
  {
    id: '2',
    text: 'Build a todo app with drag and drop functionality',
    category: 'app',
    difficulty: 'medium',
    estimatedTime: '45 seconds',
    popularity: 89
  },
  {
    id: '3',
    text: 'Make a weather dashboard with 5-day forecast',
    category: 'app',
    difficulty: 'easy',
    estimatedTime: '30 seconds',
    popularity: 76
  },
  {
    id: '4',
    text: 'Create a markdown editor with live preview',
    category: 'tool',
    difficulty: 'medium',
    estimatedTime: '45 seconds',
    popularity: 63
  },
  {
    id: '5',
    text: 'Build a memory card game with animations',
    category: 'game',
    difficulty: 'medium',
    estimatedTime: '60 seconds',
    popularity: 52
  },
  {
    id: '6',
    text: 'Design a landing page for a startup',
    category: 'website',
    difficulty: 'easy',
    estimatedTime: '30 seconds',
    popularity: 94
  }
]

// Context hint component
const ContextHint = ({ text, type }: { text: string, type: 'tip' | 'warning' | 'success' }) => {
  const colors = {
    tip: 'text-cyan-400 border-cyan-500/30 bg-cyan-500/10',
    warning: 'text-yellow-400 border-yellow-500/30 bg-yellow-500/10',
    success: 'text-green-400 border-green-500/30 bg-green-500/10'
  }

  const icons = {
    tip: (
      <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
        <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clipRule="evenodd" />
      </svg>
    ),
    warning: (
      <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
        <path fillRule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
      </svg>
    ),
    success: (
      <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
        <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
      </svg>
    )
  }

  return (
    <div className={cx('flex items-center gap-2 px-3 py-2 border rounded text-sm', colors[type])}>
      {icons[type]}
      <Text as="span">{text}</Text>
    </div>
  )
}

// Category icon
const CategoryIcon = ({ category }: { category: string }) => {
  switch (category) {
    case 'app':
      return <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"><path fillRule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 011 1v2a1 1 0 01-1 1H4a1 1 0 01-1-1V4zM3 10a1 1 0 011-1h6a1 1 0 011 1v6a1 1 0 01-1 1H4a1 1 0 01-1-1v-6zM14 9a1 1 0 00-1 1v6a1 1 0 001 1h2a1 1 0 001-1v-6a1 1 0 00-1-1h-2z" clipRule="evenodd" /></svg>
    case 'game':
      return <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"><path d="M10.394 2.08a1 1 0 00-.788 0l-7 3a1 1 0 000 1.84L5.25 8.051a.999.999 0 01.356-.257l4-1.714a1 1 0 11.788 1.838L7.667 9.088l1.94.831a1 1 0 00.787 0l7-3a1 1 0 000-1.838l-7-3zM3.31 9.397L5 10.12v4.102a8.969 8.969 0 00-1.05-.174 1 1 0 01-.89-.89 11.115 11.115 0 01.25-3.762zM9.3 16.573A9.026 9.026 0 007 14.935v-3.957l1.818.78a3 3 0 002.364 0l5.508-2.361a11.026 11.026 0 01.25 3.762 1 1 0 01-.89.89 8.968 8.968 0 00-5.35 2.524 1 1 0 01-1.4 0zM6 18a1 1 0 001-1v-2.065a8.935 8.935 0 00-2-.712V17a1 1 0 001 1z" /></svg>
    case 'tool':
      return <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"><path fillRule="evenodd" d="M11.49 3.17c-.38-1.56-2.6-1.56-2.98 0a1.532 1.532 0 01-2.286.948c-1.372-.836-2.942.734-2.106 2.106.54.886.061 2.042-.947 2.287-1.561.379-1.561 2.6 0 2.978a1.532 1.532 0 01.947 2.287c-.836 1.372.734 2.942 2.106 2.106a1.532 1.532 0 012.287.947c.379 1.561 2.6 1.561 2.978 0a1.533 1.533 0 012.287-.947c1.372.836 2.942-.734 2.106-2.106a1.533 1.533 0 01.947-2.287c1.561-.379 1.561-2.6 0-2.978a1.532 1.532 0 01-.947-2.287c.836-1.372-.734-2.942-2.106-2.106a1.532 1.532 0 01-2.287-.947zM10 13a3 3 0 100-6 3 3 0 000 6z" clipRule="evenodd" /></svg>
    case 'website':
      return <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"><path fillRule="evenodd" d="M4.083 9h1.946c.089-1.546.383-2.97.837-4.137A6.004 6.004 0 004.083 9zM10 2a8 8 0 100 16 8 8 0 000-16zM9.954 4.569C9.548 5.718 9.223 7.195 9.123 9H7.037c.225-2.053.961-3.74 1.931-4.846a7.96 7.96 0 01.986-.585zM11.963 9c-.1-1.805-.424-3.282-.83-4.431.312.158.61.341.893.548.97 1.107 1.706 2.793 1.931 4.846h-2.086-.001zm2.003 2H12.02c-.089 1.546-.383 2.97-.837 4.136A6.004 6.004 0 0013.917 11h.083zm-2.92 0h2.087c-.225 2.053-.961 3.74-1.931 4.846a7.967 7.967 0 01-.986.585c.406-1.149.73-2.626.83-4.431zm-1.046 0c-.1 1.805-.424 3.282-.83 4.431A7.96 7.96 0 018.184 15.846C7.214 14.74 6.478 13.053 6.253 11H9v.001H9.954zM4.083 11a6.004 6.004 0 002.783 4.136C6.412 13.97 6.116 12.546 6.027 11H4.083z" clipRule="evenodd" /></svg>
    case 'api':
      return <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"><path fillRule="evenodd" d="M12.316 3.051a1 1 0 01.633 1.265l-4 12a1 1 0 11-1.898-.632l4-12a1 1 0 011.265-.633zM5.707 6.293a1 1 0 010 1.414L3.414 10l2.293 2.293a1 1 0 11-1.414 1.414l-3-3a1 1 0 010-1.414l3-3a1 1 0 011.414 0zm8.586 0a1 1 0 011.414 0l3 3a1 1 0 010 1.414l-3 3a1 1 0 11-1.414-1.414L16.586 10l-2.293-2.293a1 1 0 010-1.414z" clipRule="evenodd" /></svg>
    default:
      return null
  }
}

export const GuidedPromptInput = ({
  value = '',
  onChange,
  onSubmit,
  suggestions = defaultSuggestions,
  placeholder = 'Describe what you want to build...',
  maxLength = 500,
  showCharacterCount = true,
  showSuggestions = true,
  showDifficultyIndicator = true,
  showContextHints = true,
  loading = false,
  error,
  animated = true,
  className = ''
}: GuidedPromptInputProps) => {
  const [localValue, setLocalValue] = useState(value)
  const [active, setActive] = useState(false)
  const [isFocused, setIsFocused] = useState(false)
  const [selectedSuggestion, setSelectedSuggestion] = useState<string>()
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  useEffect(() => {
    setLocalValue(value)
  }, [value])

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = e.target.value
    if (newValue.length <= maxLength) {
      setLocalValue(newValue)
      onChange?.(newValue)
    }
  }

  const handleSubmit = (e?: React.FormEvent) => {
    e?.preventDefault()
    if (localValue.trim() && !loading) {
      onSubmit?.(localValue.trim())
    }
  }

  const handleSuggestionClick = (suggestion: PromptSuggestion) => {
    setLocalValue(suggestion.text)
    onChange?.(suggestion.text)
    setSelectedSuggestion(suggestion.id)
    textareaRef.current?.focus()
  }

  const getContextHint = () => {
    const length = localValue.length
    if (length === 0) return null
    if (length < 20) return { text: 'Be more specific for better results', type: 'warning' as const }
    if (length > 200) return { text: 'Great detail! The AI will understand your vision', type: 'success' as const }
    return { text: 'Good prompt! Add features for more customization', type: 'tip' as const }
  }

  const characterPercentage = (localValue.length / maxLength) * 100
  const characterColor = characterPercentage > 90 ? 'text-red-400' : 
                        characterPercentage > 70 ? 'text-yellow-400' : 
                        'text-gray-400'

  const content = (
    <div className={cx('space-y-4', className)}>
      {/* Main Input */}
      <div className="relative">
        <textarea
          ref={textareaRef}
          value={localValue}
          onChange={handleChange}
          onFocus={() => setIsFocused(true)}
          onBlur={() => setIsFocused(false)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && e.metaKey) {
              handleSubmit()
            }
          }}
          placeholder={placeholder}
          disabled={loading}
          className={cx(
            'w-full px-4 py-3 bg-gray-900/50 border rounded-lg resize-none transition-all duration-300',
            'text-white placeholder-gray-500 font-mono',
            'focus:outline-none focus:ring-2 focus:ring-cyan-500/50',
            isFocused ? 'border-cyan-500/50' : 'border-gray-700',
            error && 'border-red-500/50',
            loading && 'opacity-50 cursor-not-allowed'
          )}
          rows={4}
        />
        
        {/* Character Count */}
        {showCharacterCount && (
          <div className={cx('absolute bottom-2 right-2 text-sm', characterColor)}>
            {localValue.length} / {maxLength}
          </div>
        )}
      </div>

      {/* Error Message */}
      {error && (
        <Animator active={true}>
          <Animated animated={[['opacity', 0, 1]]}>
            <div className="text-red-400 text-sm flex items-center gap-2">
              <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
              </svg>
              {error}
            </div>
          </Animated>
        </Animator>
      )}

      {/* Context Hints */}
      {showContextHints && !error && localValue && (
        <Animator active={true}>
          <Animated animated={[['opacity', 0, 1], ['y', -10, 0]]}>
            {(() => {
              const hint = getContextHint()
              return hint ? <ContextHint {...hint} /> : null
            })()}
          </Animated>
        </Animator>
      )}

      {/* Suggestions */}
      {showSuggestions && !localValue && (
        <div className="space-y-3">
          <Text as="h4" className="text-gray-400 text-sm font-bold">
            💡 Popular prompts to get you started:
          </Text>
          <div className="grid gap-2">
            {suggestions.map((suggestion, index) => (
              <Animator key={suggestion.id} active={active} duration={{ delay: index * 0.1 }}>
                <Animated animated={[['opacity', 0, 1], ['x', -20, 0]]}>
                  <button
                    onClick={() => handleSuggestionClick(suggestion)}
                    className={cx(
                      'w-full text-left p-3 bg-gray-900/30 border rounded-lg transition-all duration-300',
                      'hover:bg-gray-800/50 hover:border-cyan-500/50',
                      selectedSuggestion === suggestion.id ? 'border-cyan-500/50 bg-gray-800/50' : 'border-gray-700/50'
                    )}
                  >
                    <div className="flex items-start gap-3">
                      <div className="text-gray-500 mt-0.5">
                        <CategoryIcon category={suggestion.category} />
                      </div>
                      <div className="flex-1">
                        <Text as="p" className="text-gray-200 mb-1">
                          {suggestion.text}
                        </Text>
                        <div className="flex items-center gap-4 text-xs">
                          {showDifficultyIndicator && (
                            <span className={cx(
                              'flex items-center gap-1',
                              suggestion.difficulty === 'easy' && 'text-green-400',
                              suggestion.difficulty === 'medium' && 'text-yellow-400',
                              suggestion.difficulty === 'hard' && 'text-red-400'
                            )}>
                              <div className={cx(
                                'w-1.5 h-1.5 rounded-full',
                                suggestion.difficulty === 'easy' && 'bg-green-400',
                                suggestion.difficulty === 'medium' && 'bg-yellow-400',
                                suggestion.difficulty === 'hard' && 'bg-red-400'
                              )} />
                              {suggestion.difficulty}
                            </span>
                          )}
                          <span className="text-gray-500">
                            ⏱ {suggestion.estimatedTime}
                          </span>
                          {suggestion.popularity && (
                            <span className="text-gray-500">
                              👥 {suggestion.popularity} built this
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                  </button>
                </Animated>
              </Animator>
            ))}
          </div>
        </div>
      )}

      {/* Submit Button */}
      <button
        onClick={() => handleSubmit()}
        disabled={!localValue.trim() || loading}
        className={cx(
          'w-full py-3 px-6 rounded-lg font-bold transition-all duration-300',
          'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50',
          'hover:bg-cyan-500/30 hover:shadow-lg hover:shadow-cyan-500/20',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          'flex items-center justify-center gap-2'
        )}
      >
        {loading ? (
          <>
            <svg className="animate-spin h-5 w-5" fill="none" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
            </svg>
            Generating...
          </>
        ) : (
          <>
            Generate App
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </>
        )}
      </button>

      {/* Keyboard Shortcut Hint */}
      <Text as="p" className="text-center text-gray-500 text-sm">
        Press <kbd className="px-2 py-1 bg-gray-800 border border-gray-700 rounded text-xs">⌘</kbd> + <kbd className="px-2 py-1 bg-gray-800 border border-gray-700 rounded text-xs">Enter</kbd> to generate
      </Text>
    </div>
  )

  if (!animated) return content

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
          {content}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Molecules/GuidedPromptInput',
  component: GuidedPromptInput,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Enhanced chat input with smart prompt suggestions, character counting, and context hints. Guides users to write effective prompts for AI code generation.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    placeholder: {
      control: 'text',
      description: 'Placeholder text'
    },
    maxLength: {
      control: { type: 'number', min: 100, max: 1000 },
      description: 'Maximum character length'
    },
    showCharacterCount: {
      control: 'boolean',
      description: 'Show character counter'
    },
    showSuggestions: {
      control: 'boolean',
      description: 'Show prompt suggestions'
    },
    showDifficultyIndicator: {
      control: 'boolean',
      description: 'Show difficulty indicators on suggestions'
    },
    showContextHints: {
      control: 'boolean',
      description: 'Show context hints based on prompt length'
    },
    loading: {
      control: 'boolean',
      description: 'Loading state'
    },
    error: {
      control: 'text',
      description: 'Error message'
    }
  }
} satisfies Meta<typeof GuidedPromptInput>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const WithValue: Story = {
  args: {
    value: 'Create a Bitcoin price tracker with live updates and charts'
  }
}

export const Loading: Story = {
  args: {
    value: 'Building a weather app...',
    loading: true
  }
}

export const WithError: Story = {
  args: {
    value: 'Make me rich',
    error: 'Please provide more specific details about what you want to build'
  }
}

export const NoSuggestions: Story = {
  args: {
    showSuggestions: false
  }
}

export const MinimalFeatures: Story = {
  args: {
    showCharacterCount: false,
    showSuggestions: false,
    showContextHints: false,
    showDifficultyIndicator: false
  }
}

export const CustomSuggestions: Story = {
  args: {
    suggestions: [
      {
        id: '1',
        text: 'Create a personal portfolio with dark mode',
        category: 'website',
        difficulty: 'easy',
        estimatedTime: '25 seconds',
        popularity: 234
      },
      {
        id: '2',
        text: 'Build a REST API with authentication',
        category: 'api',
        difficulty: 'hard',
        estimatedTime: '90 seconds',
        popularity: 45
      }
    ]
  }
}

export const Interactive: Story = {
  render: () => {
    const [value, setValue] = useState('')
    const [submittedValue, setSubmittedValue] = useState<string>()
    const [loading, setLoading] = useState(false)
    
    const handleSubmit = (prompt: string) => {
      setLoading(true)
      setTimeout(() => {
        setSubmittedValue(prompt)
        setLoading(false)
      }, 2000)
    }
    
    return (
      <div className="space-y-6">
        <GuidedPromptInput
          value={value}
          onChange={setValue}
          onSubmit={handleSubmit}
          loading={loading}
        />
        {submittedValue && (
          <div className="p-4 bg-green-500/10 border border-green-500/50 rounded">
            <Text as="p" className="text-green-300">
              Submitted: "{submittedValue}"
            </Text>
          </div>
        )}
      </div>
    )
  }
}

export const LongText: Story = {
  args: {
    value: 'Create a comprehensive e-commerce platform with user authentication, product catalog with search and filters, shopping cart functionality, checkout process with payment integration, order history, admin dashboard for managing products and orders, email notifications, and responsive design that works on all devices'
  }
}

export const Playground: Story = {
  args: {
    placeholder: 'Describe what you want to build...',
    maxLength: 500,
    showCharacterCount: true,
    showSuggestions: true,
    showDifficultyIndicator: true,
    showContextHints: true
  }
}