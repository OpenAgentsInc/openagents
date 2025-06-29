import { streamText, StreamData } from 'ai'
import { openrouter } from '@openrouter/ai-sdk-provider'
import { createArtifactTool } from '@/lib/tools/createArtifactTool'
import { processArtifactsFromResponse, hasArtifactTags } from '@/lib/tools/xmlArtifactParser'
import { COMPLETE_SYSTEM_PROMPT } from '@/lib/prompts/artifactSystemPrompt'

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

// Enhanced system prompt for code generation and project assistance with artifact support
const getSystemPrompt = (projectName?: string, framework?: string) => `
${COMPLETE_SYSTEM_PROMPT}

# Project Context
${projectName ? `You are currently working on a project called "${projectName}".` : ''}
${framework ? `The project uses ${framework} framework.` : ''}

# OpenAgents Capabilities
- Create artifacts for substantial code using the createArtifact tool
- Deploy artifacts to live URLs with one click
- Generate complete, production-ready applications
- Support for React, HTML, JavaScript, TypeScript, Python, and more

# Project-Specific Guidelines
- Follow existing project structure and conventions
- Use TypeScript when applicable for this project
- Include necessary imports and dependencies
- Provide working, deployable code

When users request applications or substantial code, use the createArtifact tool to create deployable artifacts.
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
      model: openrouter('anthropic/claude-3.5-sonnet'),
      system: systemPrompt,
      messages: enhancedMessages,
      maxTokens: 4000,
      temperature: 0.7,
      tools: {
        createArtifact: createArtifactTool
      },
      maxSteps: 3, // Allow for tool calling and follow-up
      onStepFinish: async (stepResult) => {
        // Handle tool calls
        if (stepResult.toolCalls && stepResult.toolCalls.length > 0) {
          for (const toolCall of stepResult.toolCalls) {
            if (toolCall.toolName === 'createArtifact') {
              data.append({
                type: 'artifact',
                operation: 'tool-call',
                artifact: {
                  ...toolCall.args,
                  toolCallId: toolCall.toolCallId
                },
                timestamp: new Date().toISOString()
              })
            }
          }
        }

        // Check for XML artifacts in text content as fallback
        if (stepResult.text && hasArtifactTags(stepResult.text)) {
          const { validParameters } = processArtifactsFromResponse(stepResult.text)
          
          for (const params of validParameters) {
            data.append({
              type: 'artifact',
              operation: 'xml-fallback',
              artifact: params,
              timestamp: new Date().toISOString()
            })
          }
        }
      }
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
