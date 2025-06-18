# Programmatic screenshot automation for Claude Code with TypeScript and Bun

Enabling Claude Code to programmatically capture screenshots of local Bun/HTML applications requires careful consideration of browser automation tools, integration patterns, and security implications. Based on comprehensive research, **Puppeteer emerges as the optimal choice** due to confirmed Bun compatibility, while Playwright faces ongoing stability issues with the Bun runtime.

## Browser automation tool selection

### Puppeteer: The recommended solution

**Puppeteer offers the best combination of features for this use case:**
- ✅ **Confirmed Bun compatibility** since v0.6.7
- ✅ **Excellent TypeScript support** with native typing
- ✅ **Lightweight architecture** ideal for screenshot automation
- ✅ **Simple integration** with minimal setup requirements

**Basic implementation with Bun and TypeScript:**

```typescript
// screenshot-service.ts
import { chromium } from 'puppeteer';

interface ScreenshotOptions {
  url: string;
  fullPage?: boolean;
  width?: number;
  height?: number;
  interactions?: () => Promise<void>;
}

export async function captureScreenshot(options: ScreenshotOptions): Promise<Buffer> {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();

  if (options.width && options.height) {
    await page.setViewportSize({ width: options.width, height: options.height });
  }

  await page.goto(options.url);
  await page.waitForLoadState('networkidle');

  // Execute any interactions before screenshot
  if (options.interactions) {
    await options.interactions();
  }

  const screenshot = await page.screenshot({ fullPage: options.fullPage ?? false });
  await browser.close();

  return screenshot;
}
```

### Playwright's Bun compatibility challenges

While Playwright offers superior features, **significant compatibility issues with Bun persist**:
- Hanging processes when using `bunx --bun playwright test`
- Zombie process accumulation
- Module resolution conflicts
- Stability problems in production environments

**If you must use Playwright with Bun**, these workarounds help:
1. Use `playwright-chromium` instead of the full package
2. Avoid `@playwright/test` with Bun runtime
3. Run tests with Node.js while using Bun for development

## Integration patterns for Claude Code

### Pattern 1: MCP Server Architecture (Most Secure)

The **Model Context Protocol provides the cleanest integration** with built-in security features:

```typescript
// mcp-screenshot-server.ts
interface ScreenshotRequest {
  url: string;
  fullPage: boolean;
  interactions?: InteractionStep[];
}

interface InteractionStep {
  action: 'click' | 'fill' | 'select';
  selector: string;
  value?: string;
}

export class ScreenshotMCPServer {
  async handleRequest(request: ScreenshotRequest): Promise<Buffer> {
    // Validate request parameters
    this.validateRequest(request);

    const browser = await puppeteer.launch({
      headless: true,
      args: ['--no-sandbox', '--disable-setuid-sandbox']
    });

    try {
      const page = await browser.newPage();
      await page.goto(request.url);

      // Execute interactions
      for (const step of request.interactions || []) {
        await this.executeInteraction(page, step);
      }

      return await page.screenshot({ fullPage: request.fullPage });
    } finally {
      await browser.close();
    }
  }
}
```

### Pattern 2: Subprocess with Validation

For **direct subprocess execution**, implement strict validation:

```typescript
// secure-subprocess-runner.ts
import { spawn } from 'child_process';
import { z } from 'zod';

const CommandSchema = z.object({
  url: z.string().url().startsWith('http://localhost'),
  outputPath: z.string().regex(/^screenshots\/[\w-]+\.png$/),
  fullPage: z.boolean().optional()
});

export async function executeScreenshotCommand(params: unknown): Promise<void> {
  // Validate input
  const validated = CommandSchema.parse(params);

  return new Promise((resolve, reject) => {
    const child = spawn('bun', [
      'run',
      'screenshot-script.ts',
      JSON.stringify(validated)
    ], {
      timeout: 30000,
      env: { ...process.env, NODE_ENV: 'production' }
    });

    child.on('exit', (code) => {
      if (code === 0) resolve();
      else reject(new Error(`Process exited with code ${code}`));
    });
  });
}
```

### Pattern 3: API Gateway Approach

An **HTTP API provides network-level isolation**:

```typescript
// screenshot-api-server.ts
import { Hono } from 'hono';
import { jwt } from 'hono/jwt';

const app = new Hono();

app.use('/api/*', jwt({ secret: process.env.JWT_SECRET }));

app.post('/api/screenshot', async (c) => {
  const { url, interactions } = await c.req.json();

  // Validate localhost URLs only
  if (!url.startsWith('http://localhost')) {
    return c.json({ error: 'Only localhost URLs allowed' }, 400);
  }

  const screenshot = await captureWithInteractions(url, interactions);
  return c.body(screenshot, 200, { 'Content-Type': 'image/png' });
});

export default app;
```

## Interactive automation implementation

### Form filling and element interaction

```typescript
// interaction-helpers.ts
export async function performInteractions(page: Page, steps: InteractionStep[]) {
  for (const step of steps) {
    switch (step.action) {
      case 'fill':
        await page.fill(step.selector, step.value!);
        break;
      case 'click':
        await page.click(step.selector);
        break;
      case 'select':
        await page.selectOption(step.selector, step.value!);
        break;
    }

    // Wait for any triggered animations/transitions
    await page.waitForTimeout(500);
  }
}

// Advanced interaction workflow
export async function captureFormWorkflow(page: Page) {
  // Fill form fields
  await page.fill('#username', 'testuser');
  await page.fill('#email', 'test@example.com');

  // Handle dynamic content
  await page.click('#load-options');
  await page.waitForSelector('.options-loaded');

  // Select from dynamic dropdown
  await page.selectOption('#country', 'US');

  // Submit and wait for response
  await page.click('#submit');
  await page.waitForSelector('.success-message', { timeout: 10000 });
}
```

### Robust waiting strategies

**Ensure page stability before screenshots**:

```typescript
async function waitForStableState(page: Page): Promise<void> {
  // Wait for network to settle
  await page.waitForLoadState('networkidle');

  // Wait for specific indicators
  await page.waitForSelector('.loading', { state: 'hidden' })
    .catch(() => {}); // Loading indicator might not exist

  // Additional wait for animations
  await page.waitForTimeout(1000);

  // Check for dynamic content stability
  const initialHTML = await page.content();
  await page.waitForTimeout(500);
  const finalHTML = await page.content();

  if (initialHTML !== finalHTML) {
    await page.waitForTimeout(1500); // Content still changing
  }
}
```

## Security considerations

### Critical security measures

1. **URL Validation**: Restrict to localhost/local development servers only
2. **Input Sanitization**: Validate all selectors and values before execution
3. **Resource Limits**: Set timeouts and memory limits for browser processes
4. **Subprocess Isolation**: Use clean environment variables and restricted permissions
5. **Audit Logging**: Track all screenshot requests and interactions

### Secure implementation example

```typescript
// security-wrapper.ts
export class SecureScreenshotService {
  private readonly allowedHosts = ['localhost', '127.0.0.1'];
  private readonly maxExecutionTime = 30000;

  async capture(request: ScreenshotRequest): Promise<Buffer> {
    // Validate URL
    const url = new URL(request.url);
    if (!this.allowedHosts.includes(url.hostname)) {
      throw new Error('Invalid host');
    }

    // Sanitize selectors
    for (const interaction of request.interactions || []) {
      if (!/^[a-zA-Z0-9\-._#\[\]=:]+$/.test(interaction.selector)) {
        throw new Error('Invalid selector');
      }
    }

    // Execute with timeout
    return await Promise.race([
      this.executeCapture(request),
      new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error('Timeout')), this.maxExecutionTime)
      )
    ]);
  }
}
```

## Complete implementation example

### Bun-compatible service with TypeScript

```typescript
// screenshot-automation-service.ts
import puppeteer from 'puppeteer';
import { z } from 'zod';

// Type definitions
const InteractionSchema = z.object({
  action: z.enum(['click', 'fill', 'select', 'wait']),
  selector: z.string(),
  value: z.string().optional(),
  timeout: z.number().optional()
});

const RequestSchema = z.object({
  url: z.string().url(),
  outputPath: z.string(),
  fullPage: z.boolean().default(true),
  viewport: z.object({
    width: z.number(),
    height: z.number()
  }).optional(),
  interactions: z.array(InteractionSchema).optional()
});

type ScreenshotRequest = z.infer<typeof RequestSchema>;

export class ScreenshotAutomationService {
  async processRequest(rawRequest: unknown): Promise<void> {
    // Validate request
    const request = RequestSchema.parse(rawRequest);

    // Launch browser
    const browser = await puppeteer.launch({
      headless: true,
      args: ['--no-sandbox', '--disable-setuid-sandbox']
    });

    try {
      const page = await browser.newPage();

      // Set viewport if specified
      if (request.viewport) {
        await page.setViewportSize(request.viewport);
      }

      // Navigate to URL
      await page.goto(request.url, { waitUntil: 'networkidle0' });

      // Execute interactions
      if (request.interactions) {
        for (const interaction of request.interactions) {
          await this.executeInteraction(page, interaction);
        }
      }

      // Capture screenshot
      await page.screenshot({
        path: request.outputPath,
        fullPage: request.fullPage
      });

    } finally {
      await browser.close();
    }
  }

  private async executeInteraction(page: any, interaction: any): Promise<void> {
    const timeout = interaction.timeout || 5000;

    switch (interaction.action) {
      case 'click':
        await page.click(interaction.selector, { timeout });
        break;
      case 'fill':
        await page.fill(interaction.selector, interaction.value!, { timeout });
        break;
      case 'select':
        await page.selectOption(interaction.selector, interaction.value!, { timeout });
        break;
      case 'wait':
        await page.waitForSelector(interaction.selector, { timeout });
        break;
    }
  }
}

// CLI usage
if (import.meta.main) {
  const service = new ScreenshotAutomationService();
  const request = JSON.parse(process.argv[2]);
  await service.processRequest(request);
}
```

## Conclusion

For Claude Code integration with Bun runtime, **Puppeteer provides the most reliable solution** for programmatic screenshot capture. The MCP server pattern offers the best security model, while subprocess execution with proper validation provides a simpler alternative. Key success factors include robust error handling, comprehensive waiting strategies, and strict security measures to prevent malicious use of browser automation capabilities.
