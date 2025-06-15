# Typed Storybook Integration Analysis

## Directory Structure

The Typed storybook example has this structure:
```
/Users/christopherdavid/code/typed/examples/storybook/
├── .storybook/
│   ├── main.js
│   └── preview.js
├── stories/
│   └── Link.stories.ts
├── package.json
├── tsconfig.json
├── tsconfig.build.json
└── vite.config.mts
```

## Key Findings

### 1. Custom Storybook Framework

Typed uses a **custom Storybook framework** (`@typed/storybook`) instead of the standard React/HTML frameworks. This is configured in `.storybook/main.js`:

```javascript
framework: {
  name: dirname(require.resolve("@typed/storybook/package.json")),
  options: {},
}
```

### 2. Custom Renderer with Effect Runtime

The `@typed/storybook` package provides a custom renderer (`renderToCanvas`) that:
- Sets up Effect runtime with proper layers
- Handles Typed components which return `Fx<RenderEvent, E, R | CoreDomServices>`
- Provides essential services like routing, navigation, render queue, and random values
- Properly handles Effect fibers and interruption

Key code from `renderToCanvas.ts`:
```typescript
const program = renderToLayer(renderable).pipe(
  Layer.provideMerge(server("/")),
  Layer.provideMerge(initialMemory({ url: "/" })),
  Layer.provideMerge(renderLayer(window, { rootElement })),
  Layer.provide(RenderQueue.mixed()),
  Layer.provideMerge(getRandomValues),
  Layer.launch,
  Effect.catchAllCause(onCause)
)

const fiber = Effect.runFork(program)
```

### 3. Story Format

Stories use Typed-specific types and render functions:

```typescript
import type { Meta, StoryObj } from "@typed/storybook"
import { Link } from "@typed/ui"

const meta = {
  component: Link
} satisfies Meta<Args>

export const Example: Story = {
  render: (args) => Link({ to: args.to }, args.label),
  args: {
    to: "/foo",
    label: "Hello, World!"
  }
}
```

### 4. Component Type Definition

Typed components are defined as:
```typescript
type TypedComponent<Props = any, R = any, E = any> = (
  props: Props,
  ...children: ReadonlyArray<Renderable<CoreDomServices, E>>
) => Fx<RenderEvent, E, R | CoreDomServices>
```

### 5. Dependencies

The `@typed/storybook` package depends on:
- All Typed core packages (fx, template, router, navigation, etc.)
- Storybook builder-vite and core packages
- Effect runtime

## Key Differences from Our Current Approach

1. **Custom Framework**: We're using `@storybook/react-vite`, they use a custom `@typed/storybook` framework
2. **Renderer**: We're trying to render React components, they have a custom renderer for Typed components
3. **Effect Runtime**: Their renderer sets up the complete Effect runtime with all necessary layers
4. **Component Return Type**: Their components return `Fx<RenderEvent, ...>`, not React elements
5. **Story Types**: They import from `@typed/storybook`, not `@storybook/react`

## Solution for Our Implementation

To properly integrate Typed components with Storybook, we need to either:

1. **Use the @typed/storybook package** (if available in our monorepo)
2. **Create our own renderer** that:
   - Sets up Effect runtime with necessary layers
   - Handles the `Fx<RenderEvent, ...>` return type
   - Provides all required services (routing, navigation, render queue, etc.)
   - Properly manages Effect fibers

The current approach of trying to render Typed components as React components won't work because they have fundamentally different return types and runtime requirements.