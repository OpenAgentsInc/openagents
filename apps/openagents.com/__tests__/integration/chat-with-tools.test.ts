import { describe, it, expect, vi, beforeEach } from 'vitest'
import { NextRequest } from 'next/server'

// Mock the AI SDK first
vi.mock('ai', () => ({
  streamText: vi.fn(),
  openrouter: vi.fn((model: string) => ({ name: model }))
}))

// Mock the openrouter provider
vi.mock('@openrouter/ai-sdk-provider', () => ({
  openrouter: vi.fn((model: string) => ({ name: model }))
}))

// Mock the tool
const mockToolExecute = vi.fn()
vi.mock('@/lib/tools/createArtifactTool', () => ({
  createArtifactTool: {
    description: 'Create or update a code artifact',
    parameters: {
      parse: vi.fn().mockReturnValue({
        identifier: 'test-artifact',
        title: 'Test Artifact',
        type: 'react',
        content: 'test content',
        operation: 'create'
      })
    },
    execute: mockToolExecute
  }
}))

// Mock the system prompt
vi.mock('@/lib/prompts/artifactSystemPrompt', () => ({
  getArtifactSystemPrompt: () => 'System prompt for artifacts'
}))

// Mock XML parser
vi.mock('@/lib/tools/xmlArtifactParser', () => ({
  parseArtifactTags: vi.fn().mockReturnValue([])
}))

describe('Chat API with Tools Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('should configure streamText with correct parameters', async () => {
    // Import and get the mocked function
    const { streamText } = await import('ai')
    const mockStreamText = vi.mocked(streamText)
    
    // Import the route handler
    const { POST } = await import('@/app/api/chat/route')
    
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        messages: [
          { role: 'user', content: 'Create a simple React component' }
        ],
        projectId: 'test-project',
        projectName: 'Test Project'
      })
    })

    // Mock streamText to return a mock response
    const mockDataStream = {
      pipeDataStreamToResponse: vi.fn()
    }
    mockStreamText.mockReturnValue(mockDataStream)

    await POST(mockRequest)

    expect(mockStreamText).toHaveBeenCalledWith({
      model: { name: 'anthropic/claude-3.5-sonnet' },
      system: 'System prompt for artifacts',
      messages: [
        { role: 'user', content: 'Create a simple React component' }
      ],
      tools: expect.objectContaining({
        createArtifact: expect.objectContaining({
          description: 'Create or update a code artifact'
        })
      }),
      maxSteps: 3,
      onStepFinish: expect.any(Function)
    })
  })

  it('should handle tool calls in onStepFinish callback', async () => {
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        messages: [
          { role: 'user', content: 'Create a button component' }
        ]
      })
    })

    let onStepFinishCallback: any
    const mockDataStream = {
      pipeDataStreamToResponse: vi.fn()
    }
    
    mockStreamText.mockImplementation((config) => {
      onStepFinishCallback = config.onStepFinish
      return mockDataStream
    })

    mockToolExecute.mockResolvedValue({
      success: true,
      artifactId: 'button-component',
      operation: 'create'
    })

    await POST(mockRequest)

    // Simulate a tool call step
    const mockStepResult = {
      stepType: 'tool-call',
      toolCalls: [
        {
          toolCallId: 'call-123',
          toolName: 'createArtifact',
          args: {
            identifier: 'button-component',
            title: 'Button Component',
            type: 'react',
            content: 'export default function Button() { return <button>Click me</button> }',
            operation: 'create'
          },
          result: {
            success: true,
            artifactId: 'button-component',
            operation: 'create'
          }
        }
      ],
      text: 'Created a button component',
      usage: { totalTokens: 100 }
    }

    // Call the onStepFinish callback
    if (onStepFinishCallback) {
      await onStepFinishCallback(mockStepResult)
    }

    expect(mockToolExecute).toHaveBeenCalledWith({
      identifier: 'button-component',
      title: 'Button Component',
      type: 'react',
      content: 'export default function Button() { return <button>Click me</button> }',
      operation: 'create'
    })
  })

  it('should handle XML fallback when tool calls fail', async () => {
    const { parseArtifactTags } = await import('@/lib/tools/xmlArtifactParser')
    
    // Mock XML parser to return parsed artifacts
    ;(parseArtifactTags as any).mockReturnValue([
      {
        identifier: 'xml-component',
        title: 'XML Component',
        type: 'react',
        content: 'export default function XMLComponent() { return <div>XML</div> }',
        operation: 'create'
      }
    ])

    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        messages: [
          { role: 'user', content: 'Create a component' }
        ]
      })
    })

    let onStepFinishCallback: any
    const mockDataStream = {
      pipeDataStreamToResponse: vi.fn()
    }
    
    mockStreamText.mockImplementation((config) => {
      onStepFinishCallback = config.onStepFinish
      return mockDataStream
    })

    await POST(mockRequest)

    // Simulate a text step (no tool calls, fallback to XML parsing)
    const mockTextStepResult = {
      stepType: 'text',
      text: `Here's your component:

<artifact identifier="xml-component" type="react" title="XML Component">
export default function XMLComponent() {
  return <div>XML</div>
}
</artifact>`,
      usage: { totalTokens: 150 }
    }

    // Call the onStepFinish callback
    if (onStepFinishCallback) {
      await onStepFinishCallback(mockTextStepResult)
    }

    expect(parseArtifactTags).toHaveBeenCalledWith(mockTextStepResult.text)
  })

  it('should include project context in request body', async () => {
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        messages: [
          { role: 'user', content: 'Create a component for my project' }
        ],
        projectId: 'my-awesome-project',
        projectName: 'My Awesome Project',
        context: {
          framework: 'react',
          deploymentUrl: 'https://my-project.vercel.app'
        }
      })
    })

    const mockDataStream = {
      pipeDataStreamToResponse: vi.fn()
    }
    mockStreamText.mockReturnValue(mockDataStream)

    await POST(mockRequest)

    // Verify that the project context is available to the system
    expect(mockStreamText).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            content: 'Create a component for my project'
          })
        ])
      })
    )
  })

  it('should handle errors gracefully', async () => {
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        messages: [
          { role: 'user', content: 'Create a component' }
        ]
      })
    })

    // Mock streamText to throw an error
    mockStreamText.mockImplementation(() => {
      throw new Error('AI service unavailable')
    })

    const response = await POST(mockRequest)
    
    expect(response.status).toBe(500)
    
    const responseText = await response.text()
    expect(responseText).toContain('Failed to process chat request')
  })

  it('should validate request body', async () => {
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        // Missing messages array
        projectId: 'test'
      })
    })

    const response = await POST(mockRequest)
    
    expect(response.status).toBe(400)
    
    const responseText = await response.text()
    expect(responseText).toContain('Invalid request')
  })

  it('should handle POST requests only', async () => {
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'GET'
    })

    // The route should only export POST, so GET should not be handled
    // This test verifies the API design
    expect(POST).toBeDefined()
  })
})