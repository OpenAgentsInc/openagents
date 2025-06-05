import { useState, useEffect } from 'react'
import { Button } from '@openagentsinc/ui/web/components/button'
import { Alert, AlertDescription, AlertTitle } from '@openagentsinc/ui/web/components/alert'
import { Bot } from 'lucide-react'
import { Effect } from 'effect'
import * as Ai from '@openagentsinc/ai'

export function AiDemo() {
  const [aiResponse, setAiResponse] = useState<string>('')
  const [aiLoading, setAiLoading] = useState(false)

  // AI Service test
  const testAiService = async () => {
    console.log('Testing AI Service...')
    setAiLoading(true)
    try {
      const program = Ai.AiService.hello('Playground')
      
      const result = await Effect.runPromise(
        program.pipe(Effect.provide(Ai.AiService.AiServiceLive))
      )
      console.log('AI Service Response:', result)
      setAiResponse(String(result))
    } catch (error) {
      console.error('AI Service Error:', error)
      setAiResponse(`Error: ${error}`)
    } finally {
      setAiLoading(false)
    }
  }

  // Run AI test on mount
  useEffect(() => {
    testAiService()
  }, [])

  return (
    <div className="space-y-6">
      {/* AI Service Demo */}
      <div className="space-y-4">
        <h3 className="text-lg font-mono">AI Service Integration</h3>
        <Alert>
          <Bot className="h-4 w-4" />
          <AlertTitle>@openagentsinc/ai Package</AlertTitle>
          <AlertDescription>
            Testing the AI service integration from our new Effect-based AI package.
          </AlertDescription>
        </Alert>
        
        <div className="space-y-4">
          <div className="p-4 bg-muted rounded-lg">
            <p className="text-sm font-mono mb-2">AI Service Response:</p>
            {aiLoading ? (
              <p className="text-muted-foreground">Loading...</p>
            ) : (
              <p className="font-semibold">{aiResponse || 'No response yet'}</p>
            )}
          </div>
          
          <Button 
            onClick={testAiService} 
            disabled={aiLoading}
            variant="outline"
          >
            {aiLoading ? 'Testing...' : 'Test AI Service Again'}
          </Button>
          
          <div className="text-sm text-muted-foreground">
            <p>This demonstrates the hello world export from @openagentsinc/ai</p>
            <p>The service is using Effect patterns for dependency injection.</p>
          </div>
        </div>
      </div>
    </div>
  )
}