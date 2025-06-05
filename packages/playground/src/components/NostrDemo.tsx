import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@openagentsinc/ui/web/components/card'
import { Alert, AlertDescription } from '@openagentsinc/ui/web/components/alert'

// Full Nostr Demo component - currently disabled due to type issues
// This will be enabled once the Nostr package types are properly exported
export function NostrDemo() {
  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle>Nostr Protocol Demo (Full)</CardTitle>
        <CardDescription>
          Complete implementation with real Nostr protocol
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Alert>
          <AlertDescription>
            The full Nostr demo is temporarily disabled while we resolve TypeScript type export issues.
            Please use the simplified demo for now.
          </AlertDescription>
        </Alert>
      </CardContent>
    </Card>
  )
}