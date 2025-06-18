# @openagentsinc/autotest

Browser automation and visual testing framework for OpenAgents, designed for seamless integration with Claude Code and the OpenAgents monorepo.

## Installation

```bash
pnpm add @openagentsinc/autotest
```

## Features

- 🚀 **Bun-compatible** - Built with Puppeteer for reliable Bun runtime support
- 🎯 **Effect-based** - Consistent with OpenAgents architecture patterns
- 🔒 **Secure by default** - Localhost-only testing with path restrictions
- 🤖 **Claude Code ready** - Built-in integration for AI-powered testing
- 📸 **Visual testing** - Screenshot capture and comparison utilities
- 🎭 **Interactive testing** - Form filling, clicking, and navigation helpers

## Quick Start

### Basic Screenshot Capture

```typescript
import { Effect } from "effect"
import { BrowserServiceLive, ScreenshotServiceLive } from "@openagentsinc/autotest"
import { captureScreenshot } from "@openagentsinc/autotest/Claude"

const program = Effect.gen(function* () {
  const result = yield* captureScreenshot({
    url: "http://localhost:3000",
    fullPage: true,
    outputPath: ".autotest/screenshots/homepage.png"
  })
  
  console.log(`Screenshot saved to: ${result.path}`)
})

// Run with services
Effect.runPromise(
  program.pipe(
    Effect.provide(BrowserServiceLive),
    Effect.provide(ScreenshotServiceLive)
  )
)
```

### Interactive Testing

```typescript
import { Effect } from "effect"
import { BrowserService, navigate, fill, click, waitForSelector } from "@openagentsinc/autotest"

const testLoginFlow = Effect.gen(function* () {
  const browser = yield* BrowserService
  const browserInstance = yield* browser.launch()
  const page = yield* browser.newPage(browserInstance)
  
  // Navigate to login page
  yield* navigate(page, "http://localhost:3000/login")
  
  // Fill login form
  yield* fill(page, "#username", "testuser")
  yield* fill(page, "#password", "testpass")
  
  // Submit form
  yield* click(page, "#submit")
  
  // Wait for dashboard
  yield* waitForSelector(page, ".dashboard")
  
  // Cleanup
  yield* browser.close(browserInstance)
})
```

## Claude Code Integration

The autotest package is designed to work seamlessly with Claude Code for AI-powered testing:

### CLI Usage

```bash
# Capture a screenshot
bun run capture '{"url":"http://localhost:3000","fullPage":true}'

# With interactions
bun run capture '{
  "url": "http://localhost:3000",
  "interactions": [
    {"action": "fill", "selector": "#search", "value": "test"},
    {"action": "click", "selector": "#submit"},
    {"action": "wait", "selector": ".results"}
  ],
  "outputPath": ".autotest/screenshots/search-results.png"
}'
```

### Security Features

- **URL Validation**: Only localhost URLs allowed by default
- **Path Restrictions**: Screenshots saved only to `.autotest/` directory
- **Timeout Protection**: Maximum execution time limits
- **Input Sanitization**: All selectors and values validated

## Psionic Integration

Test Psionic hypermedia applications with specialized utilities:

```typescript
import { testPsionicApp, withPsionicTest } from "@openagentsinc/psionic/Testing"
import { BrowserServiceLive, ScreenshotServiceLive } from "@openagentsinc/autotest"

// Test a Psionic app
const testApp = withPsionicTest(app, (test) =>
  Effect.gen(function* () {
    // Visit homepage
    yield* test.visit("/")
    
    // Verify hypermedia components
    yield* test.expectHypermedia("h1")
    yield* test.expectComponent("Navigation")
    
    // Trigger HTMX interaction
    yield* test.triggerHTMX("#load-more")
    
    // Capture screenshot
    const screenshotPath = yield* test.captureScreenshot("homepage")
  })
)

// Run test
Effect.runPromise(
  testApp.pipe(
    Effect.provide(BrowserServiceLive),
    Effect.provide(ScreenshotServiceLive)
  )
)
```

## Visual Regression Testing

Compare screenshots to detect visual changes:

```typescript
import { ScreenshotService } from "@openagentsinc/autotest"

const compareScreenshots = Effect.gen(function* () {
  const service = yield* ScreenshotService
  
  // Load baseline and current screenshots
  const baseline = yield* service.load(".autotest/baseline/homepage.png")
  const current = yield* service.load(".autotest/screenshots/homepage.png")
  
  // Compare screenshots
  const result = yield* service.compare(baseline, current, 0.1) // 10% threshold
  
  if (!result.match) {
    console.log(`Visual difference detected: ${result.difference * 100}%`)
  }
})
```

## API Reference

### Browser Service

```typescript
interface BrowserService {
  launch(options?: BrowserOptions): Effect<Browser, BrowserError>
  newPage(browser: Browser): Effect<Page, PageError>
  close(browser: Browser): Effect<void, BrowserError>
  closePage(page: Page): Effect<void, PageError>
}
```

### Screenshot Service

```typescript
interface ScreenshotService {
  capture(options: ScreenshotOptions): Effect<Screenshot, ScreenshotError>
  save(screenshot: Screenshot, path: string): Effect<void, FileError>
  load(path: string): Effect<Screenshot, FileError>
  compare(baseline: Screenshot, current: Screenshot, threshold?: number): Effect<DiffResult, ComparisonError>
}
```

### Testing Utilities

- `navigate(page, url, options?)` - Navigate to URL
- `click(page, selector, timeout?)` - Click element
- `fill(page, selector, value, timeout?)` - Fill input field
- `select(page, selector, value, timeout?)` - Select dropdown option
- `waitForSelector(page, selector, options?)` - Wait for element
- `performInteractions(page, steps)` - Execute multiple interactions

## Examples

See the `examples/` directory for more usage examples:

- `basic-screenshot.ts` - Simple screenshot capture
- `form-testing.ts` - Form interaction testing
- `visual-regression.ts` - Visual regression testing
- `psionic-app.ts` - Testing Psionic applications

## Development

```bash
# Install dependencies
pnpm install

# Build package
pnpm build

# Run tests
pnpm test

# Type check
pnpm check
```

## License

MIT