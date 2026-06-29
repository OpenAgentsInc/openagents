// The browser page seam.
//
// `ComputerUsePage` is the narrow contract the browser tool drives. The REAL
// implementation (see `playwright-page.ts`) is a thin adapter over a Playwright
// `Page` + `BrowserContext`; tests inject a deterministic fake. This split is
// what keeps unit CI free of chromium and the network while the real-chromium
// path stays provable via the qa-runner (#6176/#6177).
//
// Deterministic discipline: `waitFor` takes a *condition*, never a sleep.

export type WaitForCondition =
  | { readonly kind: "url-includes"; readonly value: string }
  | { readonly kind: "url-not-includes"; readonly value: string }
  | { readonly kind: "text-visible"; readonly value: string }
  | { readonly kind: "selector-visible"; readonly selector: string };

export interface ComputerUsePage {
  /** Navigate to a (relative or absolute) URL; resolves on load. */
  readonly navigate: (url: string) => Promise<void>;
  /** Current page URL. */
  readonly url: () => Promise<string>;
  /** Click an element by role-or-CSS selector. */
  readonly click: (selector: string) => Promise<void>;
  /** Type text into an element by selector. */
  readonly type: (selector: string, text: string) => Promise<void>;
  /** Read visible text content (optionally scoped to a selector). */
  readonly readText: (selector?: string) => Promise<string>;
  /** Read the (outer) DOM HTML (optionally scoped to a selector). */
  readonly readDom: (selector?: string) => Promise<string>;
  /**
   * Wait until a CONDITION holds (never a sleep). Resolves true if the
   * condition became true within the deadline, false on timeout.
   */
  readonly waitFor: (
    condition: WaitForCondition,
    options?: { readonly timeoutMs?: number },
  ) => Promise<boolean>;
  /** Capture a screenshot to `path` (png). */
  readonly screenshot: (path: string) => Promise<void>;
}
