// Browser tool surface (computer-use).
//
// Gives Khala the browser actions a developer uses — navigate / click / type /
// readText / readDom / waitFor (condition, never sleep) / screenshot — over a
// `ComputerUsePage` seam, with executor's `acquireUseRelease` flush-on-timeout
// discipline: even if the `use` body is interrupted (vitest/bun timeout, kill,
// Effect fiber interruption), the `release` block STILL runs to close the page
// and flush artifacts (trace + video). Real chromium is wired in
// `playwright-page.ts`; unit tests inject a fake page.

import { Effect } from "effect";
import { type Timeline, makeTimeline } from "./timeline";
import type { ComputerUsePage, WaitForCondition } from "./page";

export interface BrowserSurface {
  readonly page: ComputerUsePage;
  readonly timeline: Timeline;
  readonly navigate: (url: string) => Promise<void>;
  readonly click: (selector: string, label?: string) => Promise<void>;
  readonly type: (selector: string, text: string, label?: string) => Promise<void>;
  readonly readText: (selector?: string) => Promise<string>;
  readonly readDom: (selector?: string) => Promise<string>;
  readonly waitFor: (
    condition: WaitForCondition,
    options?: { readonly timeoutMs?: number },
  ) => Promise<boolean>;
  readonly screenshot: (label: string) => Promise<string>;
}

/**
 * An acquired browser session: the live page plus the artifact-flush hook the
 * release block calls. `flush` MUST be safe to call exactly once and must close
 * the browser + persist any trace/video. The real adapter (Playwright) supplies
 * this; the fake supplies a record-only flush.
 */
export interface AcquiredBrowser {
  readonly page: ComputerUsePage;
  /** Close + flush artifacts. Guaranteed to run even on interruption. */
  readonly flush: () => Promise<void>;
}

export interface MakeBrowserSurfaceOptions {
  /** Directory artifacts (screenshots/trace/video) are written to. */
  readonly artifactDir: string;
  /** Injectable clock for deterministic timelines in tests. */
  readonly now?: () => number;
}

function describeCondition(condition: WaitForCondition): string {
  switch (condition.kind) {
    case "url-includes":
      return `url includes ${condition.value}`;
    case "url-not-includes":
      return `url does not include ${condition.value}`;
    case "text-visible":
      return `text visible "${condition.value}"`;
    case "selector-visible":
      return `selector visible ${condition.selector}`;
  }
}

function makeSurface(
  page: ComputerUsePage,
  options: MakeBrowserSurfaceOptions,
): BrowserSurface {
  const timeline = makeTimeline(options.now ? { now: options.now } : {});
  let shotCount = 0;
  const slug = (s: string) =>
    s
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 40);

  const tracked = async <A>(
    label: string,
    detail: Readonly<Record<string, string | number | boolean>>,
    run: () => Promise<A>,
  ): Promise<A> => {
    try {
      const result = await run();
      timeline.beat({ surface: "browser", label, status: "ok", detail });
      return result;
    } catch (error) {
      timeline.beat({
        surface: "browser",
        label,
        status: "error",
        detail: { ...detail, error: error instanceof Error ? error.message : String(error) },
      });
      throw error;
    }
  };

  return {
    page,
    timeline,
    navigate: (url) => tracked(`navigate to ${url}`, { url }, () => page.navigate(url)),
    click: (selector, label) =>
      tracked(label ?? `click ${selector}`, { selector }, () => page.click(selector)),
    type: (selector, text, label) =>
      // NOTE: the typed text value is intentionally NOT recorded in the beat
      // detail — it may be a credential. Only the selector + length are public.
      tracked(label ?? `type into ${selector}`, { selector, length: text.length }, () =>
        page.type(selector, text),
      ),
    readText: (selector) =>
      tracked("read text", selector ? { selector } : {}, () => page.readText(selector)),
    readDom: (selector) =>
      tracked("read dom", selector ? { selector } : {}, () => page.readDom(selector)),
    waitFor: (condition, opts) =>
      tracked(
        `wait for ${describeCondition(condition)}`,
        opts?.timeoutMs ? { timeoutMs: opts.timeoutMs } : {},
        () => page.waitFor(condition, opts),
      ),
    screenshot: async (label) => {
      const path = `${options.artifactDir}/${String(shotCount++).padStart(2, "0")}-${slug(label)}.png`;
      await tracked(`screenshot ${label}`, { path }, () => page.screenshot(path));
      return path;
    },
  };
}

/**
 * Run `use` against a browser surface acquired from `acquire`, guaranteeing the
 * acquired browser is flushed/closed even if `use` is interrupted or throws.
 *
 * This is the executor flush-on-timeout discipline expressed as an Effect
 * `acquireUseRelease`: a killed/timed-out run still closes chromium and flushes
 * the trace + video rather than leaking the process and losing artifacts.
 */
export function withBrowserSurface<A, E, R>(
  acquire: () => Promise<AcquiredBrowser>,
  options: MakeBrowserSurfaceOptions,
  use: (surface: BrowserSurface) => Effect.Effect<A, E, R>,
): Effect.Effect<A, E, R> {
  return Effect.acquireUseRelease(
    Effect.promise(async () => {
      const acquired = await acquire();
      return { acquired, surface: makeSurface(acquired.page, options) };
    }),
    ({ surface }) => use(surface),
    ({ acquired }) =>
      // RELEASE: flush is best-effort and must never mask the primary error.
      Effect.promise(() => acquired.flush().catch(() => undefined)),
  );
}
