# Autotest - Browser Automation & Visual Testing

## Overview

The `@openagentsinc/autotest` package provides comprehensive browser automation and visual testing capabilities for Claude Code. It enables programmatic control over development servers, browser navigation, screenshot capture, and automated testing of web applications.

## Key Features

- **Server Lifecycle Management**: Start, monitor, and stop development servers programmatically
- **Browser Automation**: Headless browser control via Puppeteer
- **Screenshot Capture**: Visual testing and regression detection
- **Test Orchestration**: Coordinate server startup, testing, and cleanup
- **Comprehensive Monitoring**: Track console messages, network requests, and errors
- **Effect-based Architecture**: Built with Effect for type-safe, composable operations

## Quick Start

### Basic Screenshot Capture

```bash
# Navigate to autotest package
cd packages/autotest

# Capture a screenshot of any URL
bun run src/cli.ts '{"url":"http://localhost:3000","fullPage":true}'
```

### Test Orchestration

```bash
# Run full test orchestration with a configuration file
bun src/orchestrate.ts "$(cat test-orchestration.json)"

# Test OpenAgents.com with default configuration
bun src/orchestrate.ts --default
```

## Architecture

### Core Services

#### ServerService
Manages development server lifecycle using Effect's daemon fibers to keep processes alive:

```typescript
interface ServerService {
  start: (options: ServerOptions) => Effect<ServerProcess, ServerError>
  stop: (process: ServerProcess) => Effect<void, ServerError>
  getState: (process: ServerProcess) => Effect<ServerState>
  waitForReady: (process: ServerProcess, options?: WaitOptions) => Effect<void, ServerTimeoutError>
}
```

Key features:
- Automatic port finding
- Environment variable injection (PORT)
- Real-time log streaming
- Ready state detection via regex patterns
- Graceful shutdown with SIGTERM

#### BrowserService
Controls headless browser instances:

```typescript
interface BrowserService {
  launch: (options?: LaunchOptions) => Effect<Browser, BrowserError>
  close: (browser: Browser) => Effect<void, BrowserError>
  newPage: (browser: Browser) => Effect<Page, PageError>
  closePage: (page: Page) => Effect<void, PageError>
}
```

#### ScreenshotService
Captures and manages screenshots:

```typescript
interface ScreenshotService {
  capture: (options: ScreenshotOptions) => Effect<Screenshot, ScreenshotError>
  save: (screenshot: Screenshot, filePath: string) => Effect<void, FileError>
  load: (filePath: string) => Effect<Screenshot, FileError>
  compare: (baseline: Screenshot, current: Screenshot, threshold?: number) => Effect<DiffResult, ComparisonError>
}
```

#### TestOrchestrator
Coordinates the complete testing workflow:

```typescript
interface TestOrchestrator {
  testRoute: (page: Page, route: string, interactions?: InteractionStep[]) => Effect<RouteTestResult, Error>
  runFullTest: (config: OrchestratorConfig) => Effect<TestReport, Error>
}
```

## Configuration

### Orchestrator Configuration

```typescript
interface OrchestratorConfig {
  project: {
    root: string              // Project root directory
    startCommand: string      // Command to start dev server
    port: number             // Port to use (will find available)
    env?: Record<string, string>  // Additional environment variables
    readyPattern?: RegExp    // Pattern to detect server ready
  }
  testing: {
    routes: string[]         // Routes to test
    baseUrl?: string         // Override base URL
    timeout?: number         // Test timeout in ms
    interactions?: Array<{   // Interactions per route
      route: string
      actions: Array<string | InteractionStep>
    }>
  }
  monitoring?: {
    captureConsole?: boolean    // Monitor console messages
    captureNetwork?: boolean    // Track network requests
    captureErrors?: boolean     // Capture page errors
    screenshotOnError?: boolean // Take screenshots on error
  }
}
```

### Example Configuration

```json
{
  "project": {
    "root": "/path/to/project",
    "startCommand": "bun run dev",
    "port": 3000,
    "readyPattern": "Server is running at"
  },
  "testing": {
    "routes": ["/", "/about", "/products"],
    "timeout": 30000
  },
  "monitoring": {
    "captureConsole": true,
    "captureNetwork": true,
    "captureErrors": true,
    "screenshotOnError": true
  }
}
```

## Test Reports

The orchestrator generates comprehensive test reports:

```typescript
interface TestReport {
  startedAt: Date
  completedAt: Date
  duration: number
  serverLogs: string[]
  routes: RouteTestResult[]
  summary: TestSummary
  suggestedFixes?: SuggestedFix[]
}

interface RouteTestResult {
  route: string
  success: boolean
  duration: number
  errors: TestError[]
  screenshots: string[]
  console: ConsoleMessage[]
  network: NetworkRequest[]
}
```

## Usage Examples

### 1. Testing a Local Development Server

```bash
# Create test configuration
cat > test-config.json << EOF
{
  "project": {
    "root": "$(pwd)",
    "startCommand": "npm run dev",
    "port": 3000,
    "readyPattern": "ready"
  },
  "testing": {
    "routes": ["/", "/api/health", "/dashboard"],
    "timeout": 20000
  }
}
EOF

# Run tests
bun packages/autotest/src/orchestrate.ts "$(cat test-config.json)"
```

### 2. Visual Regression Testing

```typescript
// Capture baseline screenshots
const baseline = yield* screenshotService.capture({
  page,
  fullPage: true
})
yield* screenshotService.save(baseline, "baseline.png")

// Later, capture current state and compare
const current = yield* screenshotService.capture({
  page,
  fullPage: true
})
const diff = yield* screenshotService.compare(baseline, current, 0.1)

if (!diff.match) {
  console.log(`Visual regression detected: ${diff.difference}% difference`)
}
```

### 3. Programmatic Server Testing

```typescript
import { Effect } from "effect"
import { ServerService } from "@openagentsinc/autotest"

const program = Effect.gen(function* () {
  const serverService = yield* ServerService
  
  // Start server
  const server = yield* serverService.start({
    command: "bun run dev",
    cwd: "/path/to/project",
    port: 3000,
    env: { NODE_ENV: "test" }
  })
  
  // Wait for ready
  yield* serverService.waitForReady(server, {
    timeout: 30000,
    pattern: /Server is running/
  })
  
  // Run tests...
  
  // Clean up
  yield* serverService.stop(server)
})
```

## Best Practices

### 1. Server Readiness Patterns
Always define appropriate ready patterns for your server:

```typescript
// Good patterns
readyPattern: /listening on|Server is running at|ready/i

// Be specific when possible
readyPattern: /\[vite\] Local:.*http:\/\/localhost:\d+/
```

### 2. Error Handling
Use Effect's error handling for robust test flows:

```typescript
const result = yield* testRoute(page, "/fragile-route").pipe(
  Effect.catchTag("NavigationError", () => 
    Effect.succeed({
      success: false,
      errors: [{ type: "navigation", message: "Route unavailable" }]
    })
  )
)
```

### 3. Resource Cleanup
Always ensure proper cleanup, even on failure:

```typescript
yield* Effect.acquireUseRelease(
  serverService.start(options),
  (server) => runTests(server),
  (server) => serverService.stop(server)
)
```

### 4. Screenshot Organization
Use descriptive filenames with timestamps:

```typescript
const filename = `screenshot-${route.replace(/\//g, '-')}-${Date.now()}.png`
const filepath = `.autotest/screenshots/${testRun}/${filename}`
```

## Troubleshooting

### Server Won't Start
- Check if the port is already in use
- Verify the start command is correct
- Ensure the working directory is set properly
- Check server logs in the test report

### Ready Detection Timeout
- Verify your ready pattern matches actual server output
- Increase the timeout for slower servers
- Check if the server is outputting to stderr instead of stdout

### Screenshot Failures
- Ensure the `.autotest/screenshots/` directory exists
- Check disk space
- Verify Puppeteer dependencies are installed

### Port Conflicts
The service automatically finds available ports, but you can specify a range:

```typescript
const port = yield* findAvailablePort(3000) // Searches 3000-3099
```

## CLI Commands

### Screenshot Capture
```bash
bun run src/cli.ts '{"url":"http://localhost:3000","fullPage":true,"outputPath":"./screenshots/"}'
```

Options:
- `url` (required): URL to capture
- `fullPage`: Capture full scrollable area
- `viewport`: Custom viewport dimensions
- `outputPath`: Where to save screenshots

### Test Orchestration
```bash
bun src/orchestrate.ts <config-json>
bun src/orchestrate.ts --default  # Test OpenAgents.com
```

## Integration with Claude Code

When Claude Code needs to test a web application:

1. **Start with orchestration**: Use the orchestrator for full testing
2. **Capture screenshots**: Visual verification of UI states
3. **Monitor errors**: Check console and network errors
4. **Analyze reports**: Review the generated test report

Example workflow:
```bash
# 1. Start dev server and test
bun packages/autotest/src/orchestrate.ts --default

# 2. View test results
cat packages/autotest/test-report.json

# 3. View screenshots
Read: packages/autotest/.autotest/screenshots/screenshot-*.png
```

## Technical Implementation Details

### Daemon Fibers for Process Management
The ServerService uses Effect's `forkDaemon` to create long-lived processes that survive beyond their creating scope:

```typescript
const fiber = yield* Effect.forkDaemon(
  Effect.scoped(
    Effect.gen(function* () {
      const proc = yield* executor.start(command)
      // Process stays alive until fiber is interrupted
      yield* Effect.never
    })
  )
)
```

### Stream-based Log Collection
Logs are collected in real-time using Effect streams:

```typescript
yield* proc.stdout.pipe(
  Stream.decodeText(),
  Stream.splitLines,
  Stream.tap((line) => updateState(line)),
  Stream.runDrain
).pipe(Effect.forkDaemon)
```

### Port Management
Automatic port finding with fallback:

```typescript
const findAvailablePort = (startPort: number) =>
  Effect.gen(function* () {
    for (let port = startPort; port < startPort + 100; port++) {
      const available = yield* isPortAvailable(port)
      if (available) return port
    }
    return yield* Effect.fail(new ServerPortError(startPort))
  })
```

## Future Enhancements

See GitHub issue #967 for planned improvements and feature requests.