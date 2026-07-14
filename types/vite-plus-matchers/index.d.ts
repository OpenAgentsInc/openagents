import "vitest"
import "@vitest/expect"

declare module "vitest" {
  interface Matchers<T = unknown> {
    toBeAbsent(): T
    toBeFunction(): T
    toBeString(): T
    toContainAllValues(expected: ReadonlyArray<unknown>): T
    toEndWith(expected: string): T
    toStartWith(expected: string): T
  }
}

declare module "@vitest/expect" {
  interface Assertion<T = unknown> {
    toBeAbsent(): void
    toBeFunction(): void
    toBeString(): void
    toContainAllValues(expected: ReadonlyArray<unknown>): void
    toEndWith(expected: string): void
    toStartWith(expected: string): void
  }
}

declare global {
  namespace Chai {
    interface Assertion {
      toBeAbsent(): void
      toBeFunction(): void
      toBeString(): void
      toContainAllValues(expected: ReadonlyArray<unknown>): void
      toEndWith(expected: string): void
      toStartWith(expected: string): void
    }
  }
}
