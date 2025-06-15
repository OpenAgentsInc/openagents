# @openagentsinc/pylon

The first application built with the OpenAgents SDK, showcasing WebTUI components.

## Overview

Pylon is a demonstration web application that:
- Uses the new OpenAgents SDK (coming soon)
- Showcases WebTUI terminal-inspired UI components from `@openagentsinc/ui`
- Built with Vite for fast development

## Development

```bash
# Install dependencies
pnpm install

# Build the UI package first (required)
pnpm --filter=@openagentsinc/ui build

# Start development server
pnpm --filter=@openagentsinc/pylon dev

# Build for production
pnpm --filter=@openagentsinc/pylon build

# Preview production build
pnpm --filter=@openagentsinc/pylon preview
```

## Architecture

- **Vite** - Fast build tool and dev server
- **WebTUI CSS** - Terminal-inspired components from `@openagentsinc/ui`
- **OpenAgents SDK** - Core functionality (coming soon)

## License

CC0-1.0