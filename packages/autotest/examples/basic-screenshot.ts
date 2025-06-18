import { Effect } from "effect"
import { captureScreenshot } from "../src/Claude/index.js"
import { BrowserServiceLive, ScreenshotServiceLive } from "../src/index.js"

// Example 1: Simple screenshot capture
const simpleScreenshot = Effect.gen(function*() {
  const result = yield* captureScreenshot({
    url: "http://localhost:3000",
    fullPage: true
  })

  console.log(`Screenshot saved to: ${result.path}`)
})

// Example 2: Screenshot with custom viewport
const customViewport = Effect.gen(function*() {
  const result = yield* captureScreenshot({
    url: "http://localhost:3000",
    viewport: {
      width: 1920,
      height: 1080
    },
    outputPath: ".autotest/screenshots/desktop-view.png"
  })

  console.log(`Desktop screenshot: ${result.path}`)
})

// Example 3: Mobile viewport screenshot
const mobileScreenshot = Effect.gen(function*() {
  const result = yield* captureScreenshot({
    url: "http://localhost:3000",
    viewport: {
      width: 375,
      height: 667
    },
    outputPath: ".autotest/screenshots/mobile-view.png"
  })

  console.log(`Mobile screenshot: ${result.path}`)
})

// Run examples
const runExamples = Effect.gen(function*() {
  console.log("Capturing screenshots...")

  yield* simpleScreenshot
  yield* customViewport
  yield* mobileScreenshot

  console.log("All screenshots captured successfully!")
})

// Execute with proper service provision
Effect.runPromise(
  runExamples.pipe(
    Effect.provide(BrowserServiceLive),
    Effect.provide(ScreenshotServiceLive)
  )
).catch(console.error)
