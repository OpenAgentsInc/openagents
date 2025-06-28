import type { Meta, StoryObj } from '@storybook/nextjs'
import React, { useState } from 'react'
import { AnimatorGeneralProvider, Text, cx } from '@arwes/react'
import { ArtifactsProvider, useArtifacts, useCurrentArtifact, useArtifactOperations } from './ArtifactsContext'
import { Plus, Trash2, ArrowLeft, ArrowRight, Code, Play } from 'lucide-react'

// Demo component that shows artifacts state management
function ArtifactsStateDemo() {
  const { state } = useArtifacts()
  const { artifact: currentArtifact, navigateNext, navigatePrevious } = useCurrentArtifact()
  const { addArtifact, updateArtifact, deleteArtifact, deployArtifact, clearArtifacts } = useArtifactOperations()
  
  const [isDeploying, setIsDeploying] = useState<string | null>(null)

  const handleAddArtifact = () => {
    const artifactTypes = ['Bitcoin App', 'Dashboard', 'Blog', 'E-commerce', 'Portfolio']
    const randomType = artifactTypes[Math.floor(Math.random() * artifactTypes.length)]
    
    addArtifact({
      title: `${randomType} ${Date.now()}`,
      description: `A sample ${randomType.toLowerCase()} generated for demo purposes`,
      type: 'code',
      content: `// ${randomType} component\nimport React from 'react'\n\nfunction ${randomType.replace(' ', '')}() {\n  return (\n    <div className="min-h-screen bg-gray-900 text-white p-8">\n      <h1 className="text-3xl font-bold">${randomType}</h1>\n      <p>This is a sample ${randomType.toLowerCase()} component.</p>\n    </div>\n  )\n}\n\nexport default ${randomType.replace(' ', '')}`
    })
  }

  const handleDeploy = async (id: string) => {
    setIsDeploying(id)
    try {
      await deployArtifact(id)
    } catch (error) {
      console.error('Deploy failed:', error)
    } finally {
      setIsDeploying(null)
    }
  }

  const handleDelete = (id: string) => {
    if (confirm('Are you sure you want to delete this artifact?')) {
      deleteArtifact(id)
    }
  }

  return (
    <div className="h-screen bg-black text-white p-6">
      <div className="max-w-6xl mx-auto">
        {/* Header */}
        <div className="mb-8">
          <Text className="text-2xl font-bold text-cyan-300 mb-2">
            Artifacts State Management Demo
          </Text>
          <Text className="text-cyan-300/60">
            Interactive demonstration of the artifacts context and state management system
          </Text>
        </div>

        {/* State Overview */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 mb-8">
          <div className="bg-cyan-500/10 border border-cyan-500/30 rounded-lg p-4">
            <Text className="text-cyan-300 font-bold mb-2">Total Artifacts</Text>
            <Text className="text-3xl font-bold text-cyan-400">
              {state.artifacts.length}
            </Text>
          </div>
          
          <div className="bg-cyan-500/10 border border-cyan-500/30 rounded-lg p-4">
            <Text className="text-cyan-300 font-bold mb-2">Current Artifact</Text>
            <Text className="text-cyan-400">
              {currentArtifact ? currentArtifact.title : 'None'}
            </Text>
          </div>
          
          <div className="bg-cyan-500/10 border border-cyan-500/30 rounded-lg p-4">
            <Text className="text-cyan-300 font-bold mb-2">Deploying</Text>
            <Text className="text-cyan-400">
              {state.isDeploying.length} artifacts
            </Text>
          </div>
        </div>

        {/* Controls */}
        <div className="flex flex-wrap gap-4 mb-6">
          <button
            onClick={handleAddArtifact}
            className={cx(
              'flex items-center gap-2 px-4 py-2',
              'bg-green-500/20 hover:bg-green-500/30',
              'border border-green-500/50 hover:border-green-500/70',
              'text-green-300 hover:text-green-200',
              'rounded transition-all duration-200'
            )}
          >
            <Plus size={16} />
            Add Random Artifact
          </button>

          <button
            onClick={clearArtifacts}
            className={cx(
              'flex items-center gap-2 px-4 py-2',
              'bg-red-500/20 hover:bg-red-500/30',
              'border border-red-500/50 hover:border-red-500/70',
              'text-red-300 hover:text-red-200',
              'rounded transition-all duration-200'
            )}
            disabled={state.artifacts.length === 0}
          >
            <Trash2 size={16} />
            Clear All
          </button>

          {currentArtifact && (
            <>
              <button
                onClick={navigatePrevious}
                className={cx(
                  'flex items-center gap-2 px-4 py-2',
                  'bg-cyan-500/20 hover:bg-cyan-500/30',
                  'border border-cyan-500/50 hover:border-cyan-500/70',
                  'text-cyan-300 hover:text-cyan-200',
                  'rounded transition-all duration-200'
                )}
                disabled={state.artifacts.findIndex(a => a.id === currentArtifact.id) === 0}
              >
                <ArrowLeft size={16} />
                Previous
              </button>

              <button
                onClick={navigateNext}
                className={cx(
                  'flex items-center gap-2 px-4 py-2',
                  'bg-cyan-500/20 hover:bg-cyan-500/30',
                  'border border-cyan-500/50 hover:border-cyan-500/70',
                  'text-cyan-300 hover:text-cyan-200',
                  'rounded transition-all duration-200'
                )}
                disabled={state.artifacts.findIndex(a => a.id === currentArtifact.id) === state.artifacts.length - 1}
              >
                Next
                <ArrowRight size={16} />
              </button>
            </>
          )}
        </div>

        {/* Artifacts List */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {state.artifacts.length === 0 ? (
            <div className="col-span-full text-center py-12">
              <Code className="w-16 h-16 text-cyan-500/20 mx-auto mb-4" />
              <Text className="text-gray-500 mb-2">No artifacts yet</Text>
              <Text className="text-gray-600 text-sm">
                Click "Add Random Artifact" to create some demo artifacts
              </Text>
            </div>
          ) : (
            state.artifacts.map((artifact, index) => (
              <div
                key={artifact.id}
                className={cx(
                  'border rounded-lg p-4 transition-all duration-200',
                  artifact.id === currentArtifact?.id
                    ? 'border-cyan-500/70 bg-cyan-500/10'
                    : 'border-gray-700/50 bg-gray-800/30 hover:border-gray-600/50'
                )}
              >
                <div className="flex items-start justify-between mb-3">
                  <div className="flex-1">
                    <Text className="font-bold text-cyan-300 mb-1">
                      {artifact.title}
                    </Text>
                    <Text className="text-sm text-cyan-300/60 mb-2">
                      {artifact.description}
                    </Text>
                    <div className="flex items-center gap-4 text-xs text-gray-400">
                      <span>Created: {artifact.createdAt.toLocaleDateString()}</span>
                      <span>•</span>
                      <span>Type: {artifact.type}</span>
                      {artifact.deploymentUrl && (
                        <>
                          <span>•</span>
                          <span className="text-green-400">Deployed</span>
                        </>
                      )}
                    </div>
                  </div>

                  <div className="text-xs text-cyan-400 font-mono">
                    #{index + 1}
                  </div>
                </div>

                <div className="flex gap-2">
                  {!artifact.deploymentUrl ? (
                    <button
                      onClick={() => handleDeploy(artifact.id)}
                      disabled={isDeploying === artifact.id || state.isDeploying.includes(artifact.id)}
                      className={cx(
                        'flex items-center gap-1 px-3 py-1 text-xs',
                        'bg-blue-500/20 hover:bg-blue-500/30',
                        'border border-blue-500/50 hover:border-blue-500/70',
                        'text-blue-300 hover:text-blue-200',
                        'rounded transition-all duration-200',
                        'disabled:opacity-50 disabled:cursor-not-allowed'
                      )}
                    >
                      <Play size={12} />
                      {isDeploying === artifact.id || state.isDeploying.includes(artifact.id) ? 'Deploying...' : 'Deploy'}
                    </button>
                  ) : (
                    <div className="flex items-center gap-1 px-3 py-1 text-xs bg-green-500/20 border border-green-500/50 text-green-300 rounded">
                      <Play size={12} />
                      Deployed
                    </div>
                  )}

                  <button
                    onClick={() => handleDelete(artifact.id)}
                    className={cx(
                      'flex items-center gap-1 px-3 py-1 text-xs',
                      'bg-red-500/20 hover:bg-red-500/30',
                      'border border-red-500/50 hover:border-red-500/70',
                      'text-red-300 hover:text-red-200',
                      'rounded transition-all duration-200'
                    )}
                  >
                    <Trash2 size={12} />
                    Delete
                  </button>
                </div>
              </div>
            ))
          )}
        </div>

        {/* Code Preview */}
        {currentArtifact && (
          <div className="mt-8">
            <Text className="text-cyan-300 font-bold mb-4">
              Current Artifact Code Preview
            </Text>
            <div className="bg-gray-900 border border-gray-700 rounded-lg p-4 overflow-auto">
              <pre className="text-sm text-gray-300">
                <code>{currentArtifact.content}</code>
              </pre>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

const meta = {
  title: 'Artifacts/ArtifactsContext',
  component: ArtifactsStateDemo,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Artifacts context provider and state management system. Demonstrates the complete lifecycle of artifact creation, navigation, deployment, and deletion with persistent localStorage integration.'
      }
    }
  },
  tags: ['autodocs'],
  decorators: [
    (Story) => (
      <AnimatorGeneralProvider>
        <ArtifactsProvider>
          <Story />
        </ArtifactsProvider>
      </AnimatorGeneralProvider>
    )
  ]
} satisfies Meta<typeof ArtifactsStateDemo>

export default meta
type Story = StoryObj<typeof meta>

// Interactive state management demo
export const StateManagement: Story = {
  args: {},
  parameters: {
    docs: {
      description: {
        story: 'Interactive demonstration of the complete artifacts state management system. Create, navigate, deploy, and delete artifacts to see the state changes in real-time.'
      }
    }
  }
}

// Pre-loaded with artifacts
export const PreLoadedState: Story = {
  args: {},
  render: () => {
    React.useEffect(() => {
      const mockArtifacts = [
        {
          id: 'artifact-context-1',
          title: 'React Counter App',
          description: 'Simple counter with increment/decrement',
          type: 'code' as const,
          content: `import React, { useState } from 'react'

function Counter() {
  const [count, setCount] = useState(0)

  return (
    <div className="min-h-screen bg-gray-900 text-white flex items-center justify-center">
      <div className="text-center">
        <h1 className="text-4xl font-bold mb-8">Counter App</h1>
        <div className="text-6xl font-bold mb-8 text-cyan-500">{count}</div>
        <div className="space-x-4">
          <button
            onClick={() => setCount(count - 1)}
            className="bg-red-500 hover:bg-red-600 text-white font-bold py-2 px-4 rounded"
          >
            -
          </button>
          <button
            onClick={() => setCount(count + 1)}
            className="bg-green-500 hover:bg-green-600 text-white font-bold py-2 px-4 rounded"
          >
            +
          </button>
        </div>
      </div>
    </div>
  )
}

export default Counter`,
          createdAt: new Date(Date.now() - 600000),
          updatedAt: new Date(Date.now() - 600000),
          deploymentUrl: 'https://counter-app-123456.openagents.dev'
        },
        {
          id: 'artifact-context-2',
          title: 'Todo List Manager',
          description: 'Task management with add/remove/complete',
          type: 'code' as const,
          content: `import React, { useState } from 'react'

function TodoList() {
  const [todos, setTodos] = useState([
    { id: 1, text: 'Learn React', completed: true },
    { id: 2, text: 'Build awesome apps', completed: false }
  ])
  const [newTodo, setNewTodo] = useState('')

  const addTodo = () => {
    if (newTodo.trim()) {
      setTodos([...todos, {
        id: Date.now(),
        text: newTodo,
        completed: false
      }])
      setNewTodo('')
    }
  }

  const toggleTodo = (id) => {
    setTodos(todos.map(todo =>
      todo.id === id ? { ...todo, completed: !todo.completed } : todo
    ))
  }

  const deleteTodo = (id) => {
    setTodos(todos.filter(todo => todo.id !== id))
  }

  return (
    <div className="min-h-screen bg-gray-100 p-8">
      <div className="max-w-md mx-auto">
        <h1 className="text-3xl font-bold mb-8 text-center">Todo List</h1>
        
        <div className="mb-6">
          <input
            type="text"
            value={newTodo}
            onChange={(e) => setNewTodo(e.target.value)}
            placeholder="Add a new todo..."
            className="w-full p-3 border rounded-lg"
          />
          <button
            onClick={addTodo}
            className="w-full mt-2 bg-blue-500 text-white p-3 rounded-lg hover:bg-blue-600"
          >
            Add Todo
          </button>
        </div>
        
        <div className="space-y-2">
          {todos.map(todo => (
            <div key={todo.id} className="flex items-center gap-2 p-3 bg-white rounded-lg shadow">
              <input
                type="checkbox"
                checked={todo.completed}
                onChange={() => toggleTodo(todo.id)}
              />
              <span className={\`flex-1 \${todo.completed ? 'line-through text-gray-500' : ''}\`}>
                {todo.text}
              </span>
              <button
                onClick={() => deleteTodo(todo.id)}
                className="text-red-500 hover:text-red-700"
              >
                Delete
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

export default TodoList`,
          createdAt: new Date(Date.now() - 300000),
          updatedAt: new Date(Date.now() - 300000)
        },
        {
          id: 'artifact-context-3',
          title: 'Weather Dashboard',
          description: 'Weather app with location search',
          type: 'code' as const,
          content: `import React, { useState, useEffect } from 'react'

function WeatherDashboard() {
  const [weather, setWeather] = useState(null)
  const [city, setCity] = useState('San Francisco')
  const [loading, setLoading] = useState(false)

  const fetchWeather = async () => {
    setLoading(true)
    // Mock weather data
    setTimeout(() => {
      setWeather({
        city,
        temperature: Math.round(Math.random() * 30 + 10),
        condition: ['Sunny', 'Cloudy', 'Rainy'][Math.floor(Math.random() * 3)],
        humidity: Math.round(Math.random() * 100),
        windSpeed: Math.round(Math.random() * 20)
      })
      setLoading(false)
    }, 1000)
  }

  useEffect(() => {
    fetchWeather()
  }, [])

  return (
    <div className="min-h-screen bg-gradient-to-br from-blue-400 to-purple-600 p-8">
      <div className="max-w-md mx-auto">
        <h1 className="text-3xl font-bold text-white mb-8 text-center">Weather Dashboard</h1>
        
        <div className="mb-6">
          <input
            type="text"
            value={city}
            onChange={(e) => setCity(e.target.value)}
            placeholder="Enter city name"
            className="w-full p-3 rounded-lg"
          />
          <button
            onClick={fetchWeather}
            disabled={loading}
            className="w-full mt-2 bg-white text-blue-600 p-3 rounded-lg hover:bg-gray-100 disabled:opacity-50"
          >
            {loading ? 'Loading...' : 'Get Weather'}
          </button>
        </div>
        
        {weather && (
          <div className="bg-white/20 backdrop-blur-sm rounded-lg p-6 text-white">
            <h2 className="text-2xl font-bold mb-4">{weather.city}</h2>
            <div className="text-4xl font-bold mb-2">{weather.temperature}°C</div>
            <div className="text-xl mb-4">{weather.condition}</div>
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>Humidity: {weather.humidity}%</div>
              <div>Wind: {weather.windSpeed} km/h</div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

export default WeatherDashboard`,
          createdAt: new Date(Date.now() - 120000),
          updatedAt: new Date(Date.now() - 120000)
        }
      ]
      
      localStorage.setItem('openagents-artifacts', JSON.stringify(mockArtifacts))
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return <ArtifactsStateDemo />
  },
  parameters: {
    docs: {
      description: {
        story: 'State management demo pre-loaded with multiple artifacts including one deployed artifact. Shows how the context handles different artifact states and deployment statuses.'
      }
    }
  }
}

// Performance test with many artifacts
export const PerformanceTest: Story = {
  args: {},
  render: () => {
    React.useEffect(() => {
      const manyArtifacts = Array.from({ length: 25 }, (_, i) => ({
        id: `artifact-perf-${i}`,
        title: `Performance Test ${i + 1}`,
        description: `Artifact ${i + 1} for performance testing`,
        type: 'code' as const,
        content: `// Performance test artifact ${i + 1}\nimport React from 'react'\n\nfunction PerfTest${i + 1}() {\n  return (\n    <div className="min-h-screen bg-gray-900 text-white p-8">\n      <h1 className="text-3xl font-bold">Performance Test ${i + 1}</h1>\n      <p>This is artifact number ${i + 1} for performance testing.</p>\n    </div>\n  )\n}\n\nexport default PerfTest${i + 1}`,
        createdAt: new Date(Date.now() - (i * 60000)),
        updatedAt: new Date(Date.now() - (i * 60000)),
        ...(i % 3 === 0 ? { deploymentUrl: `https://perf-test-${i}.openagents.dev` } : {})
      }))
      
      localStorage.setItem('openagents-artifacts', JSON.stringify(manyArtifacts))
      window.dispatchEvent(new Event('storage'))
    }, [])
    
    return (
      <div className="h-screen bg-black">
        <ArtifactsStateDemo />
        <div className="absolute top-4 right-4 z-50 bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-4 max-w-sm">
          <Text className="text-yellow-300 font-bold mb-2">⚡ Performance Test</Text>
          <Text className="text-yellow-300/80 text-sm">
            Loaded with 25 artifacts to test state management performance and UI responsiveness.
          </Text>
        </div>
      </div>
    )
  },
  parameters: {
    docs: {
      description: {
        story: 'Performance testing with 25 artifacts to verify efficient state management and UI rendering with larger datasets. Tests navigation performance and memory usage.'
      }
    }
  }
}