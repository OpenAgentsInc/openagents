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

// Mock fetch API for AI chat responses
const createMockResponse = (content: string) => {
  // Create a streaming response that mimics the AI SDK format
  const encoder = new TextEncoder()
  const stream = new ReadableStream({
    start(controller) {
      // Send the response in the correct AI SDK format
      const response = "Hello! I can help you build applications. Here is some React code:\n\n```jsx\nfunction HelloWorld() {\n  return <div>Hello World!</div>\n}\n```"
      
      // Split response into chunks and stream them
      const words = response.split(' ')
      words.forEach((word, i) => {
        setTimeout(() => {
          controller.enqueue(encoder.encode(`0:"${word}${i < words.length - 1 ? ' ' : ''}"\n`))
          if (i === words.length - 1) {
            controller.enqueue(encoder.encode('d:{"finishReason":"stop"}\n'))
            controller.close()
          }
        }, i * 20)
      })
    }
  })

  return new Response(stream, {
    headers: {
      'Content-Type': 'text/plain; charset=utf-8',
    },
  })
}

// Mock global fetch for AI API
global.fetch = vi.fn((url: string | URL, options?: RequestInit) => {
  const urlString = url.toString()
  
  if (urlString.includes('/api/chat')) {
    // Parse the request body to get the message
    let messages = []
    if (options?.body) {
      try {
        const body = JSON.parse(options.body as string)
        messages = body.messages || []
      } catch (e) {
        // Ignore parsing errors
      }
    }
    
    const lastMessage = messages[messages.length - 1]
    const userMessage = lastMessage?.content || 'default'
    
    return Promise.resolve(createMockResponse(userMessage))
  }
  
  // For other requests, return a basic 404
  return Promise.resolve(new Response('Not Found', { status: 404 }))
})

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
  await user.clear(input)
  await user.type(input, text)
}

// Re-export everything from React Testing Library
export * from '@testing-library/react'

// Override render with our custom version
export { customRender as render }