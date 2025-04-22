# Guidance: Understanding Effect.js Service Patterns (Tags, Layers, Testing)

## 1. Problem Statement

During the implementation of services like `PlanManager`, `TaskExecutor`, and `ContextManager`, and their corresponding tests, recurring TypeScript errors (`TS2344`, `TS2345`, `TS18046`, `TS2739`) and test failures were encountered. These issues primarily stemmed from misunderstandings of how to correctly define, provide, access, and mock services using Effect.js's core dependency injection concepts: `Effect.Tag` and `Layer`.

Attempts to fix these often led to incorrect workarounds, fragile mocks, or even the deletion of essential test logic. This document clarifies the correct patterns to use.

## 2. Core Concepts

Understanding the distinct roles of these components is crucial:

1.  **Service Interface (TypeScript `interface`):** Defines the *contract* of the service – the methods it offers and their signatures.
    ```typescript
    // Example: src/github/MyService.ts
    export interface MyService {
        readonly doSomething: (input: string) => Effect.Effect<number, Error>;
        readonly doSomethingElse: () => Effect.Effect<void>;
    }
    ```

2.  **Service Tag (`Effect.Tag`):** Acts as a unique **key** or **identifier** for the service within Effect's `Context` (its dependency injection container). It links the interface contract to its runtime implementation.
    *   **Recommended Pattern (Class-based):** This pattern worked most reliably in this project. It combines the Tag definition with the service interface.
        ```typescript
        // Example: src/github/MyService.ts
        import { Effect } from "effect";
        import type { MyService as MyServiceInterface } from "./MyService"; // Import the interface if defined separately

        // eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
        export class MyService extends Effect.Tag("MyService")<MyService, MyServiceInterface>() {}
        //                                       ^ Identifier  ^ Tag Class ^ Service Interface Type
        ```
        *Note: The `eslint-disable-next-line` is often needed because declaring a class and interface with the same name can trigger lint warnings, but it's a valid and common pattern in Effect.*
    *   **Alternative (`const Tag = Effect.Tag<Interface>()`):** While simpler, this sometimes caused inference issues in complex scenarios or tests in this project. The class-based pattern proved more robust *here*.

3.  **Service Implementation (Plain Object):** An object containing the actual logic for the methods defined in the service interface. Methods should return `Effect` values.
    ```typescript
    // Example: src/github/MyService.ts
    const myServiceImpl: MyService = {
        doSomething: (input) => Effect.succeed(input.length),
        doSomethingElse: () => Effect.log("Doing something else")
    };
    ```

4.  **Layer (`Layer.Layer`):** Responsible for *providing* a service implementation into the Effect `Context`, associated with its specific `Tag`.
    *   **`Layer.succeed`:** Used when the implementation object can be created synchronously. **Crucially, use `Tag.of(implementation)` when using the class-based Tag pattern.**
        ```typescript
        // Example: src/github/MyService.ts
        import { Layer } from "effect";

        export const MyServiceLive = Layer.succeed(
            MyService, // The Tag INSTANCE (the class itself in this pattern)
            MyService.of(myServiceImpl) // Use Tag.of() to wrap the implementation
        );
        ```
    *   **`Layer.effect`:** Used when creating the service implementation requires performing Effects (e.g., depending on other services).
        ```typescript
        // Example: src/github/AnotherService.ts
        import { Effect, Layer } from "effect";
        import { MyService } from "./MyService"; // Import the dependency Tag

        // ... interface AnotherService ...
        // ... class AnotherService extends Effect.Tag(...) ...

        export const AnotherServiceLive = Layer.effect(
            AnotherService, // The Tag instance for this service
            Effect.gen(function*() {
                const myService = yield* MyService; // Access dependency via Tag instance
                const implementation = {
                    doStuffWithDependency: () => myService.doSomething("test")
                };
                return AnotherService.of(implementation); // Return implementation wrapped in Tag.of()
            })
        );
        ```

5.  **Accessing Services (`yield* TagInstance`):** Within an `Effect.gen` function, use `yield*` with the **Tag instance** to retrieve the service implementation from the context.
    ```typescript
    // Example: src/github/SomeProgram.ts
    import { Effect } from "effect";
    import { MyService } from "./MyService";

    const program = Effect.gen(function*() {
        const service = yield* MyService; // Yield the Tag instance
        const result = yield* service.doSomething("hello");
        yield* service.doSomethingElse();
        console.log(result);
    });
    ```

## 3. Correct Testing Pattern

Testing services requires providing either the live layer or, more commonly, a *mock layer*.

1.  **Define Mocks:** Use `vi.fn()` for methods you need to spy on or provide custom mock behavior for.
2.  **Create Mock Implementation:** Create an object that matches the **Service Interface**. All methods defined in the interface *must* be present in the mock object. Methods should return `Effect`s (e.g., `Effect.succeed(...)`, `Effect.fail(...)`).
3.  **Create Mock Layer:** Use `Layer.succeed(TagInstance, TagInstance.of({ /* mock implementation object */ }))`. **Crucially, provide mocks for ALL methods defined in the interface**, even if not directly used in a specific test, to satisfy TypeScript.
    ```typescript
    // Example: test/github/MyService.test.ts
    import { describe, it, expect, vi } from "@effect/vitest";
    import { Effect, Layer } from "effect";
    import { MyService, MyServiceLive } from "../../src/github/MyService"; // Import Tag and Layer

    describe("MyService", () => {
        it("should do something", async () => {
            // 1. Define Mocks (if needed for assertions)
            const doSomethingMock = vi.fn((_input: string) => Effect.succeed(99));
            const doSomethingElseMock = vi.fn(() => Effect.void);

            // 2. Create Mock Implementation Object (implementing MyService interface)
            const mockServiceImpl: MyService = {
                 doSomething: doSomethingMock,
                 doSomethingElse: doSomethingElseMock,
            };

            // 3. Create Mock Layer using Tag.of()
            const MockMyServiceLayer = Layer.succeed(
                MyService,
                MyService.of(mockServiceImpl)
            );

            // 4. Define the Effect to Test (access service via Tag)
            const effectToTest = Effect.gen(function*() {
                const service = yield* MyService;
                const result = yield* service.doSomething("test");
                yield* service.doSomethingElse();
                return result;
            });

            // 5. Provide the Mock Layer and Run
            const result = await Effect.runPromise(
                Effect.provide(effectToTest, MockMyServiceLayer)
            );

            // 6. Assert
            expect(result).toBe(99);
            expect(doSomethingMock).toHaveBeenCalledWith("test");
            expect(doSomethingElseMock).toHaveBeenCalled();
        });

        // Test interaction with other services by providing multiple mock layers
        it("AnotherService should use MyService", async () => {
             const doSomethingMock = vi.fn((_input: string) => Effect.succeed(123));
             const MockMyServiceLayer = Layer.succeed(MyService, MyService.of({ /* ... */ doSomething: doSomethingMock, /*...*/}));

             // Assume AnotherServiceLive depends on MyService
             // The effect to test uses AnotherService
             const effectUsingAnotherService = Effect.flatMap(AnotherService, svc => svc.doStuffWithDependency());

             // Provide BOTH the real AnotherService layer AND the mock MyService layer
             const testLayer = Layer.provide(AnotherServiceLive, MockMyServiceLayer);

             const result = await Effect.runPromise(
                 Effect.provide(effectUsingAnotherService, testLayer)
             );

             expect(doSomethingMock).toHaveBeenCalled();
             expect(result).toBe(123); // Assuming doStuffWithDependency returns the number
        });
    });
    ```

## 4. Common Pitfalls Encountered (Avoid These)

*   **Incorrect Tag Definition:** Defining the Tag without the interface type (`Effect.Tag("MyTag")`) or using the Tag class itself in the generic (`Effect.Tag<MyServiceTag>`). **Use:** `class MyService extends Effect.Tag("MyId")<MyService, IMyService>() {}`.
*   **Incorrect Layer Definition:** Passing the Tag class instead of the Tag instance to `Layer.succeed`/`Layer.effect`. Using a plain implementation object `{...}` instead of wrapping it with `Tag.of({...})`. **Use:** `Layer.succeed(MyServiceTag, MyServiceTag.of({ implementation }))`.
*   **Incorrect Service Access:** Using `yield* _(MyServiceInterface)` instead of `yield* _(MyServiceTag)`. **Use:** `yield* MyServiceTag`.
*   **Incomplete Mocks:** Providing a mock implementation object to `Layer.succeed(Tag, Tag.of({...}))` that is missing methods defined in the service interface. **Fix:** Ensure the mock object implements *all* methods from the interface.
*   **Incorrect Mock Layering:** Providing a plain object `{...}` as a mock layer instead of using `Layer.succeed(Tag, Tag.of({...}))`. Adding `_tag` manually to plain mock objects. **Use:** The correct `Layer.succeed(Tag, Tag.of({...}))` pattern.
*   **Test Helper Misuse:** Using `Effect.flatMap(TagConstructor, ...)` instead of `Effect.flatMap(TagInstance, ...)` inside test helpers.
*   **Deleting Test Logic:** Removing tests or replacing logic with placeholders instead of fixing the underlying pattern errors. **Fix:** Understand the pattern, fix the error, keep the test logic.
*   **Directly Mocking `node:fs` with Vitest/ESM:** This caused persistent initialization errors (`Cannot access '__vi_import_0__'` ). **Fix:** Use Dependency Injection with `@effect/platform/FileSystem`.

## 5. Guidance Summary for Future Agents

1.  **Define Services Correctly:** Use the `interface` for the contract and the `class MyService extends Effect.Tag("Identifier")<MyService, InterfaceType>() {}` pattern for the Tag.
2.  **Implement Layers Correctly:** Use `Layer.succeed(TagInstance, TagInstance.of({ /* sync impl */ }))` or `Layer.effect(TagInstance, Effect.gen(... return TagInstance.of({ /* effectful impl */ })))`.
3.  **Access Services Correctly:** Use `yield* TagInstance` within `Effect.gen`.
4.  **Test Services Correctly:**
    *   Create mock implementations matching the full service interface.
    *   Provide mocks using `Layer.succeed(TagInstance, TagInstance.of({ /* mock impl */ }))`.
    *   Compose layers using `Layer.provide` or `Layer.mergeAll`.
    *   Run test effects using `Effect.provide(effectToTest, combinedTestLayer)`.
    *   Use `Effect.either` to test failure cases robustly.
5.  **Handle Dependencies:** For external dependencies like `fs`, use services provided by `@effect/platform` (like `FileSystem`) and inject them. Provide mock layers (`FileSystem.layerNoop({...overrides})`) in tests.
6.  **Verify Incrementally:** Run `pnpm check` and `pnpm test:run` frequently after changes.
7.  **DO NOT DELETE TESTS:** If tests fail, understand *why* based on these patterns and fix the underlying issue in the code or the test setup. Do not take shortcuts. Ask for guidance if stuck.

By adhering to these established Effect.js patterns for defining, providing, accessing, and testing services, future development should be smoother and avoid these common type errors and test failures.
