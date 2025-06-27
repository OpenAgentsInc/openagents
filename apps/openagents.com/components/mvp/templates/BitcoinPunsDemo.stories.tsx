import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx } from '@arwes/react'
import { ProjectWorkspace } from '../organisms/ProjectWorkspace.stories'

// Icon components
const PlayIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polygon points="5 3 19 12 5 21 5 3" />
  </svg>
)

const BitcoinIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="10" />
    <path d="M8 12h8M8 8h8M8 16h8" />
  </svg>
)

const RocketIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M9 11L7 21L12 17L17 21L15 11M12 2C12 2 17 4 17 11L12 13L7 11C7 4 12 2 12 2Z" />
  </svg>
)

const CheckIcon = ({ className }: { className?: string }) => (
  <svg className={className} width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <polyline points="20 6 9 17 4 12" />
  </svg>
)

// Demo flow stages
interface DemoStage {
  id: string
  title: string
  description: string
  duration: number
  action: string
}

const demoStages: DemoStage[] = [
  {
    id: 'intro',
    title: 'Welcome to OpenAgents',
    description: 'Watch as we build and deploy a Bitcoin puns website from a simple chat message',
    duration: 3000,
    action: 'Starting demo...'
  },
  {
    id: 'chat',
    title: 'Chat to Create',
    description: 'User types: "Build me a Bitcoin puns website that makes people laugh"',
    duration: 4000,
    action: 'Typing message...'
  },
  {
    id: 'understanding',
    title: 'AI Understanding',
    description: 'Claude analyzes the request and plans the implementation',
    duration: 2000,
    action: 'Processing request...'
  },
  {
    id: 'generation',
    title: 'Code Generation',
    description: 'AI generates HTML, CSS, and JavaScript files with Bitcoin puns',
    duration: 8000,
    action: 'Generating code...'
  },
  {
    id: 'deployment',
    title: 'Cloud Deployment',
    description: 'Automatic deployment to Cloudflare Workers edge network',
    duration: 5000,
    action: 'Deploying to cloud...'
  },
  {
    id: 'complete',
    title: 'Live Website',
    description: 'Your Bitcoin puns website is now live and accessible worldwide',
    duration: 3000,
    action: 'Demo complete!'
  }
]

// BitcoinPunsDemo component
export interface BitcoinPunsDemoProps {
  autoStart?: boolean
  showProgress?: boolean
  loopDemo?: boolean
  speed?: 'slow' | 'normal' | 'fast'
  animated?: boolean
  className?: string
  onStageChange?: (stage: DemoStage) => void
  onComplete?: () => void
  onStart?: () => void
}

export const BitcoinPunsDemo = ({
  autoStart = false,
  showProgress = true,
  loopDemo = false,
  speed = 'normal',
  animated = true,
  className = '',
  onStageChange,
  onComplete,
  onStart
}: BitcoinPunsDemoProps) => {
  const [active, setActive] = useState(false)
  const [currentStageIndex, setCurrentStageIndex] = useState(0)
  const [isRunning, setIsRunning] = useState(false)
  const [isComplete, setIsComplete] = useState(false)

  const speedMultipliers = {
    slow: 1.5,
    normal: 1,
    fast: 0.5
  }

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => setActive(true), 200)
      return () => clearTimeout(timer)
    } else {
      setActive(true)
    }
  }, [animated])

  useEffect(() => {
    if (autoStart && !isRunning && !isComplete) {
      startDemo()
    }
  }, [autoStart])

  const startDemo = () => {
    setIsRunning(true)
    setIsComplete(false)
    setCurrentStageIndex(0)
    onStart?.()
    runStage(0)
  }

  const runStage = (stageIndex: number) => {
    if (stageIndex >= demoStages.length) {
      setIsRunning(false)
      setIsComplete(true)
      onComplete?.()
      
      if (loopDemo) {
        setTimeout(() => {
          setCurrentStageIndex(0)
          setIsComplete(false)
          runStage(0)
        }, 2000)
      }
      return
    }

    const stage = demoStages[stageIndex]
    setCurrentStageIndex(stageIndex)
    onStageChange?.(stage)

    setTimeout(() => {
      runStage(stageIndex + 1)
    }, stage.duration * speedMultipliers[speed])
  }

  const resetDemo = () => {
    setIsRunning(false)
    setIsComplete(false)
    setCurrentStageIndex(0)
  }

  const currentStage = demoStages[currentStageIndex]
  const progressPercent = ((currentStageIndex + (isRunning ? 1 : 0)) / demoStages.length) * 100

  const demoContent = (
    <div className={cx('flex flex-col h-full bg-black', className)}>
      {/* Demo Header */}
      <div className="relative bg-gradient-to-r from-black via-gray-900 to-black border-b border-cyan-500/30 p-8 text-center overflow-hidden">
        {/* Background Animation */}
        <div className="absolute inset-0 opacity-10">
          <div className="absolute top-4 left-4 animate-pulse">
            <BitcoinIcon className="text-yellow-400" />
          </div>
          <div className="absolute top-8 right-8 animate-bounce" style={{ animationDelay: '1s' }}>
            <RocketIcon className="text-cyan-400" />
          </div>
          <div className="absolute bottom-4 left-1/4 animate-pulse" style={{ animationDelay: '2s' }}>
            <BitcoinIcon className="text-yellow-400" />
          </div>
          <div className="absolute bottom-8 right-1/4 animate-bounce" style={{ animationDelay: '0.5s' }}>
            <RocketIcon className="text-cyan-400" />
          </div>
        </div>

        <div className="relative z-10">
          <Text as="h1" className="text-4xl font-bold text-transparent bg-clip-text bg-gradient-to-r from-cyan-400 via-yellow-400 to-cyan-400 mb-4">
            Bitcoin Puns Website
          </Text>
          <Text className="text-xl text-gray-300 mb-6">
            Watch OpenAgents build and deploy a complete website from a simple chat message
          </Text>
          
          {!isRunning && !isComplete && (
            <button
              onClick={startDemo}
              className="inline-flex items-center gap-3 px-8 py-4 bg-gradient-to-r from-cyan-500 to-yellow-500 text-black font-medium rounded-lg hover:from-cyan-400 hover:to-yellow-400 transition-all duration-300 cursor-pointer transform hover:scale-105"
            >
              <PlayIcon className="w-6 h-6" />
              Start Demo
            </button>
          )}

          {isComplete && (
            <div className="space-y-4">
              <div className="flex items-center justify-center gap-2 text-green-400">
                <CheckIcon className="w-6 h-6" />
                <Text className="text-xl font-medium">Demo Complete!</Text>
              </div>
              <div className="space-x-4">
                <button
                  onClick={startDemo}
                  className="px-6 py-3 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors cursor-pointer"
                >
                  Watch Again
                </button>
                <button
                  onClick={() => window.open('https://bitcoin-puns.openagents.dev', '_blank')}
                  className="px-6 py-3 bg-yellow-500/20 text-yellow-300 border border-yellow-500/50 rounded hover:bg-yellow-500/30 transition-colors cursor-pointer"
                >
                  Visit Live Site
                </button>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Progress Bar */}
      {showProgress && (isRunning || isComplete) && (
        <div className="px-8 py-4 border-b border-cyan-500/20 bg-black/50">
          <div className="flex items-center justify-between text-sm text-gray-400 mb-2">
            <span>Demo Progress</span>
            <span>{currentStageIndex + (isComplete ? 1 : 0)}/{demoStages.length}</span>
          </div>
          <div className="w-full bg-gray-700 rounded-full h-2">
            <div
              className="h-2 rounded-full bg-gradient-to-r from-cyan-400 to-yellow-400 transition-all duration-500"
              style={{ width: `${isComplete ? 100 : progressPercent}%` }}
            />
          </div>
        </div>
      )}

      {/* Current Stage Info */}
      {(isRunning || isComplete) && currentStage && (
        <div className="px-8 py-6 border-b border-cyan-500/20 bg-cyan-500/5">
          <div className="max-w-3xl mx-auto text-center">
            <Text as="h2" className="text-2xl font-medium text-cyan-300 mb-2">
              {currentStage.title}
            </Text>
            <Text className="text-gray-300 text-lg mb-4">
              {currentStage.description}
            </Text>
            {isRunning && (
              <div className="flex items-center justify-center gap-2 text-cyan-400">
                <div className="w-2 h-2 bg-current rounded-full animate-pulse" />
                <Text className="text-sm font-medium">{currentStage.action}</Text>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Demo Workspace */}
      <div className="flex-1">
        {(isRunning || isComplete) && (
          <ProjectWorkspace
            currentProject="Bitcoin Puns Website"
            overallStatus={
              currentStageIndex <= 2 ? 'idle' :
              currentStageIndex === 3 ? 'generating' :
              currentStageIndex === 4 ? 'deploying' :
              'complete'
            }
            animated={false}
          />
        )}

        {!isRunning && !isComplete && (
          <div className="flex items-center justify-center h-full">
            <div className="text-center text-gray-500 space-y-4">
              <BitcoinIcon className="w-16 h-16 mx-auto opacity-50" />
              <Text className="text-xl">Ready to start the demo</Text>
              <Text className="text-sm">Click "Start Demo" above to begin</Text>
            </div>
          </div>
        )}
      </div>

      {/* Demo Controls */}
      {(isRunning || isComplete) && (
        <div className="px-8 py-4 border-t border-cyan-500/20 bg-black/30">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <Text className="text-sm text-gray-400">Speed:</Text>
              {(['slow', 'normal', 'fast'] as const).map((s) => (
                <button
                  key={s}
                  onClick={() => !isRunning && setCurrentStageIndex(0)}
                  className={cx(
                    'px-3 py-1 text-xs rounded transition-colors cursor-pointer',
                    speed === s
                      ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50'
                      : 'text-gray-400 hover:text-cyan-300'
                  )}
                >
                  {s.charAt(0).toUpperCase() + s.slice(1)}
                </button>
              ))}
            </div>
            
            <div className="flex items-center gap-2">
              <button
                onClick={resetDemo}
                className="px-4 py-2 text-sm bg-gray-500/20 text-gray-300 border border-gray-500/50 rounded hover:bg-gray-500/30 transition-colors cursor-pointer"
              >
                Reset
              </button>
              
              {isComplete && (
                <button
                  onClick={startDemo}
                  className="px-4 py-2 text-sm bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors cursor-pointer"
                >
                  Play Again
                </button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  )

  if (!animated) {
    return demoContent
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 1, exit: 0.6 }}>
      <Animator active={active}>
        <Animated animated={[['opacity', 0, 1], ['scale', 0.95, 1]]}>
          {demoContent}
        </Animated>
      </Animator>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Templates/BitcoinPunsDemo',
  component: BitcoinPunsDemo,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Complete demo flow component showcasing the full OpenAgents experience from chat to deployment. Perfect for demonstrations and onboarding.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    autoStart: {
      control: 'boolean',
      description: 'Automatically start demo on mount'
    },
    showProgress: {
      control: 'boolean',
      description: 'Show progress bar during demo'
    },
    loopDemo: {
      control: 'boolean',
      description: 'Loop demo continuously'
    },
    speed: {
      control: 'select',
      options: ['slow', 'normal', 'fast'],
      description: 'Demo playback speed'
    },
    animated: {
      control: 'boolean',
      description: 'Enable entrance animation'
    }
  }
} satisfies Meta<typeof BitcoinPunsDemo>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const AutoStart: Story = {
  args: {
    autoStart: true,
    speed: 'fast'
  }
}

export const SlowDemo: Story = {
  args: {
    speed: 'slow',
    showProgress: true
  }
}

export const FastDemo: Story = {
  args: {
    speed: 'fast',
    autoStart: true
  }
}

export const LoopingDemo: Story = {
  args: {
    autoStart: true,
    loopDemo: true,
    speed: 'fast'
  }
}

export const MinimalDemo: Story = {
  args: {
    showProgress: false
  }
}

export const InteractiveDemo: Story = {
  args: {},
  render: () => {
    const [currentStage, setCurrentStage] = useState<DemoStage | null>(null)
    const [completedRuns, setCompletedRuns] = useState(0)

    const handleStageChange = (stage: DemoStage) => {
      setCurrentStage(stage)
    }

    const handleComplete = () => {
      setCompletedRuns(prev => prev + 1)
    }

    return (
      <div className="h-screen space-y-4">
        <BitcoinPunsDemo
          onStageChange={handleStageChange}
          onComplete={handleComplete}
        />
        
        {/* Demo Info */}
        <div className="absolute top-4 right-4 bg-black/80 border border-cyan-500/30 rounded-lg p-4 text-sm">
          <div className="space-y-2">
            <div>
              <strong className="text-cyan-300">Current Stage:</strong>
              <div className="text-gray-300">{currentStage?.title || 'Ready to start'}</div>
            </div>
            <div>
              <strong className="text-cyan-300">Completed Runs:</strong>
              <div className="text-gray-300">{completedRuns}</div>
            </div>
          </div>
        </div>
      </div>
    )
  }
}

export const Playground: Story = {
  args: {
    autoStart: false,
    showProgress: true,
    loopDemo: false,
    speed: 'normal',
    animated: true
  }
}