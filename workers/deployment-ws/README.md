# OpenAgents Deployment WebSocket Service

Real-time WebSocket service for deployment progress tracking, built with Cloudflare Workers and Durable Objects.

## Features

- Real-time deployment status updates via WebSocket
- Persistent connection management with Durable Objects
- Automatic reconnection support
- Mock deployment simulation for testing
- Internal API for deployment service integration

## Setup

```bash
# Install dependencies
npm install

# Run locally
npm run dev
```

## API Endpoints

### WebSocket Connection
```
ws://localhost:8787/?deploymentId=<deployment-id>
```

### Health Check
```
GET /health
```

### Test Deployment (Development Only)
```
POST /test/deploy
{
  "deploymentId": "deploy-123",
  "projectName": "my-project"
}
```

### Internal Update API
```
POST /internal/deployments/<deployment-id>/update
Authorization: Bearer <api-key>
{
  "status": {
    "id": "deploy-123",
    "status": "building",
    "progress": 50,
    "stage": "Building project",
    "message": "Compiling TypeScript..."
  }
}
```

## WebSocket Message Protocol

### Client → Server
```json
{
  "type": "ping"
}
```

### Server → Client
```json
{
  "type": "deployment_update",
  "data": {
    "id": "deploy-123",
    "projectId": "my-project",
    "status": "building",
    "progress": 50,
    "stage": "Building project",
    "message": "Compiling TypeScript...",
    "timestamp": 1703001234567,
    "logs": ["[2024-12-20T10:00:00Z] Starting build..."],
    "deploymentUrl": "https://my-project.openagents.dev"
  }
}
```

## Deployment

### Development
```bash
npm run dev
```

### Production
```bash
npm run deploy:production
```

## Environment Variables

- `ENVIRONMENT` - "development" or "production"
- `INTERNAL_API_KEY` - API key for internal update endpoint

## Architecture

```
┌─────────────────┐       ┌─────────────────┐       ┌─────────────────┐
│   Client App    │  ws:// │  Worker Gateway │       │ Durable Object  │
│ (DeploymentTracker) ──→ │   (index.ts)    │  ──→  │ (DeploymentSession) │
└─────────────────┘       └─────────────────┘       └─────────────────┘
                                   ↑
                                   │ HTTP POST
                          ┌─────────────────┐
                          │ Deployment Service │
                          │ (Updates status)   │
                          └─────────────────┘
```

## Testing

1. Start the Worker locally:
   ```bash
   npm run dev
   ```

2. Connect WebSocket client:
   ```javascript
   const ws = new WebSocket('ws://localhost:8787/?deploymentId=test-123');
   ws.onmessage = (event) => console.log(JSON.parse(event.data));
   ```

3. Trigger mock deployment:
   ```bash
   curl -X POST http://localhost:8787/test/deploy \
     -H "Content-Type: application/json" \
     -d '{"deploymentId":"test-123","projectName":"my-app"}'
   ```

## Monitoring

- Use `wrangler tail` to stream logs
- Cloudflare dashboard for metrics and analytics
- Durable Object inspector for debugging state