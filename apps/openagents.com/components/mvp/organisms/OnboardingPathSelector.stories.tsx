import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx, FrameKranox, FrameCorners } from '@arwes/react'

// Types
export interface PathOption {
  id: 'template' | 'chat' | string
  title: string
  subtitle: string
  description: string
  features: string[]
  estimatedTime: string
  difficulty: 'beginner' | 'intermediate' | 'advanced'
  icon: React.ReactNode
  previewContent?: React.ReactNode
  recommended?: boolean
}

export interface OnboardingPathSelectorProps {
  paths?: PathOption[]
  onPathSelect?: (path: PathOption) => void
  selectedPath?: string
  userName?: string
  showPreview?: boolean
  animated?: boolean
  className?: string
}

// Default path options
const defaultPaths: PathOption[] = [
  {
    id: 'template',
    title: 'Start with a Template',
    subtitle: 'Deploy a proven app in 30 seconds',
    description: 'Choose from our curated collection of templates and deploy instantly. Perfect for seeing immediate results.',
    features: [
      'Guaranteed to work',
      'Deploy in 30 seconds',
      'Customize after deployment',
      'Learn by example'
    ],
    estimatedTime: '30 seconds',
    difficulty: 'beginner',
    recommended: true,
    icon: (
      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
      </svg>
    ),
    previewContent: (
      <div className="space-y-2">
        <div className="bg-gray-800/50 border border-gray-700 rounded p-3">
          <p className="text-cyan-300 text-sm mb-1">Bitcoin Price Tracker</p>
          <p className="text-gray-400 text-xs">Real-time crypto dashboard • 30s deploy</p>
        </div>
        <div className="bg-gray-800/50 border border-gray-700 rounded p-3">
          <p className="text-cyan-300 text-sm mb-1">Task Manager</p>
          <p className="text-gray-400 text-xs">Drag & drop todo app • 45s deploy</p>
        </div>
      </div>
    )
  },
  {
    id: 'chat',
    title: 'Build from Scratch',
    subtitle: 'Describe what you want to build',
    description: 'Tell our AI what you want to create and watch it generate custom code just for you. More flexible but requires clear ideas.',
    features: [
      'Completely custom',
      'AI generates unique code',
      'Learn AI prompting',
      'Unlimited possibilities'
    ],
    estimatedTime: '60 seconds',
    difficulty: 'intermediate',
    icon: (
      <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z" />
      </svg>
    ),
    previewContent: (
      <div className="space-y-2">
        <div className="bg-gray-900/50 border border-gray-700 rounded p-3">
          <p className="text-gray-400 text-xs mb-1">You:</p>
          <p className="text-cyan-300 text-sm">"Create a Bitcoin puns website"</p>
        </div>
        <div className="bg-gray-900/50 border border-gray-700 rounded p-3">
          <p className="text-gray-400 text-xs mb-1">AI:</p>
          <p className="text-green-300 text-sm">I'll create a fun Bitcoin puns site...</p>
        </div>
      </div>
    )
  }
]

// Path card component
const PathCard = ({
  path,
  isSelected,
  onClick,
  showPreview,
  index
}: {
  path: PathOption
  isSelected: boolean
  onClick: () => void
  showPreview: boolean
  index: number
}) => {
  const [isHovered, setIsHovered] = useState(false)

  const difficultyColors = {
    beginner: 'text-green-400',
    intermediate: 'text-yellow-400',
    advanced: 'text-red-400'
  }

  return (
    <Animator active={true} duration={{ enter: 0.5, delay: index * 0.2 }}>
      <Animated animated={[['opacity', 0, 1], ['y', 30, 0]]}>
        <div
          onClick={onClick}
          onMouseEnter={() => setIsHovered(true)}
          onMouseLeave={() => setIsHovered(false)}
          className={cx(
            'relative cursor-pointer transition-all duration-300',
            isSelected && 'scale-105'
          )}
        >
          <div className="relative h-full">
            <FrameKranox
              className={cx(
                isHovered && 'shadow-lg'
              )}
              style={{
                '--arwes-frames-bg-color': isSelected 
                  ? 'hsla(180, 75%, 10%, 0.8)' 
                  : 'hsla(180, 75%, 10%, 0.4)',
                '--arwes-frames-line-color': isSelected 
                  ? 'hsla(180, 75%, 50%, 1)' 
                  : 'hsla(180, 75%, 50%, 0.5)',
                '--arwes-frames-glow': isHovered ? 'true' : 'false',
                '--arwes-frames-glow-size': '20px'
              } as React.CSSProperties}
            />
            <div className="relative p-6 h-full">
            {/* Recommended badge */}
            {path.recommended && (
              <div className="absolute -top-3 left-6 px-3 py-1 bg-cyan-500/20 border border-cyan-500/50 rounded-full">
                <Text as="span" className="text-cyan-300 text-xs font-bold">
                  RECOMMENDED
                </Text>
              </div>
            )}

            {/* Header */}
            <div className="flex items-start gap-4 mb-4">
              <div className="text-cyan-400">
                {path.icon}
              </div>
              <div className="flex-1">
                <Text as="h3" className="text-xl font-bold text-white mb-1">
                  {path.title}
                </Text>
                <Text as="p" className="text-gray-400">
                  {path.subtitle}
                </Text>
              </div>
            </div>

            {/* Description */}
            <Text as="p" className="text-gray-300 mb-4">
              {path.description}
            </Text>

            {/* Features */}
            <div className="space-y-2 mb-4">
              {path.features.map((feature, i) => (
                <div key={i} className="flex items-center gap-2">
                  <svg className="w-4 h-4 text-cyan-400 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
                    <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                  </svg>
                  <Text as="span" className="text-sm text-gray-300">
                    {feature}
                  </Text>
                </div>
              ))}
            </div>

            {/* Metadata */}
            <div className="flex items-center justify-between pt-4 border-t border-gray-700/50">
              <div className="flex items-center gap-4">
                <div className="flex items-center gap-2">
                  <svg className="w-4 h-4 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <Text as="span" className="text-sm text-gray-400">
                    {path.estimatedTime}
                  </Text>
                </div>
                <div className="flex items-center gap-2">
                  <div className={cx('w-2 h-2 rounded-full', 
                    path.difficulty === 'beginner' && 'bg-green-400',
                    path.difficulty === 'intermediate' && 'bg-yellow-400',
                    path.difficulty === 'advanced' && 'bg-red-400'
                  )} />
                  <Text as="span" className={cx('text-sm capitalize', difficultyColors[path.difficulty])}>
                    {path.difficulty}
                  </Text>
                </div>
              </div>
              
              <div className={cx(
                'transition-all duration-300',
                isSelected ? 'opacity-100 scale-100' : 'opacity-0 scale-90'
              )}>
                <svg className="w-6 h-6 text-cyan-400" fill="currentColor" viewBox="0 0 20 20">
                  <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clipRule="evenodd" />
                </svg>
              </div>
            </div>

            {/* Preview on hover */}
            {showPreview && path.previewContent && isHovered && (
              <Animator active={true}>
                <Animated 
                  animated={[['opacity', 0, 1], ['y', 10, 0]]}
                  className="absolute left-0 right-0 top-full mt-4 z-10"
                >
                  <div className="relative bg-gray-900/95">
                    <FrameKranox />
                    <div className="relative p-4">
                      <Text as="p" className="text-cyan-300 text-sm mb-3 font-bold">
                        Preview
                      </Text>
                      {path.previewContent}
                    </div>
                  </div>
                </Animated>
              </Animator>
            )}
            </div>
          </div>
        </div>
      </Animated>
    </Animator>
  )
}

export const OnboardingPathSelector = ({
  paths = defaultPaths,
  onPathSelect,
  selectedPath,
  userName = 'developer',
  showPreview = true,
  animated = true,
  className = ''
}: OnboardingPathSelectorProps) => {
  const [localSelectedPath, setLocalSelectedPath] = useState(selectedPath)
  const [hasSelected, setHasSelected] = useState(false)

  const handleSelect = (path: PathOption) => {
    setLocalSelectedPath(path.id)
    setHasSelected(true)
    onPathSelect?.(path)
  }

  const content = (
    <div className={cx('space-y-8', className)}>
      {/* Header */}
      <Animator active={true}>
        <Animated animated={[['opacity', 0, 1], ['y', -20, 0]]}>
          <div className="text-center max-w-2xl mx-auto">
            <Text as="h2" className="text-3xl font-bold text-white mb-2">
              Welcome back, @{userName}!
            </Text>
            <Text as="p" className="text-xl text-gray-400">
              Let's build your first app. Choose your adventure:
            </Text>
          </div>
        </Animated>
      </Animator>

      {/* Path Options */}
      <div className="grid md:grid-cols-2 gap-6 max-w-4xl mx-auto">
        {paths.map((path, index) => (
          <PathCard
            key={path.id}
            path={path}
            isSelected={localSelectedPath === path.id}
            onClick={() => handleSelect(path)}
            showPreview={showPreview}
            index={index}
          />
        ))}
      </div>

      {/* Action Button */}
      {hasSelected && (
        <Animator active={true}>
          <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
            <div className="text-center">
              <button className="px-8 py-3 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded-lg hover:bg-cyan-500/30 transition-all duration-300 font-bold text-lg">
                Continue with {paths.find(p => p.id === localSelectedPath)?.title} →
              </button>
            </div>
          </Animated>
        </Animator>
      )}

      {/* Footer */}
      <Animator active={true}>
        <Animated 
          animated={[['opacity', 0, 0.8]]}
          className="text-center"
        >
          <Text as="p" className="text-gray-500 text-sm">
            🎯 You have 1000 free operations • No credit card required • Cancel anytime
          </Text>
        </Animated>
      </Animator>
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
  title: 'MVP/Organisms/OnboardingPathSelector',
  component: OnboardingPathSelector,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Post-authentication path selection component that guides users to choose between template deployment or AI chat. Designed to eliminate choice paralysis and ensure first success.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    userName: {
      control: 'text',
      description: 'GitHub username to display'
    },
    showPreview: {
      control: 'boolean',
      description: 'Show preview content on hover'
    },
    selectedPath: {
      control: 'select',
      options: ['template', 'chat', undefined],
      description: 'Pre-selected path'
    }
  }
} satisfies Meta<typeof OnboardingPathSelector>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const PreSelected: Story = {
  args: {
    selectedPath: 'template'
  }
}

export const NoPreview: Story = {
  args: {
    showPreview: false
  }
}

export const CustomUserName: Story = {
  args: {
    userName: 'atlantispleb'
  }
}

export const WithCallback: Story = {
  render: () => {
    const [selected, setSelected] = useState<string>()
    
    return (
      <div className="space-y-6">
        <OnboardingPathSelector
          onPathSelect={(path) => setSelected(path.id)}
        />
        {selected && (
          <div className="text-center">
            <Text as="p" className="text-cyan-300">
              Selected: {selected}
            </Text>
          </div>
        )}
      </div>
    )
  }
}

export const SinglePath: Story = {
  args: {
    paths: [defaultPaths[0]]
  }
}

export const CustomPaths: Story = {
  args: {
    paths: [
      {
        id: 'import',
        title: 'Import from GitHub',
        subtitle: 'Deploy existing repository',
        description: 'Connect your GitHub repo and deploy it instantly to our global edge network.',
        features: [
          'Works with any framework',
          'Automatic build detection',
          'Environment variables',
          'Custom domains'
        ],
        estimatedTime: '2 minutes',
        difficulty: 'advanced',
        icon: (
          <svg className="w-8 h-8" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
          </svg>
        )
      },
      {
        id: 'upload',
        title: 'Upload Files',
        subtitle: 'Drag and drop your code',
        description: 'Upload your project files directly and we\'ll handle the deployment automatically.',
        features: [
          'Support for all file types',
          'Automatic dependency detection',
          'Preview before deploy',
          'Instant hosting'
        ],
        estimatedTime: '90 seconds',
        difficulty: 'intermediate',
        icon: (
          <svg className="w-8 h-8" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
          </svg>
        )
      }
    ]
  }
}

export const NoAnimation: Story = {
  args: {
    animated: false
  }
}

export const Playground: Story = {
  args: {
    userName: 'developer',
    showPreview: true,
    animated: true
  }
}