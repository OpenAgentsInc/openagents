import { streamText, StreamData } from 'ai'
import { openrouter } from '@openrouter/ai-sdk-provider'

interface ChatRequest {
  messages: Array<{
    role: 'user' | 'assistant' | 'system'
    content: string
  }>
  projectId?: string
  projectName?: string
  context?: {
    files?: Array<{ path: string, content: string }>
    framework?: string
    deploymentUrl?: string
  }
}

// Enhanced system prompt for code generation and project assistance
const getSystemPrompt = (projectName?: string, framework?: string) => `
You are an expert AI developer assistant for OpenAgents, helping users build and deploy web applications.

${projectName ? `You are currently working on a project called "${projectName}".` : ''}
${framework ? `The project uses ${framework} framework.` : ''}

Your role:
- Help users build, debug, and enhance their web applications
- Provide clear, actionable code suggestions and solutions
- Generate complete, functional code when requested
- Explain technical concepts in a helpful way
- Guide users through deployment and project management

Guidelines:
- Always provide working, production-ready code
- Use modern best practices and patterns
- Include proper error handling
- Make code readable with clear comments when helpful
- Consider performance and security
- Be concise but thorough
- Format code properly with syntax highlighting

When generating code:
- Provide complete files or clear code blocks
- Include necessary imports and dependencies
- Use TypeScript when applicable
- Follow the existing project structure and conventions

Current capabilities:
- Code generation and editing
- Project structure guidance
- Debugging assistance
- Deployment help
- Framework-specific advice
`

export async function POST(req: Request) {
  const data = new StreamData()
  
  try {
    const body: ChatRequest = await req.json()
    const { messages, projectId, projectName, context } = body

    if (!process.env.OPENROUTER_API_KEY) {
      console.error('OPENROUTER_API_KEY is not configured')
      data.append({ error: 'API key not configured' })
      return new Response('OPENROUTER_API_KEY is not configured', { status: 500 })
    }

    // Add project context to the system prompt
    const systemPrompt = getSystemPrompt(projectName, context?.framework)
    
    // Enhance messages with project context
    const enhancedMessages = [
      ...(context?.files?.length ? [{
        role: 'system' as const,
        content: `Current project files:\n${context.files.map(f => `${f.path}:\n\`\`\`\n${f.content}\n\`\`\``).join('\n\n')}`
      }] : []),
      ...messages
    ]

    // Stream progress updates
    data.append({ 
      type: 'status', 
      status: 'connecting',
      timestamp: new Date().toISOString()
    })
    
    const result = streamText({
      model: openrouter('openai/gpt-4o-mini'),
      system: systemPrompt,
      messages: enhancedMessages,
      maxTokens: 2000,
      temperature: 0.7
    })

    // Add metadata for client consumption
    data.append({ 
      type: 'metadata'
    })

    return result.toDataStreamResponse({ data })
    
  } catch (error) {
    console.error('Chat API error:', error)
    
    const errorMessage = error instanceof Error ? error.message : 'Unknown error'
    data.append({ 
      type: 'error', 
      error: errorMessage,
      timestamp: new Date().toISOString()
    })
    
    return new Response(JSON.stringify({ 
      error: errorMessage,
      timestamp: new Date().toISOString()
    }), { 
      status: 500,
      headers: { 'Content-Type': 'application/json' }
    })
  }
}
