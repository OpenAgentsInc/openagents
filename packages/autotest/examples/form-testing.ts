import { Effect } from "effect"
import { captureScreenshot } from "../src/Claude/index.js"
import { BrowserServiceLive, ScreenshotServiceLive } from "../src/index.js"

// Example: Test a login form with interactions
const testLoginForm = Effect.gen(function*() {
  console.log("Testing login form...")

  // Step 1: Capture initial state
  const initialState = yield* captureScreenshot({
    url: "http://localhost:3000/login",
    outputPath: ".autotest/screenshots/login-initial.png"
  })

  // Step 2: Fill form and submit
  const afterLogin = yield* captureScreenshot({
    url: "http://localhost:3000/login",
    interactions: [
      {
        action: "fill",
        selector: "#username",
        value: "testuser@example.com"
      },
      {
        action: "fill",
        selector: "#password",
        value: "testpassword123"
      },
      {
        action: "click",
        selector: "#remember-me"
      },
      {
        action: "click",
        selector: "#submit-button"
      },
      {
        action: "wait",
        selector: ".dashboard",
        timeout: 10000
      }
    ],
    fullPage: true,
    outputPath: ".autotest/screenshots/login-success.png"
  })

  console.log("Login form test completed!")
  console.log(`Initial state: ${initialState.path}`)
  console.log(`After login: ${afterLogin.path}`)
})

// Example: Test a multi-step form
const testMultiStepForm = Effect.gen(function*() {
  console.log("Testing multi-step form...")

  const steps = [
    // Step 1: Personal Information
    {
      name: "personal-info",
      interactions: [
        { action: "fill" as const, selector: "#firstName", value: "John" },
        { action: "fill" as const, selector: "#lastName", value: "Doe" },
        { action: "fill" as const, selector: "#email", value: "john.doe@example.com" },
        { action: "click" as const, selector: "#next-step-1" }
      ]
    },
    // Step 2: Address Information
    {
      name: "address-info",
      interactions: [
        { action: "wait" as const, selector: "#address-form", timeout: 5000 },
        { action: "fill" as const, selector: "#street", value: "123 Main St" },
        { action: "fill" as const, selector: "#city", value: "San Francisco" },
        { action: "select" as const, selector: "#state", value: "CA" },
        { action: "fill" as const, selector: "#zip", value: "94105" },
        { action: "click" as const, selector: "#next-step-2" }
      ]
    },
    // Step 3: Review and Submit
    {
      name: "review-submit",
      interactions: [
        { action: "wait" as const, selector: "#review-form", timeout: 5000 },
        { action: "click" as const, selector: "#terms-checkbox" },
        { action: "click" as const, selector: "#submit-form" },
        { action: "wait" as const, selector: ".success-message", timeout: 10000 }
      ]
    }
  ]

  // Capture screenshot after each step
  for (const step of steps) {
    const result = yield* captureScreenshot({
      url: "http://localhost:3000/multi-step-form",
      interactions: step.interactions,
      fullPage: true,
      outputPath: `.autotest/screenshots/form-step-${step.name}.png`
    })

    console.log(`Step '${step.name}' captured: ${result.path}`)
  }

  console.log("Multi-step form test completed!")
})

// Example: Test form validation
const testFormValidation = Effect.gen(function*() {
  console.log("Testing form validation...")

  // Test invalid email
  const invalidEmail = yield* captureScreenshot({
    url: "http://localhost:3000/signup",
    interactions: [
      {
        action: "fill",
        selector: "#email",
        value: "invalid-email"
      },
      {
        action: "click",
        selector: "#submit"
      },
      {
        action: "wait",
        selector: ".error-message",
        timeout: 3000
      }
    ],
    outputPath: ".autotest/screenshots/validation-email-error.png"
  })

  // Test password requirements
  const weakPassword = yield* captureScreenshot({
    url: "http://localhost:3000/signup",
    interactions: [
      {
        action: "fill",
        selector: "#email",
        value: "valid@example.com"
      },
      {
        action: "fill",
        selector: "#password",
        value: "weak"
      },
      {
        action: "click",
        selector: "#submit"
      },
      {
        action: "wait",
        selector: ".password-requirements",
        timeout: 3000
      }
    ],
    outputPath: ".autotest/screenshots/validation-password-error.png"
  })

  console.log("Form validation test completed!")
  console.log(`Email validation: ${invalidEmail.path}`)
  console.log(`Password validation: ${weakPassword.path}`)
})

// Run all examples
const runAllTests = Effect.gen(function*() {
  yield* testLoginForm
  yield* Effect.sleep("2 seconds")

  yield* testMultiStepForm
  yield* Effect.sleep("2 seconds")

  yield* testFormValidation
})

// Execute with proper service provision
Effect.runPromise(
  runAllTests.pipe(
    Effect.provide(BrowserServiceLive),
    Effect.provide(ScreenshotServiceLive)
  )
).catch(console.error)
