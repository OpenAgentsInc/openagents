import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState, useEffect, useRef } from 'react'
import { Animator, AnimatorGeneralProvider, Animated, Text, cx, FrameKranox } from '@arwes/react'
import { StreamingMessage } from '../molecules/StreamingMessage.stories'
import { DeploymentProgress } from './DeploymentProgress.stories'
import { DeploymentSuccess } from '../templates/DeploymentSuccess.stories'
import { CodeBlock } from '../molecules/CodeBlock.stories'

// AutoPlayingDemoLoop component
export interface Demo {
  id: string
  prompt: string
  title: string
  description: string
  code: string
  deploymentUrl: string
  deployTime: number
  features: string[]
}

export interface AutoPlayingDemoLoopProps {
  demos?: Demo[]
  autoPlayDelay?: number
  demoTransitionDelay?: number
  onDemoChange?: (demo: Demo) => void
  onDemoComplete?: (demo: Demo) => void
  onInteraction?: () => void
  showControls?: boolean
  className?: string
}

const defaultDemos: Demo[] = [
  {
    id: 'bitcoin-tracker',
    prompt: 'Create a Bitcoin price tracker with live updates',
    title: 'Bitcoin Price Tracker',
    description: 'Real-time cryptocurrency price monitoring',
    code: `import React, { useState, useEffect } from 'react'

export function BitcoinTracker() {
  const [price, setPrice] = useState(null)
  const [loading, setLoading] = useState(true)
  
  useEffect(() => {
    fetch('https://api.coindesk.com/v1/bpi/currentprice.json')
      .then(res => res.json())
      .then(data => {
        setPrice(data.bpi.USD.rate)
        setLoading(false)
      })
  }, [])
  
  return (
    <div className="bitcoin-tracker">
      {loading ? (
        <p>Loading...</p>
      ) : (
        <h1>Bitcoin: ${price}</h1>
      )}
    </div>
  )
}`,
    deploymentUrl: 'bitcoin-tracker-xyz.openagents.dev',
    deployTime: 28,
    features: ['Real-time API integration', 'Responsive design', 'Auto-refresh']
  },
  {
    id: 'todo-app',
    prompt: 'Build a todo app with drag and drop',
    title: 'Task Manager',
    description: 'Modern task management with drag & drop',
    code: `import React, { useState } from 'react'
import { DragDropContext, Droppable, Draggable } from 'react-beautiful-dnd'

export function TodoApp() {
  const [todos, setTodos] = useState([
    { id: '1', content: 'Launch product' },
    { id: '2', content: 'Write documentation' }
  ])
  
  const handleDragEnd = (result) => {
    if (!result.destination) return
    
    const items = Array.from(todos)
    const [reorderedItem] = items.splice(result.source.index, 1)
    items.splice(result.destination.index, 0, reorderedItem)
    
    setTodos(items)
  }
  
  return (
    <DragDropContext onDragEnd={handleDragEnd}>
      <Droppable droppableId="todos">
        {(provided) => (
          <ul {...provided.droppableProps} ref={provided.innerRef}>
            {todos.map((todo, index) => (
              <Draggable key={todo.id} draggableId={todo.id} index={index}>
                {(provided) => (
                  <li
                    ref={provided.innerRef}
                    {...provided.draggableProps}
                    {...provided.dragHandleProps}
                  >
                    {todo.content}
                  </li>
                )}
              </Draggable>
            ))}
            {provided.placeholder}
          </ul>
        )}
      </Droppable>
    </DragDropContext>
  )
}`,
    deploymentUrl: 'todo-app-abc.openagents.dev',
    deployTime: 35,
    features: ['Drag & drop interface', 'Local storage', 'Keyboard shortcuts']
  },
  {
    id: 'weather-dashboard',
    prompt: 'Create a weather dashboard with charts',
    title: 'Weather Dashboard',
    description: 'Beautiful weather visualization',
    code: `import React, { useState, useEffect } from 'react'
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip } from 'recharts'

export function WeatherDashboard() {
  const [weather, setWeather] = useState(null)
  const [forecast, setForecast] = useState([])
  
  useEffect(() => {
    navigator.geolocation.getCurrentPosition(async (position) => {
      const { latitude, longitude } = position.coords
      const response = await fetch(\`/api/weather?lat=\${latitude}&lon=\${longitude}\`)
      const data = await response.json()
      
      setWeather(data.current)
      setForecast(data.forecast)
    })
  }, [])
  
  return (
    <div className="weather-dashboard">
      {weather && (
        <>
          <h1>{weather.temp}°F</h1>
          <p>{weather.description}</p>
          <LineChart width={600} height={300} data={forecast}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="time" />
            <YAxis />
            <Tooltip />
            <Line type="monotone" dataKey="temp" stroke="#8884d8" />
          </LineChart>
        </>
      )}
    </div>
  )
}`,
    deploymentUrl: 'weather-dash-789.openagents.dev',
    deployTime: 42,
    features: ['Geolocation API', 'Interactive charts', 'Hourly forecast']
  }
]

type DemoPhase = 'prompt' | 'streaming' | 'building' | 'deploying' | 'success' | 'transition'

export const AutoPlayingDemoLoop = ({
  demos = defaultDemos,
  autoPlayDelay = 2000,
  demoTransitionDelay = 3000,
  onDemoChange,
  onDemoComplete,
  onInteraction,
  showControls = true,
  className = ''
}: AutoPlayingDemoLoopProps) => {
  const [currentDemoIndex, setCurrentDemoIndex] = useState(0)
  const [phase, setPhase] = useState<DemoPhase>('prompt')
  const [isPaused, setIsPaused] = useState(false)
  const [hasInteracted, setHasInteracted] = useState(false)
  const phaseTimeoutRef = useRef<NodeJS.Timeout>()
  const demoTimeoutRef = useRef<NodeJS.Timeout>()
  const currentDemo = demos[currentDemoIndex]

  // Clear timeouts on unmount
  useEffect(() => {
    return () => {
      if (phaseTimeoutRef.current) clearTimeout(phaseTimeoutRef.current)
      if (demoTimeoutRef.current) clearTimeout(demoTimeoutRef.current)
    }
  }, [])

  // Handle demo progression
  useEffect(() => {
    if (isPaused || hasInteracted) return

    const progressDemo = () => {
      switch (phase) {
        case 'prompt':
          phaseTimeoutRef.current = setTimeout(() => {
            setPhase('streaming')
          }, autoPlayDelay)
          break
          
        case 'streaming':
          phaseTimeoutRef.current = setTimeout(() => {
            setPhase('building')
          }, 3000) // Time for streaming message
          break
          
        case 'building':
          phaseTimeoutRef.current = setTimeout(() => {
            setPhase('deploying')
          }, 2000) // Time for building phase
          break
          
        case 'deploying':
          phaseTimeoutRef.current = setTimeout(() => {
            setPhase('success')
            onDemoComplete?.(currentDemo)
          }, 2500) // Time for deployment
          break
          
        case 'success':
          phaseTimeoutRef.current = setTimeout(() => {
            setPhase('transition')
          }, demoTransitionDelay)
          break
          
        case 'transition':
          const nextIndex = (currentDemoIndex + 1) % demos.length
          setCurrentDemoIndex(nextIndex)
          setPhase('prompt')
          onDemoChange?.(demos[nextIndex])
          break
      }
    }

    progressDemo()

    return () => {
      if (phaseTimeoutRef.current) clearTimeout(phaseTimeoutRef.current)
    }
  }, [phase, isPaused, hasInteracted, currentDemoIndex, currentDemo, demos, autoPlayDelay, demoTransitionDelay, onDemoChange, onDemoComplete])

  const handleInteraction = () => {
    setHasInteracted(true)
    setIsPaused(true)
    onInteraction?.()
  }

  const handleResume = () => {
    setHasInteracted(false)
    setIsPaused(false)
  }

  const handleDemoSelect = (index: number) => {
    setCurrentDemoIndex(index)
    setPhase('prompt')
    setHasInteracted(true)
    setIsPaused(true)
  }

  return (
    <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
      <div 
        className={cx('relative', className)}
        onClick={handleInteraction}
        onMouseEnter={() => !hasInteracted && setIsPaused(true)}
        onMouseLeave={() => !hasInteracted && setIsPaused(false)}
      >
        {/* Demo Container */}
        <div className="relative min-h-[600px]">
          <FrameKranox className="absolute inset-0" />
          <div className="relative p-8 h-full">
          {/* Status Badge */}
          <div className="absolute top-4 right-4 flex items-center gap-2">
            <div className="flex items-center gap-2 px-3 py-1 bg-red-500/20 border border-red-500/50 rounded-full">
              <div className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
              <Text as="span" className="text-red-300 text-sm font-mono">
                LIVE DEMO
              </Text>
            </div>
            {isPaused && (
              <div className="px-3 py-1 bg-yellow-500/20 border border-yellow-500/50 rounded-full">
                <Text as="span" className="text-yellow-300 text-sm font-mono">
                  PAUSED
                </Text>
              </div>
            )}
          </div>

          {/* Prompt Phase */}
          {phase === 'prompt' && (
            <Animator active={true}>
              <div className="space-y-6">
                <Animated animated={[['opacity', 0, 1], ['y', 20, 0]]}>
                  <div className="max-w-2xl">
                    <Text as="h3" className="text-2xl font-bold text-cyan-300 mb-2">
                      Demo {currentDemoIndex + 1} of {demos.length}
                    </Text>
                    <Text as="p" className="text-gray-400 mb-6">
                      Watch as we build and deploy in under {currentDemo.deployTime} seconds
                    </Text>
                    
                    <div className="bg-gray-900/50 border border-gray-700 rounded-lg p-4">
                      <Text as="p" className="text-cyan-300 font-mono">
                        $ openagents chat
                      </Text>
                      <Text 
                        as="p" 
                        className="text-gray-200 font-mono mt-2"
                        manager="sequence"
                      >
                        {currentDemo.prompt}
                      </Text>
                    </div>
                  </div>
                </Animated>
              </div>
            </Animator>
          )}

          {/* Streaming Phase */}
          {phase === 'streaming' && (
            <div className="space-y-4">
              <StreamingMessage
                role="user"
                content={currentDemo.prompt}
                animated={true}
                showTimestamp={false}
              />
              <StreamingMessage
                role="assistant"
                content={`I'll help you create ${currentDemo.title}. This will include ${currentDemo.features.join(', ')}. Let me generate the code for you...`}
                isStreaming={true}
                streamingSpeed={30}
                model="llama-3-8b-instruct"
                provider="cloudflare"
                animated={true}
                showTimestamp={false}
              />
            </div>
          )}

          {/* Building Phase */}
          {phase === 'building' && (
            <Animator active={true}>
              <div className="space-y-6">
                <Animated animated={[['opacity', 0, 1]]}>
                  <Text as="h3" className="text-xl font-bold text-cyan-300">
                    Generating {currentDemo.title}...
                  </Text>
                </Animated>
                
                <Animated 
                  animated={[['opacity', 0, 1], ['y', 20, 0]]}
                  style={{ animationDelay: '0.2s' }}
                >
                  <CodeBlock
                    code={currentDemo.code}
                    language="typescript"
                    filename="App.tsx"
                    showLineNumbers={true}
                    animated={true}
                  />
                </Animated>
              </div>
            </Animator>
          )}

          {/* Deploying Phase */}
          {phase === 'deploying' && (
            <DeploymentProgress
              projectName={currentDemo.title}
              stages={[
                { id: 'build', label: 'Building application', status: 'completed' },
                { id: 'optimize', label: 'Optimizing for edge', status: 'completed' },
                { id: 'deploy', label: 'Deploying to 320+ locations', status: 'in-progress' },
                { id: 'dns', label: 'Configuring DNS', status: 'pending' }
              ]}
              currentStage={2}
              estimatedTime={currentDemo.deployTime}
            />
          )}

          {/* Success Phase */}
          {phase === 'success' && (
            <DeploymentSuccess
              projectName={currentDemo.title}
              deploymentUrl={`https://${currentDemo.deploymentUrl}`}
              deploymentTime={currentDemo.deployTime}
              showNextSteps={false}
            />
          )}
          </div>
        </div>

        {/* Controls */}
        {showControls && (
          <div className="mt-6 flex items-center justify-between">
            <div className="flex gap-2">
              {demos.map((demo, index) => (
                <button
                  key={demo.id}
                  onClick={(e) => {
                    e.stopPropagation()
                    handleDemoSelect(index)
                  }}
                  className={cx(
                    'w-3 h-3 rounded-full transition-all duration-300',
                    index === currentDemoIndex
                      ? 'bg-cyan-400 shadow-lg shadow-cyan-400/50'
                      : 'bg-gray-600 hover:bg-gray-500'
                  )}
                  aria-label={`Go to demo ${index + 1}: ${demo.title}`}
                />
              ))}
            </div>

            {hasInteracted && (
              <Animator active={true}>
                <Animated animated={[['opacity', 0, 1], ['x', 20, 0]]}>
                  <button
                    onClick={(e) => {
                      e.stopPropagation()
                      handleResume()
                    }}
                    className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 rounded hover:bg-cyan-500/30 transition-colors flex items-center gap-2"
                  >
                    <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
                      <path d="M10 18a8 8 0 100-16 8 8 0 000 16zM9.555 7.168A1 1 0 008 8v4a1 1 0 001.555.832l3-2a1 1 0 000-1.664l-3-2z" />
                    </svg>
                    Resume Demo
                  </button>
                </Animated>
              </Animator>
            )}
          </div>
        )}

        {/* Interaction Hint */}
        {!hasInteracted && (
          <Animator active={true}>
            <Animated 
              animated={[['opacity', 0, 0.8]]}
              className="absolute bottom-4 left-1/2 -translate-x-1/2"
            >
              <Text as="p" className="text-gray-500 text-sm">
                Click to interact • Hover to pause
              </Text>
            </Animated>
          </Animator>
        )}
      </div>
    </AnimatorGeneralProvider>
  )
}

// Storybook configuration
const meta = {
  title: 'MVP/Organisms/AutoPlayingDemoLoop',
  component: AutoPlayingDemoLoop,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Auto-playing demo carousel that cycles through live demonstrations of the platform capabilities. Shows the complete flow from chat prompt to deployed application. Critical for homepage conversion.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    autoPlayDelay: {
      control: { type: 'number', min: 1000, max: 5000, step: 500 },
      description: 'Delay before auto-play starts (ms)'
    },
    demoTransitionDelay: {
      control: { type: 'number', min: 1000, max: 5000, step: 500 },
      description: 'Delay between demos (ms)'
    },
    showControls: {
      control: 'boolean',
      description: 'Show demo navigation controls'
    }
  }
} satisfies Meta<typeof AutoPlayingDemoLoop>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {}
}

export const FastTransition: Story = {
  args: {
    autoPlayDelay: 1000,
    demoTransitionDelay: 1500
  }
}

export const NoControls: Story = {
  args: {
    showControls: false
  }
}

export const SingleDemo: Story = {
  args: {
    demos: [defaultDemos[0]]
  }
}

export const WithCallbacks: Story = {
  render: () => {
    const [events, setEvents] = useState<string[]>([])
    
    const addEvent = (event: string) => {
      setEvents(prev => [...prev.slice(-4), event])
    }
    
    return (
      <div className="space-y-6">
        <AutoPlayingDemoLoop
          onDemoChange={(demo) => addEvent(`Changed to: ${demo.title}`)}
          onDemoComplete={(demo) => addEvent(`Completed: ${demo.title}`)}
          onInteraction={() => addEvent('User interacted')}
        />
        
        <div className="bg-gray-900/50 border border-gray-700 rounded p-4">
          <Text as="h4" className="text-cyan-300 mb-2">Events</Text>
          <div className="space-y-1">
            {events.map((event, index) => (
              <Text key={index} as="p" className="text-gray-400 text-sm font-mono">
                {event}
              </Text>
            ))}
          </div>
        </div>
      </div>
    )
  }
}

export const CustomDemos: Story = {
  args: {
    demos: [
      {
        id: 'portfolio',
        prompt: 'Create a developer portfolio site',
        title: 'Portfolio Website',
        description: 'Professional developer portfolio',
        code: `export function Portfolio() {
  return <div>Portfolio Content</div>
}`,
        deploymentUrl: 'portfolio-demo.openagents.dev',
        deployTime: 25,
        features: ['About section', 'Project gallery', 'Contact form']
      },
      {
        id: 'blog',
        prompt: 'Build a markdown blog',
        title: 'Markdown Blog',
        description: 'Static blog with markdown support',
        code: `export function Blog() {
  return <div>Blog Content</div>
}`,
        deploymentUrl: 'blog-demo.openagents.dev',
        deployTime: 30,
        features: ['Markdown rendering', 'Syntax highlighting', 'RSS feed']
      }
    ]
  }
}

export const Playground: Story = {
  args: {
    autoPlayDelay: 2000,
    demoTransitionDelay: 3000,
    showControls: true
  }
}