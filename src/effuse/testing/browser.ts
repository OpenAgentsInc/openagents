/**
 * Effuse TestBrowser Service
 *
 * Effect-native DOM interaction for testing.
 * Provides queries, actions, assertions, and waiting utilities.
 */

import { Context, Effect } from "effect"
import { TestError, type WaitOptions } from "./errors.js"

/**
 * Service interface for browser/DOM testing operations.
 *
 * All methods return Effects for composability and proper error handling.
 * Can be backed by Happy-DOM (fast) or Playwright (real browser).
 */
export interface TestBrowser {
  // ─────────────────────────────────────────────────────────────────
  // Queries
  // ─────────────────────────────────────────────────────────────────

  /**
   * Query for an element by CSS selector.
   * Fails with TestError if not found.
   */
  readonly query: <T extends Element = Element>(
    selector: string
  ) => Effect.Effect<T, TestError>

  /**
   * Query for an element, returning null if not found.
   * Never fails.
   */
  readonly queryOption: <T extends Element = Element>(
    selector: string
  ) => Effect.Effect<T | null, never>

  /**
   * Query for all elements matching selector.
   */
  readonly queryAll: <T extends Element = Element>(
    selector: string
  ) => Effect.Effect<T[], never>

  // ─────────────────────────────────────────────────────────────────
  // Actions
  // ─────────────────────────────────────────────────────────────────

  /**
   * Click an element by selector.
   */
  readonly click: (selector: string) => Effect.Effect<void, TestError>

  /**
   * Type text into an input element.
   */
  readonly type: (
    selector: string,
    text: string
  ) => Effect.Effect<void, TestError>

  /**
   * Clear an input element's value.
   */
  readonly clear: (selector: string) => Effect.Effect<void, TestError>

  /**
   * Check or uncheck a checkbox.
   */
  readonly check: (
    selector: string,
    checked?: boolean
  ) => Effect.Effect<void, TestError>

  /**
   * Dispatch a custom event on an element.
   */
  readonly dispatchEvent: (
    selector: string,
    event: Event | string
  ) => Effect.Effect<void, TestError>

  // ─────────────────────────────────────────────────────────────────
  // Inspection
  // ─────────────────────────────────────────────────────────────────

  /**
   * Get the innerHTML of an element.
   */
  readonly getInnerHTML: (selector: string) => Effect.Effect<string, TestError>

  /**
   * Get the text content of an element.
   */
  readonly getText: (selector: string) => Effect.Effect<string, TestError>

  /**
   * Get an attribute value from an element.
   */
  readonly getAttribute: (
    selector: string,
    attribute: string
  ) => Effect.Effect<string | null, TestError>

  /**
   * Check if an element is visible (has display/visibility).
   */
  readonly isVisible: (selector: string) => Effect.Effect<boolean, never>

  /**
   * Check if an element exists in the DOM.
   */
  readonly exists: (selector: string) => Effect.Effect<boolean, never>

  // ─────────────────────────────────────────────────────────────────
  // Assertions (Effect-native)
  // ─────────────────────────────────────────────────────────────────

  /**
   * Assert that an element contains specific text.
   * Fails with TestError if not.
   */
  readonly expectText: (
    selector: string,
    text: string
  ) => Effect.Effect<void, TestError>

  /**
   * Assert that an element is visible.
   */
  readonly expectVisible: (selector: string) => Effect.Effect<void, TestError>

  /**
   * Assert that an element is hidden.
   */
  readonly expectHidden: (selector: string) => Effect.Effect<void, TestError>

  /**
   * Assert that selector matches exactly N elements.
   */
  readonly expectCount: (
    selector: string,
    count: number
  ) => Effect.Effect<void, TestError>

  /**
   * Assert that an element has a specific attribute value.
   */
  readonly expectAttribute: (
    selector: string,
    attribute: string,
    value: string
  ) => Effect.Effect<void, TestError>

  // ─────────────────────────────────────────────────────────────────
  // Waiting (Stream-based internally)
  // ─────────────────────────────────────────────────────────────────

  /**
   * Wait for an element to appear in the DOM.
   */
  readonly waitFor: (
    selector: string,
    options?: WaitOptions
  ) => Effect.Effect<Element, TestError>

  /**
   * Wait for an element to be removed from the DOM.
   */
  readonly waitForHidden: (
    selector: string,
    options?: WaitOptions
  ) => Effect.Effect<void, TestError>

  /**
   * Wait for an element to contain specific text.
   */
  readonly waitForText: (
    selector: string,
    text: string,
    options?: WaitOptions
  ) => Effect.Effect<void, TestError>
}

/**
 * Effect Context.Tag for TestBrowser service.
 */
export class TestBrowserTag extends Context.Tag("effuse/testing/TestBrowser")<
  TestBrowserTag,
  TestBrowser
>() {}
