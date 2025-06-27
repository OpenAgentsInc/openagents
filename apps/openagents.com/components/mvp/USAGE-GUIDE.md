# MVP Components Usage Guide

## Quick Start Examples

### Building a Chat Interface

```typescript
import { ChatInterface } from './organisms/ChatInterface'
import { StatusBadge } from './atoms/StatusBadge'
import { ChatMessage } from './molecules/ChatMessage'
import { StreamingMessage } from './molecules/StreamingMessage'

// Basic chat implementation
function MyChat() {
  const [messages, setMessages] = useState([])
  const [status, setStatus] = useState('idle')
  const [streamingContent, setStreamingContent] = useState('')

  const handleSendMessage = async (text: string) => {
    // Add user message
    setMessages(prev => [...prev, {
      id: Date.now().toString(),
      role: 'user',
      content: text,
      timestamp: new Date()
    }])
    
    // Set generating status
    setStatus('generating')
    
    // Simulate streaming AI response
    setStreamingContent('')
    const response = await generateStreamingResponse(text, (chunk) => {
      setStreamingContent(prev => prev + chunk)
    })
    
    // Add final AI message
    setMessages(prev => [...prev, {
      id: (Date.now() + 1).toString(),
      role: 'assistant',
      content: response,
      timestamp: new Date(),
      model: 'claude-3-sonnet'
    }])
    
    setStreamingContent('')
    setStatus('idle')
  }

  return (
    <div className="h-screen bg-black">
      <StatusBadge status={status} className="absolute top-4 right-4" />
      <ChatInterface 
        messages={messages}
        streamingMessage={streamingContent ? {
          role: 'assistant',
          content: streamingContent,
          model: 'claude-3-sonnet'
        } : null}
        isLoading={status === 'generating'}
        onSendMessage={handleSendMessage}
        autoScroll={true}
      />
    </div>
  )
}
```

### Creating a Deployment Flow

```typescript
import { DeploymentProgress } from './organisms/DeploymentProgress'
import { DeploymentSuccess } from './templates/DeploymentSuccess'
import { DeploymentStage } from './molecules/DeploymentStage'

function DeploymentFlow() {
  const [currentStage, setCurrentStage] = useState('initializing')
  const [isComplete, setIsComplete] = useState(false)
  const [deploymentUrl, setDeploymentUrl] = useState('')

  const stages = [
    { id: 'initializing', title: 'Initializing', status: 'complete' },
    { id: 'building', title: 'Building Application', status: 'running' },
    { id: 'optimizing', title: 'Optimizing for Edge', status: 'pending' },
    { id: 'deploying', title: 'Deploying to 320+ Locations', status: 'pending' }
  ]

  useEffect(() => {
    // Simulate deployment progression
    const stageOrder = ['initializing', 'building', 'optimizing', 'deploying', 'complete']
    let currentIndex = 0

    const interval = setInterval(() => {
      currentIndex++
      if (currentIndex < stageOrder.length - 1) {
        setCurrentStage(stageOrder[currentIndex])
      } else {
        setIsComplete(true)
        setDeploymentUrl('https://bitcoin-puns.openagents.dev')
        clearInterval(interval)
      }
    }, 3000)

    return () => clearInterval(interval)
  }, [])

  if (isComplete) {
    return (
      <DeploymentSuccess
        projectName="Bitcoin Puns"
        deploymentUrl={deploymentUrl}
        deploymentTime={45}
        onViewProject={() => window.open(deploymentUrl)}
        onBackToDashboard={() => console.log('Back to dashboard')}
        onCreateAnother={() => console.log('Create another project')}
      />
    )
  }

  return (
    <DeploymentProgress
      stages={stages}
      currentStage={currentStage}
      projectName="Bitcoin Puns"
      estimatedTime={60}
    />
  )
}
```

### Building a Code Generation UI

```typescript
import { GenerationProgress } from './organisms/GenerationProgress'
import { CodeBlock } from './molecules/CodeBlock'
import { GenerationStep } from './molecules/GenerationStep'

function CodeGenerationView() {
  const [currentStep, setCurrentStep] = useState(0)
  const [generatedFiles, setGeneratedFiles] = useState({})
  const [activeFile, setActiveFile] = useState('index.html')

  const steps = [
    { 
      id: 'html', 
      file: 'index.html', 
      status: 'complete',
      description: 'Creating HTML structure'
    },
    { 
      id: 'css', 
      file: 'styles.css', 
      status: 'generating',
      description: 'Styling the interface'
    },
    { 
      id: 'js', 
      file: 'app.js', 
      status: 'pending',
      description: 'Adding interactivity'
    }
  ]

  const codeExamples = {
    'index.html': `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Bitcoin Puns</title>
    <link rel="stylesheet" href="styles.css">
</head>
<body>
    <div id="app">
        <h1>Bitcoin Puns Generator</h1>
        <button id="generate">Generate Pun</button>
        <div id="pun-display"></div>
    </div>
    <script src="app.js"></script>
</body>
</html>`,
    'styles.css': `/* Bitcoin Puns Styling */
body {
    font-family: 'Monaco', monospace;
    background: linear-gradient(135deg, #f7931a, #ff6b35);
    margin: 0;
    padding: 20px;
}

#app {
    max-width: 600px;
    margin: 0 auto;
    text-align: center;
    background: white;
    padding: 40px;
    border-radius: 10px;
    box-shadow: 0 10px 30px rgba(0,0,0,0.2);
}`,
    'app.js': `// Bitcoin Puns Logic
const puns = [
    "I'm bit-curious about Bitcoin!",
    "This is crypto-nite!",
    "To the moon and back!",
    "HODL on tight!"
];

document.getElementById('generate').addEventListener('click', () => {
    const randomPun = puns[Math.floor(Math.random() * puns.length)];
    document.getElementById('pun-display').textContent = randomPun;
});`
  }

  return (
    <div className="p-8 bg-black min-h-screen">
      <GenerationProgress
        steps={steps}
        currentStep={currentStep}
        onStepClick={(index) => setCurrentStep(index)}
        totalFiles={3}
        completedFiles={steps.filter(s => s.status === 'complete').length}
      />
      
      <div className="mt-8 grid grid-cols-1 lg:grid-cols-2 gap-8">
        {/* File tabs */}
        <div>
          <div className="flex space-x-2 mb-4">
            {Object.keys(codeExamples).map((filename) => (
              <button
                key={filename}
                onClick={() => setActiveFile(filename)}
                className={`px-4 py-2 text-sm font-mono ${
                  activeFile === filename 
                    ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50'
                    : 'text-cyan-500 border border-cyan-500/30 hover:bg-cyan-500/10'
                }`}
              >
                {filename}
              </button>
            ))}
          </div>
          
          <CodeBlock
            code={codeExamples[activeFile]}
            language={activeFile.endsWith('.html') ? 'html' : activeFile.endsWith('.css') ? 'css' : 'javascript'}
            title={activeFile}
            showLineNumbers={true}
            highlightLines={activeFile === 'index.html' ? [8, 9, 10] : undefined}
          />
        </div>

        {/* Generation steps detail */}
        <div className="space-y-4">
          {steps.map((step, index) => (
            <GenerationStep
              key={step.id}
              file={step.file}
              status={step.status}
              description={step.description}
              isActive={currentStep === index}
              progress={step.status === 'generating' ? 65 : undefined}
            />
          ))}
        </div>
      </div>
    </div>
  )
}
```

### Complete Project Workspace

```typescript
import { ProjectWorkspace } from './organisms/ProjectWorkspace'
import { StatusBadge } from './atoms/StatusBadge'

function App() {
  const [layout, setLayout] = useState('three-column')
  const [messages, setMessages] = useState([])
  const [generationSteps, setGenerationSteps] = useState([])
  const [deploymentStage, setDeploymentStage] = useState('idle')
  const [appStatus, setAppStatus] = useState('idle')

  const handleLayoutChange = (newLayout) => {
    setLayout(newLayout)
  }

  const handlePanelToggle = (panelId) => {
    // Handle panel collapse/expand
    console.log(`Toggle panel: ${panelId}`)
  }

  return (
    <div className="h-screen bg-black">
      <div className="absolute top-4 right-4 z-50">
        <StatusBadge status={appStatus} />
      </div>
      
      <ProjectWorkspace
        currentProject="My Bitcoin App"
        layout={layout}
        onLayoutChange={handleLayoutChange}
        leftPanel={{
          id: 'chat',
          type: 'chat',
          title: 'Project Chat',
          content: { 
            messages,
            onSendMessage: (text) => {
              setAppStatus('generating')
              // Handle message sending
            }
          },
          isCollapsed: false,
          isMaximized: false
        }}
        centerPanel={{
          id: 'generation',
          type: 'generation',
          title: 'Code Generation',
          content: { 
            steps: generationSteps,
            currentStep: 0,
            totalFiles: 5
          },
          isCollapsed: false,
          isMaximized: false
        }}
        rightPanel={{
          id: 'deployment',
          type: 'deployment',
          title: 'Cloud Deployment',
          content: { 
            currentStage: deploymentStage,
            stages: [
              { id: 'build', title: 'Building', status: 'pending' },
              { id: 'deploy', title: 'Deploying', status: 'pending' }
            ]
          },
          isCollapsed: false,
          isMaximized: false
        }}
        onPanelToggle={handlePanelToggle}
      />
    </div>
  )
}
```

### Building Onboarding Experience

```typescript
import { AutoPlayingDemoLoop } from './organisms/AutoPlayingDemoLoop'
import { OnboardingPathSelector } from './organisms/OnboardingPathSelector'
import { HeroCallToAction } from './atoms/HeroCallToAction'
import { LiveUsageStats } from './atoms/LiveUsageStats'

function OnboardingFlow() {
  const [currentStep, setCurrentStep] = useState('landing')
  const [isAuthenticated, setIsAuthenticated] = useState(false)

  const handleGetStarted = () => {
    if (!isAuthenticated) {
      // Trigger GitHub auth
      console.log('Starting GitHub authentication...')
      setIsAuthenticated(true)
      setCurrentStep('path-selection')
    }
  }

  const handlePathSelection = (path) => {
    console.log(`Selected path: ${path}`)
    // Navigate to appropriate flow
  }

  if (currentStep === 'landing') {
    return (
      <div className="min-h-screen bg-black">
        {/* Hero section */}
        <div className="relative">
          <div className="max-w-6xl mx-auto px-8 py-20">
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-12 items-center">
              {/* Left side - content */}
              <div>
                <h1 className="text-5xl font-bold text-cyan-300 mb-6">
                  Chat to Deploy
                </h1>
                <p className="text-xl text-cyan-500 mb-8">
                  Build and deploy web applications in seconds using AI. 
                  No code required.
                </p>
                
                <HeroCallToAction
                  ctaText="Deploy Your First App"
                  benefitsText="Free • No Credit Card • 60 Second Deploy"
                  countdown={30}
                  onClick={handleGetStarted}
                />
                
                <div className="mt-8">
                  <LiveUsageStats
                    deploymentsToday={1247}
                    activeUsers={89}
                    averageDeployTime={47}
                  />
                </div>
              </div>
              
              {/* Right side - demo */}
              <div>
                <AutoPlayingDemoLoop
                  demos={[
                    {
                      title: 'Bitcoin Puns App',
                      description: 'Built in 45 seconds',
                      framework: 'vanilla'
                    },
                    {
                      title: 'Weather Dashboard',
                      description: 'Built in 52 seconds',
                      framework: 'react'
                    },
                    {
                      title: 'Todo Manager',
                      description: 'Built in 38 seconds',
                      framework: 'vue'
                    }
                  ]}
                  autoPlayInterval={8000}
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    )
  }

  if (currentStep === 'path-selection') {
    return (
      <div className="min-h-screen bg-black flex items-center justify-center">
        <OnboardingPathSelector
          onTemplateSelect={() => handlePathSelection('template')}
          onChatSelect={() => handlePathSelection('chat')}
          templateBenefits={[
            'Deploy in 30 seconds',
            'Proven designs',
            'One-click customization'
          ]}
          chatBenefits={[
            'Build anything',
            'AI guidance',
            'Learn as you go'
          ]}
        />
      </div>
    )
  }

  return null
}
```

## Common Patterns

### 1. Status Management

```typescript
// Centralized status that flows through components
const [appStatus, setAppStatus] = useState<
  'idle' | 'generating' | 'deploying' | 'deployed' | 'error'
>('idle')

// Status flows through multiple components
<StatusBadge status={appStatus} />
<ChatInterface isLoading={appStatus === 'generating'} />
<DeploymentProgress isActive={appStatus === 'deploying'} />
<ProjectHeader status={appStatus} />
```

### 2. Message Streaming

```typescript
// Enhanced streaming with proper state management
const [streamingContent, setStreamingContent] = useState('')
const [streamingMetadata, setStreamingMetadata] = useState(null)

useEffect(() => {
  if (!aiResponse) return

  const stream = aiResponse.stream()
  
  stream.on('start', (metadata) => {
    setStreamingMetadata(metadata)
    setStreamingContent('')
  })
  
  stream.on('data', (chunk) => {
    setStreamingContent(prev => prev + chunk)
  })
  
  stream.on('end', () => {
    // Convert streaming message to regular message
    setMessages(prev => [...prev, {
      role: 'assistant',
      content: streamingContent,
      ...streamingMetadata
    }])
    setStreamingContent('')
    setStreamingMetadata(null)
  })
}, [aiResponse])

// Display streaming message with proper metadata
{streamingContent && (
  <StreamingMessage
    content={streamingContent}
    model={streamingMetadata?.model}
    role="assistant"
    animated={[['opacity', 0, 1]]}
  />
)}
```

### 3. Copy Functionality

```typescript
// Enhanced copy handler with analytics and feedback
const handleCopy = useCallback((text: string, source: string) => {
  navigator.clipboard.writeText(text).then(() => {
    // Track analytics
    analytics.track('content_copied', {
      source,
      length: text.length,
      timestamp: Date.now()
    })
    
    // Show toast notification
    showToast('Copied to clipboard!', 'success')
  }).catch((error) => {
    console.error('Copy failed:', error)
    showToast('Failed to copy', 'error')
  })
}, [])

// Use throughout the application
<CopyButton 
  text={codeSnippet} 
  onCopy={() => handleCopy(codeSnippet, 'code_block')}
  variant="icon-only"
  size="sm"
/>

<ChatMessage 
  content={message.content}
  onCopy={() => handleCopy(message.content, 'chat_message')}
  showCopyButton={true}
/>

<DeploymentUrl
  url={deploymentUrl}
  onCopy={() => handleCopy(deploymentUrl, 'deployment_url')}
/>
```

### 4. Animation Coordination

```typescript
// Coordinated animations with proper timing
const [animationPhase, setAnimationPhase] = useState(0)
const [isVisible, setIsVisible] = useState(false)

useEffect(() => {
  // Entrance sequence
  const timers = [
    setTimeout(() => {
      setIsVisible(true)
      setAnimationPhase(1)
    }, 100),
    setTimeout(() => setAnimationPhase(2), 300),
    setTimeout(() => setAnimationPhase(3), 500)
  ]
  
  return () => timers.forEach(clearTimeout)
}, [])

// Apply coordinated animations
<AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
  <Animator active={isVisible}>
    <div className="space-y-6">
      <Animated 
        animated={[['y', -20, 0], ['opacity', 0, 1]]}
        className={animationPhase >= 1 ? 'animate-in' : ''}
      >
        <ChatInterface />
      </Animated>
      
      <Animated 
        animated={[['x', -30, 0], ['opacity', 0, 1]]}
        className={animationPhase >= 2 ? 'animate-in' : ''}
      >
        <GenerationProgress />
      </Animated>
      
      <Animated 
        animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}
        className={animationPhase >= 3 ? 'animate-in' : ''}
      >
        <DeploymentProgress />
      </Animated>
    </div>
  </Animator>
</AnimatorGeneralProvider>
```

### 5. Error Handling

```typescript
// Comprehensive error handling pattern
const [error, setError] = useState<Error | null>(null)
const [retryCount, setRetryCount] = useState(0)

const safeExecute = useCallback(async (fn: () => Promise<void>) => {
  try {
    setError(null)
    await fn()
  } catch (err) {
    setError(err)
    setAppStatus('error')
    
    // Log to analytics
    analytics.track('error_occurred', {
      error: err.message,
      component: 'MyComponent',
      retryCount
    })
  }
}, [retryCount])

const handleRetry = useCallback(() => {
  setRetryCount(prev => prev + 1)
  setError(null)
  // Retry the failed operation
}, [])

// Display errors with recovery options
{error && (
  <div className="p-4 mb-4 bg-red-500/20 border border-red-500/50 rounded">
    <div className="flex items-start gap-3">
      <StatusBadge status="error" />
      <div className="flex-1">
        <Text className="text-red-400 font-semibold mb-2">
          {error.message}
        </Text>
        <div className="flex gap-2">
          <button 
            onClick={handleRetry}
            className="px-3 py-1 bg-red-500/20 text-red-300 border border-red-500/50 hover:bg-red-500/30"
          >
            Retry
          </button>
          <button 
            onClick={() => setError(null)}
            className="px-3 py-1 text-red-400 hover:text-red-300"
          >
            Dismiss
          </button>
        </div>
      </div>
    </div>
  </div>
)}
```

## Best Practices

### 1. Component Composition

```typescript
// ❌ Don't: Over-configure single components
<ChatInterface 
  messageDisplayConfig={{
    showTimestamp: true,
    showModel: true,
    showAvatar: false,
    actions: {
      copy: true,
      retry: true,
      edit: false
    },
    styling: {
      messageSpacing: 'compact',
      codeBlockTheme: 'dark'
    }
  }}
/>

// ✅ Do: Compose smaller, focused components
<ChatInterface>
  <ChatMessage 
    showTimestamp={true}
    showModelBadge={true}
    enableCopy={true}
    enableRetry={true}
  />
  <StreamingMessage 
    showCursor={true}
    animationSpeed="normal"
  />
</ChatInterface>
```

### 2. State Synchronization

```typescript
// ❌ Don't: Multiple sources of truth
const [chatStatus, setChatStatus] = useState('idle')
const [genStatus, setGenStatus] = useState('idle')
const [deployStatus, setDeployStatus] = useState('idle')

// ✅ Do: Single source of truth with derived state
const [appState, setAppState] = useState({
  phase: 'chat', // 'chat' | 'generation' | 'deployment'
  status: 'idle', // 'idle' | 'processing' | 'complete' | 'error'
  data: {},
  error: null
})

const isGenerating = appState.phase === 'generation' && appState.status === 'processing'
const isDeploying = appState.phase === 'deployment' && appState.status === 'processing'
const hasError = appState.status === 'error'
```

### 3. Performance Optimization

```typescript
// ❌ Don't: Recreate callbacks on every render
<ChatInterface 
  onSendMessage={(text) => {
    // Complex logic here
    processMessage(text)
    updateAnalytics(text)
    saveToHistory(text)
  }}
/>

// ✅ Do: Memoize callbacks and expensive operations
const handleSendMessage = useCallback((text: string) => {
  processMessage(text)
  updateAnalytics(text)
  saveToHistory(text)
}, [processMessage, updateAnalytics, saveToHistory])

const expensiveData = useMemo(() => {
  return computeExpensiveValue(rawData)
}, [rawData])

<ChatInterface onSendMessage={handleSendMessage} />
```

### 4. Accessibility

```typescript
// Always include comprehensive accessibility
<div className="chat-interface" role="main" aria-label="Chat interface">
  <StatusBadge 
    status={status}
    role="status"
    aria-live="polite"
    aria-label={`Current status: ${status}`}
  />
  
  <CopyButton 
    text={code}
    aria-label="Copy code to clipboard"
    onKeyDown={(e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault()
        handleCopy()
      }
    }}
  />
  
  <ChatInterface
    aria-label="Chat conversation"
    role="log"
    aria-live="polite"
  />
</div>
```

### 5. Component Testing

```typescript
// Comprehensive test setup for MVP components
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { AnimatorGeneralProvider } from '@arwes/react'
import { ChatInterface } from './ChatInterface'

const TestWrapper = ({ children }) => (
  <AnimatorGeneralProvider duration={{ enter: 0, exit: 0 }}>
    {children}
  </AnimatorGeneralProvider>
)

test('handles message sending with Enter key', async () => {
  const handleSend = jest.fn()
  
  render(
    <TestWrapper>
      <ChatInterface 
        messages={[]}
        onSendMessage={handleSend}
        isLoading={false}
      />
    </TestWrapper>
  )
  
  const input = screen.getByPlaceholderText(/ask me to build/i)
  
  fireEvent.change(input, { target: { value: 'Build a website' } })
  fireEvent.keyDown(input, { key: 'Enter', shiftKey: false })
  
  await waitFor(() => {
    expect(handleSend).toHaveBeenCalledWith('Build a website')
  })
})

test('displays streaming messages correctly', async () => {
  const streamingMessage = {
    role: 'assistant',
    content: 'I am typing...',
    model: 'claude-3-sonnet'
  }
  
  render(
    <TestWrapper>
      <ChatInterface 
        messages={[]}
        streamingMessage={streamingMessage}
        isLoading={true}
      />
    </TestWrapper>
  )
  
  expect(screen.getByText('I am typing...')).toBeInTheDocument()
  expect(screen.getByText('claude-3-sonnet')).toBeInTheDocument()
})
```

## Advanced Integration Patterns

### Real-time WebSocket Integration

```typescript
// WebSocket integration with MVP components
const useDeploymentSocket = (projectId: string) => {
  const [deploymentState, setDeploymentState] = useState({
    stage: 'idle',
    progress: 0,
    logs: []
  })

  useEffect(() => {
    const ws = new WebSocket(`wss://api.openagents.dev/deploy/${projectId}`)
    
    ws.onmessage = (event) => {
      const data = JSON.parse(event.data)
      
      setDeploymentState(prev => ({
        ...prev,
        stage: data.stage,
        progress: data.progress,
        logs: [...prev.logs, data.log]
      }))
    }
    
    return () => ws.close()
  }, [projectId])

  return deploymentState
}

// Use in components
function LiveDeploymentView({ projectId }) {
  const deployment = useDeploymentSocket(projectId)
  
  return (
    <DeploymentProgress
      currentStage={deployment.stage}
      progress={deployment.progress}
      logs={deployment.logs}
      projectName="My App"
    />
  )
}
```

This usage guide provides comprehensive examples and patterns for effectively using the MVP component library while following best practices for performance, accessibility, and maintainability.