# openagents.com

The official OpenAgents website, built with Psionic hypermedia framework.

## Development

### Prerequisites

- Bun runtime installed
- Run `pnpm install` from the repository root

### Running the development server

From the repository root:

```bash
pnpm --filter=@openagentsinc/openagents.com dev
```

Or from this directory:

```bash
pnpm run dev
```

The site will be available at http://localhost:3003

## Architecture

This is a Psionic application demonstrating:

- Server-rendered hypermedia pages
- Minimal client-side JavaScript
- Real-time updates (coming soon with WebSocket integration)
- Integration with OpenAgents SDK

## Pages

- `/` - Landing page introducing OpenAgents
- `/agents` - Agent marketplace (currently showing mock data)
- `/docs` - Documentation and quick start guide
- `/about` - Mission and technology overview

## Deployment

Coming soon - this will be deployed as the main openagents.com website.