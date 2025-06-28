import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook } from '@testing-library/react'
import { useArtifactCreation } from '@/hooks/useArtifactCreation'
import { Message } from 'ai'

// Mock the artifacts context
const mockAddArtifact = vi.fn()
vi.mock('@/components/artifacts/ArtifactsContext', () => ({
  useArtifactOperations: () => ({
    addArtifact: mockAddArtifact
  })
}))

describe('useArtifactCreation', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockAddArtifact.mockReturnValue('test-artifact-id')
  })

  describe('extractCodeFromMessage', () => {
    it('should extract TypeScript/TSX code blocks', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const content = `Here's a React component:
\`\`\`tsx
import React from 'react'

function HelloWorld() {
  return <div>Hello, World!</div>
}

export default HelloWorld
\`\`\`
That's the component!`

      const extracted = result.current.extractCodeFromMessage(content)
      
      expect(extracted).not.toBeNull()
      expect(extracted?.code).toContain('function HelloWorld()')
      expect(extracted?.code).toContain('export default HelloWorld')
      expect(extracted?.language).toBe('tsx')
    })

    it('should extract JavaScript code blocks', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const content = `\`\`\`javascript
const sum = (a, b) => a + b
console.log(sum(1, 2))
\`\`\``

      const extracted = result.current.extractCodeFromMessage(content)
      
      expect(extracted).not.toBeNull()
      expect(extracted?.code).toContain('const sum = (a, b) => a + b')
      expect(extracted?.language).toBe('javascript')
    })

    it('should extract generic code blocks', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const content = `\`\`\`
function genericCode() {
  return "This has no language specified"
}
\`\`\``

      const extracted = result.current.extractCodeFromMessage(content)
      
      expect(extracted).not.toBeNull()
      expect(extracted?.code).toContain('function genericCode()')
      expect(extracted?.language).toBe('tsx') // defaults to tsx
    })

    it('should extract the last code block when multiple exist', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const content = `First block:
\`\`\`js
console.log('first')
\`\`\`

Second block:
\`\`\`tsx
export default function Final() {
  return <div>Final component</div>
}
\`\`\``

      const extracted = result.current.extractCodeFromMessage(content)
      
      expect(extracted?.code).toContain('Final component')
      expect(extracted?.code).not.toContain('console.log')
      expect(extracted?.language).toBe('tsx')
    })

    it('should return null when no code blocks exist', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const content = 'This is just plain text without any code blocks'
      const extracted = result.current.extractCodeFromMessage(content)
      
      expect(extracted).toBeNull()
    })

    it('should handle empty code blocks', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const content = `\`\`\`tsx
\`\`\``

      const extracted = result.current.extractCodeFromMessage(content)
      
      expect(extracted).not.toBeNull()
      expect(extracted?.code).toBe('')
    })
  })

  describe('createArtifactFromMessage', () => {
    it('should create artifact from assistant message with code', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `Here's a Bitcoin tracker:
\`\`\`tsx
import React from 'react'

export default function BitcoinTracker() {
  return <div>Bitcoin Tracker</div>
}
\`\`\``,
        createdAt: new Date()
      }

      const userMessage = 'Build a Bitcoin tracker app'
      const artifactId = result.current.createArtifactFromMessage(message, userMessage)
      
      expect(artifactId).toBe('test-artifact-id')
      expect(mockAddArtifact).toHaveBeenCalledWith({
        title: 'BitcoinTracker',
        description: 'React component',
        type: 'code',
        content: expect.stringContaining('export default function BitcoinTracker()'),
        conversationId: 'msg-123',
        messageId: 'msg-123'
      })
    })

    it('should not create artifact from user messages', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'user',
        content: `\`\`\`tsx
export default function Test() {}
\`\`\``,
        createdAt: new Date()
      }

      const artifactId = result.current.createArtifactFromMessage(message)
      
      expect(artifactId).toBeNull()
      expect(mockAddArtifact).not.toHaveBeenCalled()
    })

    it('should not create artifact without code blocks', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: 'This is just a text response without code',
        createdAt: new Date()
      }

      const artifactId = result.current.createArtifactFromMessage(message)
      
      expect(artifactId).toBeNull()
      expect(mockAddArtifact).not.toHaveBeenCalled()
    })

    it('should not create artifact for incomplete components', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `\`\`\`tsx
// This is just a comment, not a complete component
const x = 5
\`\`\``,
        createdAt: new Date()
      }

      const artifactId = result.current.createArtifactFromMessage(message)
      
      expect(artifactId).toBeNull()
      expect(mockAddArtifact).not.toHaveBeenCalled()
    })

    it('should extract title from export default statement', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `\`\`\`tsx
export default function CustomDashboard() {
  return <div>Dashboard</div>
}
\`\`\``,
        createdAt: new Date()
      }

      result.current.createArtifactFromMessage(message)
      
      expect(mockAddArtifact).toHaveBeenCalledWith(
        expect.objectContaining({
          title: 'CustomDashboard'
        })
      )
    })

    it('should extract title from user message when available', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `\`\`\`tsx
function App() {
  return <div>App</div>
}
export default App
\`\`\``,
        createdAt: new Date()
      }

      const userMessage = 'Create a todo list application'
      result.current.createArtifactFromMessage(message, userMessage)
      
      expect(mockAddArtifact).toHaveBeenCalledWith(
        expect.objectContaining({
          title: 'Todo List Application'
        })
      )
    })

    it('should extract description from JSDoc comments', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `\`\`\`tsx
/**
 * A real-time Bitcoin price tracker component
 */
export default function BitcoinTracker() {
  return <div>Bitcoin Tracker</div>
}
\`\`\``,
        createdAt: new Date()
      }

      result.current.createArtifactFromMessage(message)
      
      expect(mockAddArtifact).toHaveBeenCalledWith(
        expect.objectContaining({
          description: 'A real-time Bitcoin price tracker component'
        })
      )
    })

    it('should detect components with state management', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `\`\`\`tsx
import { useState, useEffect } from 'react'

export default function Counter() {
  const [count, setCount] = useState(0)
  
  useEffect(() => {
    console.log('Count changed:', count)
  }, [count])
  
  return <div>{count}</div>
}
\`\`\``,
        createdAt: new Date()
      }

      result.current.createArtifactFromMessage(message)
      
      expect(mockAddArtifact).toHaveBeenCalledWith(
        expect.objectContaining({
          description: 'Interactive React component with state and effects'
        })
      )
    })

    it('should handle arrow function components', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `\`\`\`tsx
const MyComponent = () => {
  return <div>Hello</div>
}

export default MyComponent
\`\`\``,
        createdAt: new Date()
      }

      const artifactId = result.current.createArtifactFromMessage(message)
      
      expect(artifactId).toBe('test-artifact-id')
      expect(mockAddArtifact).toHaveBeenCalledWith(
        expect.objectContaining({
          title: 'MyComponent',
          type: 'code'
        })
      )
    })
  })

  describe('edge cases', () => {
    it('should handle malformed code blocks gracefully', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `Here's some code:
\`\`\`tsx
export default function Incomplete() {
  // Missing closing brace
  return <div>Test</div>
\`\`\``,
        createdAt: new Date()
      }

      // Should still create artifact even with syntax errors
      const artifactId = result.current.createArtifactFromMessage(message)
      
      expect(artifactId).toBe('test-artifact-id')
    })

    it('should handle messages with code but no proper component structure', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `\`\`\`tsx
import React from 'react'

// Just imports and types, no component
interface Props {
  name: string
}

type State = {
  count: number
}
\`\`\``,
        createdAt: new Date()
      }

      const artifactId = result.current.createArtifactFromMessage(message)
      
      expect(artifactId).toBeNull()
      expect(mockAddArtifact).not.toHaveBeenCalled()
    })

    it('should handle very long code blocks', () => {
      const { result } = renderHook(() => useArtifactCreation())
      
      const longCode = `
import React from 'react'

export default function VeryLongComponent() {
  ${'const item = "test"\n'.repeat(100)}
  return <div>Long component</div>
}
`
      
      const message: Message = {
        id: 'msg-123',
        role: 'assistant',
        content: `\`\`\`tsx${longCode}\`\`\``,
        createdAt: new Date()
      }

      const artifactId = result.current.createArtifactFromMessage(message)
      
      expect(artifactId).toBe('test-artifact-id')
      expect(mockAddArtifact).toHaveBeenCalledWith(
        expect.objectContaining({
          content: expect.stringContaining('const item = "test"')
        })
      )
    })
  })
})