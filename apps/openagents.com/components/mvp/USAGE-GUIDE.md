# MVP Components Usage Guide

## Quick Start Examples

### Building a Chat Interface

```typescript
import { ChatInterface } from './organisms/ChatInterface.stories'
import { StatusBadge } from './atoms/StatusBadge.stories'

// Basic chat implementation
function MyChat() {
  const [messages, setMessages] = useState([])
  const [status, setStatus] = useState('idle')

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
    
    // Simulate AI response
    const response = await generateResponse(text)
    
    // Add AI message
    setMessages(prev => [...prev, {
      id: Date.now().toString(),
      role: 'assistant',
      content: response,
      timestamp: new Date(),
      model: 'claude-3-sonnet'
    }])
    
    setStatus('idle')
  }

  return (
    <div className="h-screen bg-black">
      <StatusBadge status={status} className="absolute top-4 right-4" />
      <ChatInterface 
        messages={messages}
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
import { DeploymentProgress } from './organisms/DeploymentProgress.stories'
import { DeploymentSuccess } from './templates/DeploymentSuccess.stories'

function DeploymentFlow() {
  const [stage, setStage] = useState('initializing')
  const [isComplete, setIsComplete] = useState(false)

  useEffect(() => {
    // Simulate deployment stages
    const stages = ['initializing', 'building', 'optimizing', 'deploying', 'complete']
    let currentIndex = 0

    const interval = setInterval(() => {
      currentIndex++
      if (currentIndex < stages.length) {
        setStage(stages[currentIndex])
      } else {
        setIsComplete(true)
        clearInterval(interval)
      }
    }, 3000)

    return () => clearInterval(interval)
  }, [])

  if (isComplete) {
    return (
      <DeploymentSuccess
        projectName="Bitcoin Puns"
        deploymentUrl="https://bitcoin-puns.openagents.dev"
        deploymentTime={45}
        onViewProject={() => window.open('https://bitcoin-puns.openagents.dev')}
        onBackToDashboard={() => console.log('Back to dashboard')}
      />
    )
  }

  return (
    <DeploymentProgress
      currentStage={stage}
      projectName="Bitcoin Puns"
    />
  )
}
```

### Building a Code Generation UI

```typescript
import { GenerationProgress } from './organisms/GenerationProgress.stories'
import { CodeBlock } from './molecules/CodeBlock.stories'

function CodeGenerationView() {
  const [currentStep, setCurrentStep] = useState(0)
  const [generatedCode, setGeneratedCode] = useState('')

  const steps = [
    { file: 'index.html', status: 'complete' },
    { file: 'styles.css', status: 'generating' },
    { file: 'app.js', status: 'pending' }
  ]

  return (
    <div className="p-8 bg-black min-h-screen">
      <GenerationProgress
        steps={steps}
        currentStep={currentStep}
        onStepClick={(index) => setCurrentStep(index)}
      />
      
      {generatedCode && (
        <CodeBlock
          code={generatedCode}
          language="javascript"
          title={steps[currentStep].file}
          showLineNumbers={true}
          highlightLines={[5, 6, 7]}
        />
      )}
    </div>
  )
}
```

### Complete Project Workspace

```typescript
import { ProjectWorkspace } from './organisms/ProjectWorkspace.stories'

function App() {
  const [layout, setLayout] = useState('three-column')
  const [messages, setMessages] = useState([])
  const [generationSteps, setGenerationSteps] = useState([])
  const [deploymentStage, setDeploymentStage] = useState('idle')

  return (
    <ProjectWorkspace
      currentProject="My Bitcoin App"
      layout={layout}
      onLayoutChange={setLayout}
      leftPanel={{
        id: 'chat',
        type: 'chat',
        title: 'Chat',
        content: { messages }
      }}
      centerPanel={{
        id: 'generation',
        type: 'generation',
        title: 'Code Generation',
        content: { steps: generationSteps }
      }}
      rightPanel={{
        id: 'deployment',
        type: 'deployment',
        title: 'Deployment',
        content: { stage: deploymentStage }
      }}
    />
  )
}
```

## Common Patterns

### 1. Status Management

```typescript
// Centralized status that flows through components
const [appStatus, setAppStatus] = useState<
  'idle' | 'generating' | 'deploying' | 'deployed' | 'error'
>('idle')

// Pass to multiple components
<StatusBadge status={appStatus} />
<ChatInterface isLoading={appStatus === 'generating'} />
<DeploymentProgress isActive={appStatus === 'deploying'} />
```

### 2. Message Streaming

```typescript
// Streaming AI responses
const [streamingContent, setStreamingContent] = useState('')

useEffect(() => {
  const stream = aiResponse.stream()
  
  stream.on('data', (chunk) => {
    setStreamingContent(prev => prev + chunk)
  })
  
  stream.on('end', () => {
    // Convert streaming message to regular message
    setMessages(prev => [...prev, {
      role: 'assistant',
      content: streamingContent
    }])
    setStreamingContent('')
  })
}, [])

// Display streaming message
{streamingContent && (
  <StreamingMessage
    content={streamingContent}
    model="claude-3-sonnet"
  />
)}
```

### 3. Copy Functionality

```typescript
// Reusable copy handler
const handleCopy = (text: string, source: string) => {
  navigator.clipboard.writeText(text)
  
  // Track what was copied
  analytics.track('content_copied', {
    source,
    length: text.length
  })
  
  // Show toast notification
  showToast('Copied to clipboard!')
}

// Use in multiple places
<CopyButton 
  text={codeSnippet} 
  onCopy={() => handleCopy(codeSnippet, 'code_block')}
/>

<ChatMessage 
  onCopy={() => handleCopy(message.content, 'chat_message')}
/>
```

### 4. Animation Coordination

```typescript
// Coordinated animations across components
const [animationPhase, setAnimationPhase] = useState(0)

useEffect(() => {
  // Stagger component animations
  const timers = [
    setTimeout(() => setAnimationPhase(1), 100),
    setTimeout(() => setAnimationPhase(2), 300),
    setTimeout(() => setAnimationPhase(3), 500)
  ]
  
  return () => timers.forEach(clearTimeout)
}, [])

// Apply to components
<ChatInterface animated={animationPhase >= 1} />
<GenerationProgress animated={animationPhase >= 2} />
<DeploymentProgress animated={animationPhase >= 3} />
```

### 5. Error Handling

```typescript
// Consistent error handling pattern
const [error, setError] = useState<string | null>(null)

const safeExecute = async (fn: () => Promise<void>) => {
  try {
    setError(null)
    await fn()
  } catch (err) {
    setError(err.message)
    setAppStatus('error')
  }
}

// Display errors consistently
{error && (
  <div className="p-4 mb-4 bg-red-500/20 border border-red-500/50 rounded">
    <StatusBadge status="error" className="mb-2" />
    <Text className="text-red-400">{error}</Text>
  </div>
)}
```

## Best Practices

### 1. Component Composition

```typescript
// ❌ Don't: Deeply nested props
<ChatInterface 
  messageConfig={{
    showTimestamp: true,
    showModel: true,
    actions: {
      copy: true,
      retry: true
    }
  }}
/>

// ✅ Do: Flat, clear props
<ChatInterface 
  showTimestamps={true}
  showModelBadges={true}
  enableCopy={true}
  enableRetry={true}
/>
```

### 2. State Synchronization

```typescript
// ❌ Don't: Multiple sources of truth
const [chatStatus, setChatStatus] = useState('idle')
const [genStatus, setGenStatus] = useState('idle')
const [deployStatus, setDeployStatus] = useState('idle')

// ✅ Do: Single source of truth
const [appState, setAppState] = useState({
  status: 'idle',
  phase: 'chat', // 'chat' | 'generation' | 'deployment'
  error: null
})
```

### 3. Performance Optimization

```typescript
// ❌ Don't: Recreate callbacks on every render
<ChatInterface 
  onSendMessage={(text) => {
    // Complex logic here
  }}
/>

// ✅ Do: Memoize callbacks
const handleSendMessage = useCallback((text: string) => {
  // Complex logic here
}, [dependencies])

<ChatInterface onSendMessage={handleSendMessage} />
```

### 4. Accessibility

```typescript
// Always include keyboard navigation
<CopyButton 
  text={code}
  aria-label="Copy code to clipboard"
  onKeyDown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      handleCopy()
    }
  }}
/>

// Announce status changes
<StatusBadge 
  status={status}
  role="status"
  aria-live="polite"
/>
```

### 5. Testing Components

```typescript
// Example test setup
import { render, screen, fireEvent } from '@testing-library/react'
import { ChatInterface } from './ChatInterface.stories'

test('sends message on Enter key', () => {
  const handleSend = jest.fn()
  
  render(
    <ChatInterface 
      messages={[]}
      onSendMessage={handleSend}
    />
  )
  
  const input = screen.getByPlaceholderText(/ask me to build/i)
  fireEvent.change(input, { target: { value: 'Build a website' } })
  fireEvent.keyDown(input, { key: 'Enter' })
  
  expect(handleSend).toHaveBeenCalledWith('Build a website')
})
```