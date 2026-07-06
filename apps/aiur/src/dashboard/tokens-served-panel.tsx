import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

import { useTokensServedLive } from './use-tokens-served-live'

export function TokensServedPanel() {
  const { status, snapshot, errorMessage } = useTokensServedLive()

  return (
    <Card data-testid="tokens-served-panel">
      <CardHeader>
        <CardTitle>Khala Tokens Served</CardTitle>
        <CardDescription>
          Live Khala Sync scope <code>scope.public.tokens-served</code> — proof
          that Aiur is genuinely connected to the same Khala Sync engine as
          every other OpenAgents surface, not a mock.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {status === 'connecting' && (
          <p className="font-mono text-sm text-khala-text-muted" data-testid="tokens-served-status">
            Connecting to Khala Sync...
          </p>
        )}
        {status === 'error' && (
          <p className="font-mono text-sm text-khala-danger" data-testid="tokens-served-status">
            Khala Sync connection error{errorMessage ? `: ${errorMessage}` : ''}
          </p>
        )}
        {snapshot !== undefined && (
          <div data-testid="tokens-served-value">
            <p className="font-mono text-3xl font-semibold text-khala-text">
              {snapshot.total.toLocaleString()}
            </p>
            {snapshot.lastEventAt !== null && (
              <p className="mt-1 text-xs text-khala-text-faint">
                Last event: {snapshot.lastEventAt}
              </p>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
