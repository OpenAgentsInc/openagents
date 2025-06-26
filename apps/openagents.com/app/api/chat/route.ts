import { streamText } from 'ai'
import { openrouter } from '@openrouter/ai-sdk-provider'

export async function POST(req: Request) {
  try {
    const { messages } = await req.json()

    if (!process.env.OPENROUTER_API_KEY) {
      console.error('OPENROUTER_API_KEY is not configured')
      return new Response('OPENROUTER_API_KEY is not configured', { status: 500 })
    }
    
    const result = streamText({
      model: openrouter('openai/gpt-4o-mini'),
      system: 'You are a computer terminal. Never ask questions. Do not mention being an AI or language model. Respond like a computer system. Use short, direct statements like "Awaiting instructions" or "Processing complete". No markdown formatting.',
      messages,
      maxTokens: 1000,
    })

    return result.toDataStreamResponse()
  } catch (error) {
    console.error('Chat API error:', error)
    return new Response(error instanceof Error ? error.message : 'Unknown error', { status: 500 })
  }
}
