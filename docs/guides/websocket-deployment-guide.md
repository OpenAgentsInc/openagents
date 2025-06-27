# WebSocket Deployment Infrastructure Guide

This guide covers the real-time WebSocket infrastructure for deployment tracking in OpenAgents, built with Cloudflare Workers and Durable Objects.

## Table of Contents
- [Overview](#overview)
- [Architecture](#architecture)
- [Local Development](#local-development)
- [Production Deployment](#production-deployment)
- [Integration](#integration)
- [Testing](#testing)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

## Overview

The WebSocket infrastructure provides real-time deployment progress tracking for templates and projects. It consists of:

1. **Cloudflare Worker** - WebSocket gateway handling connections
2. **Durable Objects** - Stateful session management per deployment
3. **Client Library** - Auto-reconnecting WebSocket client with React hooks
4. **Mock Simulator** - Development testing without real deployments

## Architecture

```
┌─────────────────┐     wss://        ┌─────────────────────┐
│   Next.js App   │ ──────────────> │ Cloudflare Worker   │
│ (DeploymentTracker) websocket     │   (Gateway)         │
└─────────────────┘                  └──────────┬──────────┘
                                                 │
                                                 ▼
                                    ┌─────────────────────────┐
                                    │   Durable Object       │
                                    │ (DeploymentSession)    │
                                    │  - Manages connections │
                                    │  - Broadcasts updates  │
                                    └───────────┬─────────────┘
                                                │
                                                ▼
                                    ┌─────────────────────────┐
                                    │  Deployment Service     │
                                    │ (Sends status updates)  │
                                    └─────────────────────────┘
```

### Message Flow

1. Client connects with `?deploymentId=xxx`
2. Worker routes to Durable Object for that deployment
3. Deployment service sends updates via internal API
4. Durable Object broadcasts to all connected clients
5. Clients receive real-time progress updates

## Local Development

### 1. Install Dependencies

```bash
cd workers/deployment-ws
npm install
```

### 2. Start Local Worker

```bash
npm run dev
```

This starts the Worker on `http://localhost:8787` with:
- WebSocket endpoint: `ws://localhost:8787/?deploymentId=xxx`
- Health check: `http://localhost:8787/health`
- Test deployment: `POST http://localhost:8787/test/deploy`

### 3. Configure Next.js App

Create `.env.local`:
```env
NEXT_PUBLIC_DEPLOYMENT_WS_URL=ws://localhost:8787
```

### 4. Test Mock Deployment

```bash
# Trigger a mock deployment
curl -X POST http://localhost:8787/test/deploy \
  -H "Content-Type: application/json" \
  -d '{
    "deploymentId": "test-123",
    "projectName": "my-app"
  }'
```

## Production Deployment

### Prerequisites

1. Cloudflare account with Workers enabled
2. Wrangler CLI installed and authenticated
3. Custom domain configured (optional)

### 1. Configure Production Environment

Edit `wrangler.toml`:
```toml
[env.production]
routes = [
  { pattern = "api.openagents.com/deployment-ws/*", custom_domain = true }
]
vars = { 
  ENVIRONMENT = "production",
  INTERNAL_API_KEY = "your-secure-api-key"
}
```

### 2. Deploy to Cloudflare

```bash
cd workers/deployment-ws

# Login to Cloudflare
wrangler login

# Deploy to production
npm run deploy:production
```

### 3. Configure DNS

In Cloudflare Dashboard:
1. Add CNAME record: `api.openagents.com` → `your-worker.workers.dev`
2. Enable "Proxied" for SSL/TLS
3. Configure SSL/TLS mode to "Full"

### 4. Update Production Environment

In your Next.js app deployment (Vercel):
```env
NEXT_PUBLIC_DEPLOYMENT_WS_URL=wss://api.openagents.com/deployment-ws
```

## Integration

### Client-Side Integration

```typescript
// Using the DeploymentTracker component
import { DeploymentTracker } from '@/components/DeploymentTracker'

function TemplateDeployment() {
  const [deploymentId, setDeploymentId] = useState<string | null>(null)
  
  const handleDeploy = () => {
    const id = `deploy-${Date.now()}`
    setDeploymentId(id)
    // Start actual deployment process
  }
  
  return (
    <>
      <button onClick={handleDeploy}>Deploy</button>
      
      {deploymentId && (
        <DeploymentTracker
          deploymentId={deploymentId}
          projectName="My Project"
          onComplete={(url) => console.log('Deployed to:', url)}
        />
      )}
    </>
  )
}
```

### Server-Side Integration

Send deployment updates from your deployment service:

```typescript
// Update deployment status
async function updateDeploymentStatus(
  deploymentId: string,
  status: DeploymentStatus
) {
  const response = await fetch(
    `https://api.openagents.com/deployment-ws/internal/deployments/${deploymentId}/update`,
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${process.env.INTERNAL_API_KEY}`
      },
      body: JSON.stringify({ status })
    }
  )
  
  if (!response.ok) {
    throw new Error('Failed to update deployment status')
  }
}

// Example usage during deployment
await updateDeploymentStatus('deploy-123', {
  id: 'deploy-123',
  projectId: 'my-project',
  status: 'building',
  progress: 50,
  stage: 'Building project',
  message: 'Compiling TypeScript...',
  timestamp: Date.now(),
  logs: ['npm install completed', 'Building...']
})
```

## Testing

### Unit Tests

The WebSocket infrastructure includes:
- Mock deployment simulator
- Automatic fallback when WebSocket unavailable
- Test endpoints for development

### Integration Testing

1. **Test WebSocket Connection**:
```javascript
const ws = new WebSocket('wss://api.openagents.com/deployment-ws?deploymentId=test-123')

ws.onopen = () => console.log('Connected')
ws.onmessage = (event) => {
  const message = JSON.parse(event.data)
  console.log('Update:', message)
}
```

2. **Test Internal API**:
```bash
curl -X POST https://api.openagents.com/deployment-ws/internal/deployments/test-123/update \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "status": {
      "id": "test-123",
      "status": "success",
      "progress": 100,
      "deploymentUrl": "https://test.openagents.dev"
    }
  }'
```

## Monitoring

### Cloudflare Analytics

Monitor in Cloudflare Dashboard:
- Request count and WebSocket connections
- Error rates and status codes
- Geographic distribution
- Bandwidth usage

### Real-time Logs

```bash
# Stream live logs
wrangler tail --env production

# Filter for specific deployment
wrangler tail --env production --search "deploy-123"
```

### Durable Object Metrics

View in Cloudflare Dashboard:
- Active Durable Objects count
- Storage usage
- Request/response times
- WebSocket message volume

## Troubleshooting

### Common Issues

#### WebSocket Won't Connect

1. **Check CORS headers** - Worker includes permissive CORS by default
2. **Verify deployment ID** - Must be provided as query parameter
3. **Check SSL/TLS** - Production requires `wss://` not `ws://`
4. **Firewall rules** - Ensure WebSocket upgrade allowed

#### Messages Not Received

1. **Check connection status** - Use browser DevTools Network tab
2. **Verify message format** - Must be valid JSON
3. **Check Durable Object** - Use `wrangler tail` to see logs
4. **Test with mock** - Fallback to mock simulator

#### High Latency

1. **Check region** - Durable Objects have home region
2. **Monitor connection count** - Too many connections per DO
3. **Review message size** - Keep updates small
4. **Check heartbeat** - Ensure ping/pong working

### Debug Commands

```bash
# Check Worker status
wrangler status

# View recent errors
wrangler tail --env production --status error

# Test health endpoint
curl https://api.openagents.com/deployment-ws/health

# Check Durable Object state
wrangler tail --env production --search "DeploymentSession"
```

### Environment Variables

Ensure these are set correctly:

```toml
# wrangler.toml
[env.production]
vars = {
  ENVIRONMENT = "production",
  INTERNAL_API_KEY = "secure-key-here"
}

[[durable_objects.bindings]]
name = "DEPLOYMENT_SESSIONS"
class_name = "DeploymentSession"
```

## Best Practices

1. **Connection Management**
   - Reuse WebSocket connections when possible
   - Implement exponential backoff for reconnection
   - Clean up connections on component unmount

2. **Message Format**
   - Keep messages small and focused
   - Use consistent status enums
   - Include timestamps for ordering

3. **Error Handling**
   - Graceful fallback to polling if WebSocket fails
   - Log errors with deployment context
   - Provide user feedback for connection issues

4. **Security**
   - Use secure API key for internal updates
   - Validate deployment IDs
   - Rate limit connections per IP

5. **Performance**
   - Batch updates when possible
   - Use WebSocket hibernation
   - Monitor Durable Object memory usage

## Future Enhancements

1. **Authentication** - Add JWT validation for connections
2. **Compression** - Enable WebSocket compression
3. **Analytics** - Track deployment success rates
4. **Replays** - Store and replay deployment history
5. **Scaling** - Multiple Durable Objects per deployment

## Resources

- [Cloudflare Workers Documentation](https://developers.cloudflare.com/workers/)
- [Durable Objects Guide](https://developers.cloudflare.com/workers/learning/using-durable-objects/)
- [WebSocket API Reference](https://developers.cloudflare.com/workers/examples/websockets/)
- [Wrangler CLI](https://developers.cloudflare.com/workers/wrangler/)

---

For questions or issues, please open a GitHub issue in the OpenAgents repository.