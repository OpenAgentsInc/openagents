import React from 'react'
import { render, RenderOptions } from '@testing-library/react'
import { vi } from 'vitest'
import { AnimatorGeneralProvider } from '@arwes/react'
import { ToastProvider } from '@/components/Toast'
import { ArtifactsProvider } from '@/components/artifacts/ArtifactsContext'

// Mock Convex client and provider
const MockConvexProvider = ({ children }: { children: React.ReactNode }) => {
  return <div data-testid="mock-convex-provider">{children}</div>
}

// Mock auth hook for testing
export const mockAuthHook = {
  isAuthenticated: true,
  signIn: vi.fn(),
  signOut: vi.fn(),
  user: { id: 'test-user', name: 'Test User' }
}

// Mock useAuth hook
vi.mock('@/hooks/useAuth', () => ({
  useAuth: () => mockAuthHook
}))

// Create a more realistic useChat mock
type ChatMessage = {
  id: string
  role: 'user' | 'assistant'
  content: string
  createdAt: Date
}

let mockChatState: {
  messages: ChatMessage[]
  input: string
  isLoading: boolean
  error: Error | null
} = {
  messages: [],
  input: '',
  isLoading: false,
  error: null
}

// Reset function for test isolation
export const resetMockChatState = () => {
  mockChatState = {
    messages: [],
    input: '',
    isLoading: false,
    error: null
  }
}

// Mock useChat from ai/react
vi.mock('ai/react', () => ({
  useChat: (config: any) => {
    // Start with initial messages if provided
    if (config?.initialMessages && mockChatState.messages.length === 0) {
      mockChatState.messages = [...config.initialMessages]
    }

    const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      mockChatState.input = e.target.value
    }

    const handleSubmit = (e: React.FormEvent) => {
      e.preventDefault()
      if (!mockChatState.input.trim() || mockChatState.isLoading) return

      // Add user message
      const userMessage: ChatMessage = {
        id: `user-${Date.now()}`,
        role: 'user',
        content: mockChatState.input,
        createdAt: new Date()
      }
      
      mockChatState.messages = [...mockChatState.messages, userMessage]
      mockChatState.input = ''
      mockChatState.isLoading = true

      // Simulate AI response after short delay
      setTimeout(() => {
        const aiMessage: ChatMessage = {
          id: `ai-${Date.now()}`,
          role: 'assistant',
          content: 'Hello! I can help you build applications. Here is some React code:\n\n```jsx\nfunction HelloWorld() {\n  return <div>Hello World!</div>\n}\n```',
          createdAt: new Date()
        }
        mockChatState.messages = [...mockChatState.messages, aiMessage]
        mockChatState.isLoading = false
        
        // Trigger onFinish callback if provided
        if (config?.onFinish) {
          config.onFinish(aiMessage)
        }
      }, 100)
    }

    return {
      messages: mockChatState.messages,
      input: mockChatState.input,
      handleInputChange,
      handleSubmit,
      isLoading: mockChatState.isLoading,
      error: mockChatState.error,
      reload: vi.fn(),
      setMessages: vi.fn((newMessages: ChatMessage[]) => {
        mockChatState.messages = newMessages
      })
    }
  }
}))

// All providers needed for integration testing
function AllTheProviders({ children }: { children: React.ReactNode }) {
  return (
    <MockConvexProvider>
      <AnimatorGeneralProvider>
        <ToastProvider>
          <ArtifactsProvider>
            {children}
          </ArtifactsProvider>
        </ToastProvider>
      </AnimatorGeneralProvider>
    </MockConvexProvider>
  )
}

// Custom render function that includes all providers
const customRender = (
  ui: React.ReactElement,
  options?: Omit<RenderOptions, 'wrapper'>,
) => render(ui, { wrapper: AllTheProviders, ...options })

// Mock AI chat response for testing
export const mockChatResponse = {
  id: 'test-message-id',
  role: 'assistant' as const,
  content: 'This is a test AI response. Here is some React code:\n\n```jsx\nfunction TestComponent() {\n  return <div>Hello World!</div>\n}\n```',
  createdAt: new Date()
}

// Helper to wait for async operations
export const waitForAsync = () => new Promise(resolve => setTimeout(resolve, 0))

// Helper to simulate user typing
export const simulateTyping = async (input: HTMLElement, text: string) => {
  const userEvent = await import('@testing-library/user-event')
  const user = userEvent.default.setup()
  
  // Ensure element is interactive by focusing it
  if (input instanceof HTMLTextAreaElement || input instanceof HTMLInputElement) {
    // Clear existing value first
    input.value = ''
    
    // Focus the element
    input.focus()
    
    // Manually trigger input change to simulate typing
    input.value = text
    
    // Fire change and input events
    const changeEvent = new Event('change', { bubbles: true })
    const inputEvent = new Event('input', { bubbles: true })
    
    input.dispatchEvent(inputEvent)
    input.dispatchEvent(changeEvent)
  } else {
    // Fallback to user-event for other elements
    try {
      await user.click(input)
      await user.clear(input)
      await user.type(input, text)
    } catch (error) {
      // If user events fail, try direct manipulation
      console.warn('User event failed, using direct manipulation:', error)
      if (input instanceof HTMLElement) {
        (input as any).value = text
      }
    }
  }
}

// Re-export everything from React Testing Library
export * from '@testing-library/react'

// Override render with our custom version
export { customRender as render }