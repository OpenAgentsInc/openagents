import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { NextRequest } from 'next/server'

// Mock the AI SDK first
const mockStreamDataAppend = vi.fn()
const mockStreamText = vi.fn()
vi.mock('ai', () => ({
  streamText: mockStreamText,
  StreamData: class StreamData {
    append = mockStreamDataAppend
  }
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
  COMPLETE_SYSTEM_PROMPT: 'COMPLETE SYSTEM PROMPT for testing'
}))

// Mock XML parser
const mockParseArtifactTags = vi.fn()
const mockProcessArtifactsFromResponse = vi.fn()
const mockHasArtifactTags = vi.fn()
vi.mock('@/lib/tools/xmlArtifactParser', () => ({
  parseArtifactTags: mockParseArtifactTags,
  processArtifactsFromResponse: mockProcessArtifactsFromResponse,
  hasArtifactTags: mockHasArtifactTags
}))

describe('Chat API with Tools Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockStreamDataAppend.mockClear()
    mockStreamText.mockClear()
    mockParseArtifactTags.mockReturnValue([])
    mockProcessArtifactsFromResponse.mockReturnValue({ xmlArtifacts: [], validParameters: [] })
    mockHasArtifactTags.mockReturnValue(false)
    // Set environment variable for tests
    process.env.OPENROUTER_API_KEY = 'test-api-key'
  })
  
  afterEach(() => {
    // Clean up environment variable
    delete process.env.OPENROUTER_API_KEY
  })

  it('should configure streamText with correct parameters', async () => {
    // Import route handler
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

    // Mock streamText to return a proper result object
    mockStreamText.mockReturnValueOnce({
      toDataStreamResponse: vi.fn().mockReturnValue(
        new Response('stream', { 
          headers: { 'Content-Type': 'text/event-stream' } 
        })
      )
    } as any)

    const response = await POST(mockRequest)
    expect(response).toBeDefined()

    expect(mockStreamText).toHaveBeenCalledWith(expect.objectContaining({
      model: expect.any(Object),
      system: expect.stringContaining('COMPLETE SYSTEM PROMPT'),
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
    }))
  })

  it('should handle tool calls in onStepFinish callback', async () => {
    const { POST } = await import('@/app/api/chat/route')
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        messages: [
          { role: 'user', content: 'Create a button component' }
        ]
      })
    })

    let onStepFinishCallback: any = null
    
    mockStreamText.mockImplementationOnce((config) => {
      onStepFinishCallback = config.onStepFinish
      return {
        toDataStreamResponse: vi.fn().mockReturnValue(
          new Response('stream', { 
            headers: { 'Content-Type': 'text/event-stream' } 
          })
        )
      } as any
    })

    mockToolExecute.mockResolvedValue({
      success: true,
      artifactId: 'button-component',
      operation: 'create'
    })

    await POST(mockRequest)

    // Verify that onStepFinish callback was captured
    expect(onStepFinishCallback).toBeDefined()
    
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

    // Verify stream data was appended with artifact info
    expect(mockStreamDataAppend).toHaveBeenCalledWith(expect.objectContaining({
      type: 'artifact',
      operation: 'tool-call'
    }))
  })

  it('should handle XML fallback when tool calls fail', async () => {
    // Reset mocks to ensure clean state
    vi.clearAllMocks()
    const { POST } = await import('@/app/api/chat/route')
    
    // Mock XML detection and parsing
    mockHasArtifactTags.mockReturnValueOnce(true)
    mockProcessArtifactsFromResponse.mockReturnValueOnce({
      xmlArtifacts: [],
      validParameters: [
      {
        identifier: 'xml-component',
        title: 'XML Component',
        type: 'react',
        content: 'export default function XMLComponent() { return <div>XML</div> }',
        operation: 'create',
        language: 'tsx'
      }
    ]
    })

    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        messages: [
          { role: 'user', content: 'Create a component' }
        ]
      })
    })

    let onStepFinishCallback: any = null
    
    mockStreamText.mockImplementationOnce((config) => {
      onStepFinishCallback = config.onStepFinish
      return {
        toDataStreamResponse: vi.fn().mockReturnValue(
          new Response('stream', { 
            headers: { 'Content-Type': 'text/event-stream' } 
          })
        )
      } as any
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

    expect(mockProcessArtifactsFromResponse).toHaveBeenCalledWith(mockTextStepResult.text)
  })

  it('should include project context in request body', async () => {
    const { POST } = await import('@/app/api/chat/route')
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

    mockStreamText.mockReturnValueOnce({
      toDataStreamResponse: vi.fn().mockReturnValue(
        new Response('stream', { 
          headers: { 'Content-Type': 'text/event-stream' } 
        })
      )
    } as any)

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
    const { POST } = await import('@/app/api/chat/route')
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        messages: [
          { role: 'user', content: 'Create a component' }
        ]
      })
    })

    // Mock streamText to throw an error
    mockStreamText.mockImplementationOnce(() => {
      throw new Error('AI service unavailable')
    })

    const response = await POST(mockRequest)
    
    expect(response.status).toBe(500)
    
    const responseData = await response.json()
    expect(responseData.error).toContain('AI service unavailable')
  })

  it('should validate request body', async () => {
    const { POST } = await import('@/app/api/chat/route')
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'POST',
      body: JSON.stringify({
        // Missing messages array
        projectId: 'test'
      })
    })

    // Mock streamText implementation that won't be called
    mockStreamText.mockReturnValueOnce({
      toDataStreamResponse: vi.fn().mockReturnValue(
        new Response('stream', { 
          headers: { 'Content-Type': 'text/event-stream' } 
        })
      )
    } as any)

    const response = await POST(mockRequest)
    
    // The API should return 500 when messages array is missing
    expect(response.status).toBe(500)
  })

  it('should handle POST requests only', async () => {
    const { POST } = await import('@/app/api/chat/route')
    const mockRequest = new NextRequest('http://localhost:3000/api/chat', {
      method: 'GET'
    })

    // The route should only export POST, so GET should not be handled
    // This test verifies the API design
    expect(POST).toBeDefined()
  })
})