/**
 * QA-3 (#8908): the renderer half of the Desktop visual-baseline harness.
 *
 * When the probe loads the renderer with `?visualBaseline=<state>`, this
 * module mounts the REAL React workbench (the same `mountReactWorkbench`
 * production boot uses) over one frozen fixture `DesktopShellState` from
 * `visual-baseline-fixtures.ts` — no preload bridge, no providers, no
 * network. Determinism measures taken here:
 *
 * - `Date` is frozen to `VISUAL_BASELINE_FROZEN_NOW_ISO` before mount so
 *   relative timestamps ("4m") and any incidental clock read are constant.
 * - Animations, transitions, and the composer caret are disabled by an
 *   injected stylesheet so no frame-timing dependent pixels exist.
 * - The ready flag (`data-visual-baseline-ready="1"` on <html>) is set only
 *   after the shell mounted, `document.fonts.ready` resolved, and two
 *   animation frames settled — the probe captures strictly after that.
 */
import { Effect, Exit, Scope, SubscriptionRef } from "@effect-native/core/effect";
import type { IntentReporter } from "@effect-native/core";
import { migrateDesktopPreferences } from "../desktop-preferences-contract.ts";
import { preferencesRootAttributes, themeForPreferences } from "../desktop-preferences-effects.ts";
import { mountReactWorkbench } from "./react-primitive-adapters.tsx";
import {
  VISUAL_BASELINE_FROZEN_NOW_ISO,
  isVisualBaselineShellStateName,
  isVisualBaselineStateName,
  visualBaselineShellState,
} from "./visual-baseline-fixtures.ts";
import { mountVisualBaselineWorkbench } from "./visual-baseline-workbench.tsx";

/** Freeze the renderer clock: `new Date()` and `Date.now()` return the fixture
 * instant; explicit-argument construction stays untouched (fixture parsing). */
const freezeClock = (frozenIso: string): void => {
  const RealDate = Date;
  const frozenMs = RealDate.parse(frozenIso);
  class FrozenDate extends RealDate {
    constructor(...args: ReadonlyArray<unknown>) {
      if (args.length === 0) super(frozenMs);
      else super(...(args as [string | number | Date]));
    }
    static override now(): number {
      return frozenMs;
    }
  }
  (globalThis as { Date: DateConstructor }).Date = FrozenDate as unknown as DateConstructor;
};

const disableMotion = (documentRef: Document): void => {
  const style = documentRef.createElement("style");
  style.textContent = [
    "*, *::before, *::after {",
    "  animation: none !important;",
    "  transition: none !important;",
    "  caret-color: transparent !important;",
    "  scroll-behavior: auto !important;",
    "}",
  ].join("\n");
  documentRef.head.appendChild(style);
};

const nextFrame = (): Promise<void> =>
  new Promise((resolve) => requestAnimationFrame(() => resolve()));

/**
 * Mount one fixture state and resolve once the frame is capture-stable.
 * Unknown state names mark `data-visual-baseline-error` so the probe fails
 * loudly instead of capturing a blank frame.
 */
export const mountVisualBaseline = async (root: HTMLElement, stateName: string): Promise<void> => {
  if (!isVisualBaselineStateName(stateName)) {
    document.documentElement.dataset.visualBaselineError = `unknown state: ${stateName}`;
    return;
  }
  freezeClock(VISUAL_BASELINE_FROZEN_NOW_ISO);
  disableMotion(document);
  const preferences = migrateDesktopPreferences(undefined).preferences;
  for (const [name, value] of Object.entries(preferencesRootAttributes(preferences))) {
    document.documentElement.setAttribute(name, value);
  }
  const theme = themeForPreferences(preferences);
  if (isVisualBaselineShellStateName(stateName)) {
    const report: IntentReporter = () => Effect.void;
    const scope = Effect.runSync(Scope.make());
    window.addEventListener(
      "pagehide",
      () => {
        void Effect.runPromise(Scope.close(scope, Exit.void));
      },
      { once: true },
    );
    await Effect.runPromise(
      Scope.provide(scope)(
        Effect.gen(function* () {
          const state = yield* SubscriptionRef.make(visualBaselineShellState(stateName));
          yield* mountReactWorkbench(root, SubscriptionRef.changes(state), report, { theme });
        }),
      ),
    );
  } else {
    mountVisualBaselineWorkbench(root, stateName, theme);
  }
  document.getElementById("openagents-boot-frame")?.remove();
  await document.fonts.ready;
  await nextFrame();
  await nextFrame();
  document.documentElement.dataset.visualBaselineState = stateName;
  document.documentElement.dataset.visualBaselineReady = "1";
};
