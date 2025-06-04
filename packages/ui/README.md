# @openagentsinc/ui

Shared UI components for OpenAgents applications, extracted from the Commander project.

## Features

- **Pane System**: Draggable and resizable floating windows
- **Hotbar**: Bottom navigation with keyboard shortcuts
- **Modern Stack**: React 19, Tailwind CSS v4, TypeScript
- **Platform Ready**: Designed for future mobile support

## Installation

```bash
pnpm add @openagentsinc/ui
```

## Usage

### Pane System

```tsx
import { PaneManager, Pane } from '@openagentsinc/ui/web'

function App() {
  return (
    <PaneManager>
      <Pane id="main" title="Main Window">
        Your content here
      </Pane>
    </PaneManager>
  )
}
```

### Hotbar

```tsx
import { Hotbar, HotbarItem } from '@openagentsinc/ui/web'

function App() {
  return (
    <Hotbar>
      <HotbarItem slot={1} onClick={() => console.log('Item 1')}>
        Item 1
      </HotbarItem>
    </Hotbar>
  )
}
```

## Architecture

The package is structured for multi-platform support:

```
src/
├── core/           # Platform-agnostic logic
│   ├── state/      # Zustand stores
│   ├── types/      # TypeScript definitions
│   └── utils/      # Shared utilities
├── web/            # Web-specific components
│   ├── components/ # React components
│   ├── styles/     # Tailwind CSS
│   └── hooks/      # Web hooks
└── mobile/         # Future mobile support
```

## Development

```bash
# Install dependencies
pnpm install

# Type checking
pnpm check

# Build
pnpm build

# Test
pnpm test

# Generate exports
pnpm codegen
```

## License

CC0-1.0