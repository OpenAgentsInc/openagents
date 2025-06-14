# Typed-Storybook Integration: Complete Setup Guide

This document provides an exhaustive guide for setting up Storybook integration with the Typed framework in a monorepo environment. This setup creates a custom Storybook renderer that integrates deeply with Effect and the Typed component model.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Prerequisites](#prerequisites)
3. [Package Structure](#package-structure)
4. [Step-by-Step Setup](#step-by-step-setup)
5. [Configuration Files](#configuration-files)
6. [TypeScript Integration](#typescript-integration)
7. [Writing Stories](#writing-stories)
8. [Advanced Configuration](#advanced-configuration)
9. [Troubleshooting](#troubleshooting)
10. [Migration Notes](#migration-notes)

## Architecture Overview

The Typed-Storybook integration consists of several key components:

1. **Custom Storybook Renderer**: Implements Storybook's renderer interface for Typed components
2. **Effect Integration**: Uses Effect for error handling, lifecycle management, and dependency injection
3. **Template System**: Integrates with `@typed/template` for rendering
4. **TypeScript Types**: Comprehensive type definitions for type-safe story development
5. **Preset Configuration**: Storybook preset that configures Vite and the custom renderer

### Key Features

- **Effect-based Error Handling**: Sophisticated error display with full Effect `Cause` information
- **Lifecycle Management**: Fiber-based cleanup and resource management
- **Type Safety**: Full TypeScript support with generic constraints
- **Hot Reloading**: Proper cleanup and reinitialization on story changes
- **Service Layer Integration**: Access to DOM, navigation, and routing services

## Prerequisites

Before setting up the Typed-Storybook integration, ensure you have:

- **Node.js**: Version 18+ recommended
- **pnpm**: For workspace management (version 9.0.6+ recommended)
- **TypeScript**: Version 5.6+ for project references and advanced typing
- **Existing Typed Project**: With `@typed/template`, `@typed/fx`, and Effect dependencies

### Required Typed Packages

Your monorepo should already have these core packages:
- `@typed/core`
- `@typed/template` 
- `@typed/fx`
- `@typed/context`
- `@typed/dom`
- `@typed/navigation`
- `@typed/router`
- `effect` (version 3.8.4+)

## Package Structure

The integration requires two main components:

```
your-monorepo/
├── packages/
│   └── storybook/                    # Custom Storybook renderer package
│       ├── src/
│       │   ├── index.ts
│       │   ├── preset.ts
│       │   ├── renderToCanvas.ts
│       │   ├── types.ts
│       │   └── preview.ts
│       ├── package.json
│       ├── tsconfig.json
│       └── tsconfig.build.json
└── examples/
    └── storybook/                    # Storybook application
        ├── .storybook/
        │   ├── main.js
        │   └── preview.js
        ├── stories/
        │   └── *.stories.ts
        ├── package.json
        └── tsconfig.json
```

## Step-by-Step Setup

### Step 1: Create the Storybook Renderer Package

#### 1.1 Package Configuration

Create `packages/storybook/package.json`:

```json
{
  "name": "@your-org/storybook",
  "version": "0.1.0",
  "description": "Storybook integration for Typed framework",
  "publishConfig": {
    "access": "public",
    "directory": "dist"
  },
  "exports": {
    "./*": {
      "require": "./build/cjs/*.js"
    }
  },
  "scripts": {
    "build": "pnpm build-all && pnpm build-pack",
    "build-all": "tsc -b tsconfig.build.json",
    "build-pack": "concurrently \"pnpm build-cjs\" \"pnpm build-mjs\" && pnpm build-post",
    "build-cjs": "babel build/esm --config-file ../../.babel.cjs.json --out-dir build/cjs --out-file-extension .js --source-maps",
    "build-mjs": "babel build/esm --config-file ../../.babel.mjs.json --out-dir build/mjs --out-file-extension .mjs --source-maps",
    "build-post": "build-utils pack-v2",
    "clean": "rimraf build dist docs",
    "test": "vitest",
    "lint": "eslint --ext .ts,.tsx src/**/*",
    "tc": "tsc --noEmit"
  },
  "peerDependencies": {
    "@storybook/builder-vite": "^8.3.5",
    "@storybook/core": "^8.3.5",
    "storybook": "^8.3.5"
  },
  "dependencies": {
    "@storybook/csf": "^0.1.11",
    "@storybook/types": "^8.3.5",
    "@your-org/context": "workspace:*",
    "@your-org/core": "workspace:*",
    "@your-org/dom": "workspace:*",
    "@your-org/fx": "workspace:*",
    "@your-org/navigation": "workspace:*",
    "@your-org/route": "workspace:*",
    "@your-org/router": "workspace:*",
    "@your-org/template": "workspace:*",
    "effect": "^3.8.4"
  },
  "devDependencies": {
    "@storybook/builder-vite": "^8.3.5",
    "@storybook/core": "^8.3.5",
    "storybook": "^8.3.5"
  }
}
```

#### 1.2 Main Entry Point

Create `packages/storybook/src/index.ts`:

```typescript
/**
 * @since 1.0.0
 */

export * from "./preset.js"
export * from "./types.js"
```

#### 1.3 TypeScript Definitions

Create `packages/storybook/src/types.ts`:

```typescript
/**
 * @since 1.0.0
 */

import type { CSFExports, Renderer } from "@storybook/csf"
import type {
  BuilderOptions,
  StorybookConfig as BaseStorybookConfig,
  TypescriptOptions as BaseTypescriptOptions,
  WebRenderer
} from "@storybook/types"
import type { CoreDomServices, RenderEvent } from "@your-org/template"
import type { InlineConfig } from "vite"
import type * as Effect from "effect/Effect"
import type * as Fx from "@your-org/fx/Fx"

/**
 * Custom renderer interface for Typed components in Storybook
 * 
 * @since 1.0.0
 * @category Renderer
 */
export interface TypedRenderer extends WebRenderer {
  component: TypedComponent<any>
  storyResult: Fx.Fx<RenderEvent, any, CoreDomServices>
}

/**
 * Function signature for Typed components
 * 
 * @since 1.0.0
 * @category Component
 */
export interface TypedComponent<Args = {}> {
  (args: Args, ...children: ReadonlyArray<Fx.Fx<RenderEvent, any, any>>): Fx.Fx<RenderEvent, any, CoreDomServices>
}

/**
 * Story metadata with Typed component integration
 * 
 * @since 1.0.0
 * @category Story
 */
export interface Meta<Args = {}> {
  component?: TypedComponent<Args>
  title?: string
  parameters?: Record<string, any>
  argTypes?: Record<keyof Args, any>
  args?: Partial<Args>
  decorators?: ReadonlyArray<(story: () => any, context: any) => any>
  render?: (args: Args) => Fx.Fx<RenderEvent, any, CoreDomServices>
}

/**
 * Individual story configuration
 * 
 * @since 1.0.0
 * @category Story
 */
export interface StoryObj<Args = {}, T = Meta<Args>> {
  args?: Partial<Args>
  argTypes?: Record<keyof Args, any>
  parameters?: Record<string, any>
  render?: (args: Args) => Fx.Fx<RenderEvent, any, CoreDomServices>
  play?: (context: any) => Promise<void> | void
}

/**
 * Storybook configuration with Vite integration
 * 
 * @since 1.0.0
 * @category Configuration
 */
export interface StorybookConfig extends BaseStorybookConfig {
  core?: BaseStorybookConfig["core"] & {
    builder?: BuilderOptions<"@storybook/builder-vite">
  }
  viteFinal?: (config: InlineConfig, options: { configType: "DEVELOPMENT" | "PRODUCTION" }) => InlineConfig | Promise<InlineConfig>
  typescript?: BaseTypescriptOptions
}

/**
 * Framework-specific options
 * 
 * @since 1.0.0
 * @category Configuration
 */
export interface FrameworkOptions {
  builder?: Record<string, any>
}
```

#### 1.4 Storybook Preset Configuration

Create `packages/storybook/src/preset.ts`:

```typescript
/**
 * @since 1.0.0
 */

import type { PresetProperty, Options } from "@storybook/types"
import type { StorybookConfig } from "./types.js"
import { renderToCanvas } from "./renderToCanvas.js"
import { dirname, join } from "path"
import { fileURLToPath } from "url"

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

/**
 * Default addons (none for now)
 * 
 * @since 1.0.0
 * @category Configuration
 */
export const addons: PresetProperty<"addons"> = []

/**
 * Core configuration for the Typed framework
 * 
 * @since 1.0.0
 * @category Configuration
 */
export const core: PresetProperty<"core", StorybookConfig> = {
  builder: "@storybook/builder-vite",
  renderer: "@your-org/storybook"
}

/**
 * Preview annotations (pass-through for now)
 * 
 * @since 1.0.0
 * @category Configuration
 */
export const previewAnnotations: PresetProperty<"previewAnnotations"> = (
  entry: any[] = []
) => {
  return [...entry]
}

/**
 * Vite configuration hook
 * 
 * @since 1.0.0
 * @category Configuration
 */
export const viteFinal: NonNullable<StorybookConfig["viteFinal"]> = async (
  config,
  { configType }
) => {
  // Add any custom Vite configuration here
  return {
    ...config,
    // Example: Add path resolution for your workspace packages
    resolve: {
      ...config.resolve,
      alias: {
        ...config.resolve?.alias,
        // Add aliases for workspace packages if needed
      }
    }
  }
}

// Export the render function for Storybook
export { renderToCanvas }
```

#### 1.5 Custom Renderer Implementation

Create `packages/storybook/src/renderToCanvas.ts`:

```typescript
/**
 * @since 1.0.0
 */

import type { RenderContext, WebRenderer } from "@storybook/types"
import type { TypedRenderer } from "./types.js"
import * as Effect from "effect/Effect"
import * as Cause from "effect/Cause"
import * as Fiber from "effect/Fiber"
import * as Layer from "effect/Layer"
import * as Runtime from "effect/Runtime"
import { CoreDomServices } from "@your-org/dom"
import { CurrentEnvironment } from "@your-org/environment"
import { Navigation } from "@your-org/navigation"
import { CurrentRoute, makeCurrentRoute } from "@your-org/router"
import { RenderQueue, renderToLayer } from "@your-org/template"
import * as Route from "@your-org/route"

/**
 * Global runtime for managing Effect-based rendering
 */
let globalRuntime: Runtime.Runtime<any> | undefined
let currentFiber: Fiber.RuntimeFiber<any, any> | undefined

/**
 * Main render function that integrates Typed components with Storybook
 * 
 * @since 1.0.0
 * @category Rendering
 */
export const renderToCanvas = async (
  renderContext: RenderContext<TypedRenderer>,
  canvasElement: HTMLElement
) => {
  const { storyFn, showError, showMain } = renderContext

  try {
    // Clean up previous render
    if (currentFiber) {
      await Runtime.runPromise(Fiber.interrupt(currentFiber))
      currentFiber = undefined
    }

    // Clear the canvas
    canvasElement.innerHTML = ""

    // Get the story result
    const storyResult = storyFn()

    // Create the Effect for rendering
    const renderEffect = Effect.gen(function* () {
      // Setup the render queue and target element
      const renderQueue = yield* RenderQueue
      yield* renderQueue.setRenderRoot(canvasElement)

      // Render the story
      yield* renderToLayer(storyResult)
    })

    // Create the layer stack for services
    const layer = Layer.mergeAll(
      CoreDomServices.layer,
      RenderQueue.layer,
      // Add routing services for stories that might need them
      Layer.succeed(CurrentRoute, makeCurrentRoute(Route.parse("/"))),
      Navigation.memory(),
      // Environment service
      Layer.succeed(CurrentEnvironment, "browser" as const)
    )

    // Apply the layer to the effect
    const program = Effect.provide(renderEffect, layer)

    // Get or create the runtime
    if (!globalRuntime) {
      globalRuntime = await Effect.runPromise(Effect.runtime<any>())
    }

    // Run the effect and store the fiber for cleanup
    currentFiber = Runtime.runFork(globalRuntime)(program)

    // Handle the result
    await Runtime.runPromise(
      Effect.gen(function* () {
        const result = yield* Fiber.join(currentFiber!)
        showMain()
        return result
      }).pipe(
        Effect.catchAll((cause) => {
          // Display Effect errors in Storybook's error panel
          const errorMessage = Cause.pretty(cause)
          const error = new Error(`Story render failed: ${errorMessage}`)
          
          // Add cause information to the error for debugging
          ;(error as any).cause = cause
          
          showError(error)
          return Effect.void
        })
      )
    )
  } catch (error) {
    // Handle any non-Effect errors
    console.error("Storybook render error:", error)
    showError(error instanceof Error ? error : new Error(String(error)))
  }
}

/**
 * Cleanup function called when Storybook unmounts
 * 
 * @since 1.0.0
 * @category Lifecycle
 */
export const cleanup = async () => {
  if (currentFiber) {
    try {
      if (globalRuntime) {
        await Runtime.runPromise(Fiber.interrupt(currentFiber))
      }
    } catch (error) {
      console.warn("Error during Storybook cleanup:", error)
    } finally {
      currentFiber = undefined
    }
  }
}

// Register cleanup on page unload
if (typeof window !== "undefined") {
  window.addEventListener("beforeunload", cleanup)
  
  // Also cleanup on hot reload (development)
  if (import.meta.hot) {
    import.meta.hot.dispose(cleanup)
  }
}
```

#### 1.6 Preview Export

Create `packages/storybook/src/preview.ts`:

```typescript
/**
 * @since 1.0.0
 */

export { renderToCanvas } from "./renderToCanvas.js"
```

#### 1.7 TypeScript Configuration

Create `packages/storybook/tsconfig.json`:

```json
{
  "extends": "../../tsconfig/base.json",
  "compilerOptions": {
    "outDir": "build/esm",
    "declarationDir": "build/dts"
  },
  "references": [
    { "path": "../context" },
    { "path": "../core" },
    { "path": "../dom" },
    { "path": "../fx" },
    { "path": "../navigation" },
    { "path": "../route" },
    { "path": "../router" },
    { "path": "../template" }
  ]
}
```

Create `packages/storybook/tsconfig.build.json`:

```json
{
  "extends": "./tsconfig.json",
  "exclude": ["test/**/*", "**/*.test.ts", "**/*.spec.ts"]
}
```

### Step 2: Create the Storybook Example Application

#### 2.1 Example Package Configuration

Create `examples/storybook/package.json`:

```json
{
  "name": "@your-org/example-storybook",
  "private": true,
  "version": "0.1.0",
  "description": "Storybook example for Typed components",
  "scripts": {
    "storybook": "storybook dev -p 6006",
    "build-storybook": "storybook build"
  },
  "dependencies": {
    "@your-org/core": "workspace:*",
    "@your-org/fx": "workspace:*",
    "@your-org/storybook": "workspace:*",
    "@your-org/template": "workspace:*",
    "@your-org/ui": "workspace:*",
    "effect": "^3.8.4"
  },
  "devDependencies": {
    "@storybook/addon-essentials": "^8.3.5",
    "@storybook/addon-interactions": "^8.3.5",
    "@storybook/addon-links": "^8.3.5",
    "@storybook/blocks": "^8.3.5",
    "@storybook/test": "^8.3.5",
    "@vitejs/plugin-react": "^4.3.2",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "storybook": "^8.3.5"
  }
}
```

#### 2.2 Storybook Main Configuration

Create `examples/storybook/.storybook/main.js`:

```javascript
import { dirname, join } from "path"
import { fileURLToPath } from "url"

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

/**
 * This function is used to resolve the absolute path of a package.
 */
function getAbsolutePath(value) {
  return dirname(require.resolve(join(value, "package.json")))
}

/** @type {import('@your-org/storybook').StorybookConfig} */
const config = {
  stories: ["../stories/**/*.stories.@(js|jsx|mjs|ts|tsx)"],
  
  addons: [
    getAbsolutePath("@storybook/addon-links"),
    getAbsolutePath("@storybook/addon-essentials"),
    getAbsolutePath("@storybook/addon-interactions")
  ],
  
  framework: {
    name: getAbsolutePath("@your-org/storybook"),
    options: {}
  },
  
  typescript: {
    check: false,
    reactDocgen: "react-docgen-typescript",
    reactDocgenTypescriptOptions: {
      shouldExtractLiteralValuesFromEnum: true,
      propFilter: (prop) => (prop.parent ? !/node_modules/.test(prop.parent.fileName) : true)
    }
  },
  
  docs: {
    autodocs: "tag"
  },
  
  viteFinal: (config, { configType }) => {
    // Customize Vite config here if needed
    return config
  }
}

export default config
```

#### 2.3 Storybook Preview Configuration

Create `examples/storybook/.storybook/preview.js`:

```javascript
/** @type {import('@your-org/storybook').Meta} */
export const parameters = {
  actions: { argTypesRegex: "^on[A-Z].*" },
  controls: {
    matchers: {
      color: /(background|color)$/i,
      date: /Date$/
    }
  }
}
```

#### 2.4 TypeScript Configuration

Create `examples/storybook/tsconfig.json`:

```json
{
  "extends": "../../tsconfig/base.json",
  "compilerOptions": {
    "allowJs": true,
    "skipLibCheck": true
  },
  "include": [
    "stories/**/*",
    ".storybook/**/*"
  ],
  "references": [
    { "path": "../../packages/storybook" },
    { "path": "../../packages/ui" }
  ]
}
```

### Step 3: Writing Stories

#### 3.1 Basic Story Example

Create `examples/storybook/stories/Button.stories.ts`:

```typescript
import type { Meta, StoryObj } from "@your-org/storybook"
import { Button } from "@your-org/ui"
import { html } from "@your-org/template"

type Args = {
  label: string
  variant: "primary" | "secondary" | "danger"
  disabled: boolean
  onClick: () => void
}

const meta = {
  title: "Components/Button",
  component: Button,
  parameters: {
    layout: "centered"
  },
  tags: ["autodocs"],
  argTypes: {
    variant: {
      control: { type: "select" },
      options: ["primary", "secondary", "danger"]
    },
    onClick: { action: "clicked" }
  }
} satisfies Meta<Args>

export default meta

type Story = StoryObj<Args, typeof meta>

export const Primary: Story = {
  args: {
    label: "Primary Button",
    variant: "primary",
    disabled: false
  }
}

export const Secondary: Story = {
  args: {
    label: "Secondary Button", 
    variant: "secondary",
    disabled: false
  }
}

export const Danger: Story = {
  args: {
    label: "Danger Button",
    variant: "danger", 
    disabled: false
  }
}

export const Disabled: Story = {
  args: {
    label: "Disabled Button",
    variant: "primary",
    disabled: true
  }
}

// Example with custom render function
export const WithIcon: Story = {
  args: {
    label: "Button with Icon",
    variant: "primary",
    disabled: false
  },
  render: (args) => 
    Button(
      { 
        variant: args.variant, 
        disabled: args.disabled,
        onClick: args.onClick 
      },
      html`<span>📱</span> ${args.label}`
    )
}
```

#### 3.2 Complex Story with State

Create `examples/storybook/stories/Counter.stories.ts`:

```typescript
import type { Meta, StoryObj } from "@your-org/storybook"
import { RefSubject } from "@your-org/fx"
import { html } from "@your-org/template"
import * as Fx from "@your-org/fx/Fx"
import * as Effect from "effect/Effect"

type Args = {
  initialValue: number
  step: number
}

// Counter component with state
const Counter = (args: Args) => 
  Effect.gen(function* () {
    const count = yield* RefSubject.of(args.initialValue)
    
    return html`
      <div style="text-align: center; padding: 20px;">
        <h2>Count: ${count}</h2>
        <button 
          onclick=${() => count.update(n => n - args.step)}
        >
          -${args.step}
        </button>
        <button 
          onclick=${() => count.update(n => n + args.step)}
          style="margin-left: 10px;"
        >
          +${args.step}
        </button>
      </div>
    `
  }).pipe(Fx.fromEffect)

const meta = {
  title: "Examples/Counter",
  parameters: {
    layout: "centered"
  },
  tags: ["autodocs"],
  argTypes: {
    initialValue: {
      control: { type: "number" }
    },
    step: {
      control: { type: "number", min: 1 }
    }
  }
} satisfies Meta<Args>

export default meta

type Story = StoryObj<Args, typeof meta>

export const Default: Story = {
  render: Counter,
  args: {
    initialValue: 0,
    step: 1
  }
}

export const StartingAtTen: Story = {
  render: Counter,
  args: {
    initialValue: 10,
    step: 1
  }
}

export const BigSteps: Story = {
  render: Counter,
  args: {
    initialValue: 0,
    step: 5
  }
}
```

## Advanced Configuration

### Custom Error Handling

You can customize how errors are displayed by modifying the `renderToCanvas.ts` file:

```typescript
// In renderToCanvas.ts
const formatError = (cause: Cause.Cause<any>): string => {
  // Custom error formatting logic
  return `Custom Error Format:\n${Cause.pretty(cause)}`
}

// Then use it in the error handler:
showError(new Error(formatError(cause)))
```

### Adding Services

To add additional services to your stories, modify the layer stack in `renderToCanvas.ts`:

```typescript
// Add custom services
const layer = Layer.mergeAll(
  CoreDomServices.layer,
  RenderQueue.layer,
  Layer.succeed(CurrentRoute, makeCurrentRoute(Route.parse("/"))),
  Navigation.memory(),
  Layer.succeed(CurrentEnvironment, "browser" as const),
  // Add your custom services here
  YourCustomService.layer,
  AnotherService.layer
)
```

### Vite Configuration

Customize the Vite configuration in your `preset.ts`:

```typescript
export const viteFinal: NonNullable<StorybookConfig["viteFinal"]> = async (
  config,
  { configType }
) => {
  return {
    ...config,
    define: {
      ...config.define,
      // Add custom defines
      __STORYBOOK__: true
    },
    resolve: {
      ...config.resolve,
      alias: {
        ...config.resolve?.alias,
        // Add custom aliases
        "@components": path.resolve(__dirname, "../src/components")
      }
    },
    plugins: [
      ...config.plugins,
      // Add custom plugins
    ]
  }
}
```

## Troubleshooting

### Common Issues

#### 1. "Cannot find module" errors
- Ensure all workspace packages are built: `pnpm build`
- Check that TypeScript project references are correct
- Verify `pnpm-workspace.yaml` includes your packages

#### 2. Effect errors not displaying properly
- Check that `renderToCanvas.ts` has proper error handling
- Ensure Effect version compatibility across all packages
- Verify the runtime is properly initialized

#### 3. Stories not hot reloading
- Ensure cleanup function is properly implemented
- Check that fibers are being interrupted correctly
- Verify HMR is enabled in your Vite configuration

#### 4. TypeScript errors in stories
- Check that your story types extend the correct interfaces
- Ensure component types match the expected signature
- Verify tsconfig references are correct

### Debug Mode

Add debug logging to your renderer:

```typescript
// In renderToCanvas.ts
const DEBUG = process.env.NODE_ENV === "development"

const renderEffect = Effect.gen(function* () {
  if (DEBUG) {
    console.log("Starting story render:", renderContext.title)
  }
  
  const renderQueue = yield* RenderQueue
  yield* renderQueue.setRenderRoot(canvasElement)
  
  if (DEBUG) {
    console.log("Render queue setup complete")
  }
  
  yield* renderToLayer(storyResult)
  
  if (DEBUG) {
    console.log("Story render complete")
  }
})
```

## Migration Notes

When migrating this setup to a new monorepo:

### 1. Update Package Names
- Replace all instances of `@your-org` with your actual organization name
- Update workspace package references in `package.json` files
- Modify import paths in TypeScript files

### 2. Adapt TypeScript Configuration
- Ensure your base TypeScript configuration matches the requirements
- Update project references to match your package structure
- Verify that path mappings are correct for your setup

### 3. Build System Integration
- Adapt the build scripts to match your build pipeline
- Ensure Babel configuration is compatible (you may need to create the referenced `.babel.cjs.json` and `.babel.mjs.json` files)
- Update any build tools references (like `build-utils pack-v2`)

### 4. Service Dependencies
- Review the services used in `renderToCanvas.ts` and ensure they match your Typed setup
- Add or remove services based on your application's needs
- Update layer composition to match your service architecture

### 5. Testing Integration
- Set up test configuration if using Vitest
- Configure any additional testing utilities needed
- Ensure story testing works with your test runner

## Example Babel Configuration

If you don't have Babel configured, create these files:

`.babel.cjs.json`:
```json
{
  "presets": [
    ["@babel/preset-env", {
      "targets": {
        "node": "18"
      },
      "modules": "cjs"
    }]
  ],
  "plugins": [
    "@effect/babel-plugin",
    "babel-plugin-annotate-pure-calls"
  ]
}
```

`.babel.mjs.json`:
```json
{
  "presets": [
    ["@babel/preset-env", {
      "targets": {
        "node": "18"
      },
      "modules": false
    }]
  ],
  "plugins": [
    "@effect/babel-plugin",
    "babel-plugin-annotate-pure-calls"
  ]
}
```

## Conclusion

This setup provides a comprehensive integration between Storybook and the Typed framework, leveraging Effect's powerful abstractions for error handling and lifecycle management. The custom renderer ensures that Typed components work seamlessly within Storybook's environment while maintaining full type safety and Effect integration.

The modular structure allows for easy customization and extension, while the comprehensive error handling and cleanup ensures a robust development experience. By following this guide, you should have a fully functional Storybook setup that showcases your Typed components effectively.