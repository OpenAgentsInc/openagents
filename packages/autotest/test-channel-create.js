// Test script to create a channel and capture console output
import { BrowserService, BrowserServiceLive } from "./src/Browser/Service.js";
import { ScreenshotService, ScreenshotServiceLive } from "./src/Screenshot/Service.js";
import { Effect, Layer, Runtime } from "effect";

const AppLayer = Layer.mergeAll(BrowserServiceLive, ScreenshotServiceLive);

const program = Effect.gen(function* () {
  const browser = yield* BrowserService;
  const screenshot = yield* ScreenshotService;
  
  // Launch browser
  const browserInstance = yield* browser.launch({ headless: true });
  const page = yield* browser.newPage(browserInstance);
  
  // Enable console logging
  yield* Effect.promise(() => page.evaluate(() => {
    console.log = (function(origLog) {
      return function(...args) {
        window._consoleLogs = window._consoleLogs || [];
        window._consoleLogs.push({ type: 'log', args });
        origLog.apply(console, args);
      };
    })(console.log);
    
    console.error = (function(origError) {
      return function(...args) {
        window._consoleLogs = window._consoleLogs || [];
        window._consoleLogs.push({ type: 'error', args });
        origError.apply(console, args);
      };
    })(console.error);
  }));
  
  // Navigate to create channel page
  yield* Effect.promise(() => page.goto("http://localhost:3003/channels/create", { waitUntil: "networkidle2" }));
  
  // Fill form
  yield* Effect.promise(() => page.type('input[name="name"]', 'Test Channel from Autotest'));
  yield* Effect.promise(() => page.type('textarea[name="about"]', 'This is a test channel created via autotest'));
  
  // Click create button
  yield* Effect.promise(() => page.click('button[type="submit"]'));
  
  // Wait for navigation or error
  yield* Effect.promise(() => page.waitForTimeout(3000));
  
  // Get console logs
  const logs = yield* Effect.promise(() => page.evaluate(() => window._consoleLogs || []));
  console.log("Console logs:", logs);
  
  // Take screenshot
  const screenshotPath = yield* screenshot.capture({
    page,
    fullPage: true,
    path: ".autotest/screenshots/channel-create-result.png"
  });
  
  console.log("Screenshot saved to:", screenshotPath.path);
  
  // Get current URL
  const currentUrl = yield* Effect.promise(() => page.url());
  console.log("Current URL:", currentUrl);
  
  // Close browser
  yield* browser.close(browserInstance);
});

const runtime = Runtime.defaultRuntime;
Effect.runPromise(Effect.provide(program, AppLayer))
  .then(() => console.log("Test completed"))
  .catch(console.error);