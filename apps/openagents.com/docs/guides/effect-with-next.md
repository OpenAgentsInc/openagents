# Integrating Effect with Next.js: A Pattern Guide

## 1. Introduction

This document outlines a set of robust, scalable patterns for integrating the Effect library into a modern Next.js application. The core philosophy is a strict **separation of concerns**, where the framework-specific code (Next.js routes and React components) acts as a thin bridge to a powerful, self-contained business logic layer built with Effect.

This approach results in applications that are:
*   **Highly Type-Safe:** Leveraging Effect's powerful type system to catch errors at compile time.
*   **Extremely Testable:** Business logic can be tested in isolation, with full control over dependencies and environment.
*   **Maintainable:** Code is organized by domain logic in the `lib` directory, not by framework conventions.
*   **Resilient:** Error handling is explicit, structured, and exhaustive.

## 2. Core Principles

Before diving into server and client patterns, it's crucial to understand the foundational principles that enable this architecture.

### 2.1. Separation of Concerns: The `lib` Directory

The cornerstone of this architecture is isolating business logic from the web framework.

*   **`app/` Directory:** This contains only the Next.js framework code. API Routes and React Components in this directory should be "thin." Their primary responsibility is to interface with the outside world (HTTP requests, user interactions) and delegate all meaningful work to the Effect programs defined in `lib/`.
*   **`lib/` Directory:** This is the heart of the application. It contains all Effect-native code: services, schemas, configurations, and the core business logic workflows. This code has no knowledge of Next.js and could be run in any JavaScript environment.

### 2.2. Dependency Injection: Context and Layers

Effect uses a powerful dependency injection (DI) system to manage services.

*   **The "What" (`Context.Tag`):** A service is first defined as an interface, representing a contract or capability. A `Context.Tag` is created for this interface, acting as a unique, typed identifier for the service within Effect's DI system.
*   **The "How" (`Layer`):** A `Layer` provides the concrete, "live" implementation of a service. It describes how to construct the service, including its own dependencies. This separates the definition of a service from its implementation, which is critical for testability.

### 2.3. Schema-Driven Development

Use `@effect/schema` at all I/O boundaries of the application. This ensures that any data entering or leaving the system is valid and conforms to an expected structure.

*   **API Boundaries:** Define schemas for API route request bodies and responses.
*   **External Services:** Define schemas for the requests and responses when interacting with third-party APIs (e.g., ConvertKit).
*   **Type Safety:** Schemas act as the single source of truth for data shapes, providing static types and runtime validation from a single definition.

### 2.4. Structured Error Handling

Avoid generic `try/catch` blocks. Instead, model all potential failures as part of the Effect type system.

*   **Custom Tagged Errors:** For business-logic-specific failures (e.g., invalid input, resource not found), create custom error classes that extend `Data.TaggedError`. This gives each error a unique, identifiable tag.
*   **Exhaustive Handling:** Use `Effect.catchTags` to handle specific, known errors by their tag. This forces you to consider each failure case explicitly, leading to more robust programs. A final `Effect.catchAll` can be used to handle any unexpected or unrecoverable errors.

### 2.5. Configuration as a Dependency

Treat application configuration (e.g., environment variables, API keys) as a dependency, not as a global singleton.

*   **`Effect.Config`:** Define configurations using `Effect.Config` primitives (e.g., `Config.string`, `Config.number`). These definitions are just descriptions.
*   **Injection:** The Effect runtime injects the actual configuration values when the program is run. This allows tests to easily provide a mock `ConfigProvider` layer, giving full control over configuration values without manipulating environment variables.

## 3. Server-Side Patterns (API Routes)

This pattern describes how to handle incoming HTTP requests in a Next.js API Route.

### 3.1. The Thin Route Handler (`app/api/.../route.ts`)

The API route file is the entry point. It should be minimal.

1.  **Import:** Import the `main` Effect program from its corresponding file in the `lib/` directory.
2.  **Delegate:** In the Next.js handler function (e.g., `POST`), pass the incoming `Request` object directly to the `main` program.
3.  **Execute:** Execute the returned Effect program using `Effect.runPromise`. This will produce a `Promise<Response>` that Next.js can serve.

### 3.2. The Main Server Program (`lib/Server.ts`)

This file contains the complete, end-to-end logic for handling an API request. The `main` function orchestrates the entire workflow.

1.  **Define the Entry Point:** Create a function (e.g., `main`) that accepts the native `Request` object and is typed to return a fully resolved Effect: `Effect.Effect<never, never, Response>`. This signature signifies that all dependencies have been provided and all errors have been handled.
2.  **Wrap Native I/O:** Use `Effect.tryPromise` to bring asynchronous operations on the `Request` object (like `request.json()`) into the Effect context. Map any potential promise rejections to a custom tagged error.
3.  **Validate Input:** Use `Schema.parseEither` to validate the incoming request body against a defined schema. Map the `ParseError` to a custom tagged error for clear, domain-specific failure handling.
4.  **Orchestrate Logic:** Use an `Effect.gen` generator to define the "happy path" workflow. Inside the generator, `yield*` to:
    *   Resolve service dependencies (e.g., `yield* _(MyService)`).
    *   Call service methods to perform business logic.
    *   Construct the successful `Response` object.
5.  **Compose and Resolve:** The `Effect.gen` program will have a type signature that includes its dependencies (e.g., `MyService`) and all possible errors. Pipe this program through a series of combinators to resolve these:
    *   **`Effect.provide(...)`**: Provide the live `Layer` implementation for every service the program requires. This removes the dependencies from the type signature.
    *   **`Effect.catchTags({...})`**: Handle each known, tagged error. The handler for each tag should create and succeed with a corresponding error `Response` (e.g., with a 400 or 404 status code). This removes the handled errors from the type signature.
    *   **`Effect.catchAll(...)`**: Add a final catch-all to handle any remaining or unexpected errors. This should return a generic 500 server error `Response`. This final step ensures the error channel of the Effect is `never`.
6.  **Return the Program:** The function returns the fully composed Effect program, ready to be executed by the route handler.

## 4. Client-Side Patterns (React Components)

This pattern describes how to interact with Effect programs from within a React Client Component.

### 4.1. The Thin React Component (`app/page.tsx`)

The component's responsibility is to manage UI state and trigger Effect programs based on user interaction. It must be a `"use client"` component.

1.  **Define State:** Use React state (`useState`) to manage form inputs, loading status, and success/error messages.
2.  **Create Event Handler:** Create an `async` event handler (e.g., `onSubmit`).
3.  **Execute Safely:** Inside the handler, call the client-side Effect program using **`Effect.runPromiseExit`**. This is critical. `runPromiseExit` returns a `Promise` that resolves with an `Exit` object (`Success` or `Failure`) and *never* rejects.
4.  **Handle the Result:** Use `Exit.match` on the returned `Exit` object to handle the two cases:
    *   `onSuccess`: Update component state to reflect a successful operation.
    *   `onFailure`: Update component state to display an appropriate error message to the user. This prevents unhandled exceptions from crashing the UI.

### 4.2. The Client-Side Effect Program (`lib/Client.ts`)

This file defines the Effect workflow that runs in the browser.

1.  **Define the Entry Point:** Create a function that accepts any necessary data from the component (e.g., form input) and returns an Effect program.
2.  **Make API Calls:** Use `@effect/platform/HttpClient` to make `fetch` requests to your application's own API routes.
3.  **Use Schemas:** Use `Http.request.schemaBody` to safely encode the request body and `Http.response.schemaBodyJson` to validate and parse the JSON response from the server.
4.  **Use Configuration:** Access any client-side environment variables (e.g., `NEXT_PUBLIC_API_URL`) via `Effect.Config`.

## 5. Testing Strategy

The architectural patterns above make testing natural and comprehensive.

*   **Isolate Logic:** Test the Effect programs in `lib/` directly, without needing to render Next.js components or run a server.
*   **Mock Dependencies:** In your tests, use `Effect.provide` to inject mock service layers instead of live ones. This allows you to test a service's logic without executing its actual dependencies.
*   **Mock Configuration:** Create a test-specific `ConfigProvider` and provide it to your Effect program using `Layer.setConfigProvider`. This lets you specify any configuration values needed for a test scenario.
*   **Mock Network:** Use a library like Mock Service Worker (`msw`) to intercept and mock any outgoing HTTP requests made by `HttpClient`. This allows you to simulate various API responses (success, different error types, network failures) and test how your Effect program handles them.
