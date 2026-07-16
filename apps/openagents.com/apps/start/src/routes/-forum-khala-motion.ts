import {
  makeKhalaChoreography,
  planKhalaChoreography,
  runKhalaDomMotion,
  type KhalaTransitionState,
} from "@effect-native/khala-ui";
import { Deferred, Effect, Scope } from "effect";

const forumBoardAssemblyId = "forumBoardAssembly";
export const forumBoardAssemblyDurationMillis = 240;

export type ForumKhalaAssemblyPhase =
  | "hidden-static"
  | "reduced-static"
  | "settled"
  | "started"
  | "unsupported-static";

export type ForumKhalaAssemblyReceipt = Readonly<{
  activeDrivers: number;
  phase: ForumKhalaAssemblyPhase;
  scheduledWork: number;
  state: KhalaTransitionState;
}>;

export type ForumKhalaVisibility = Readonly<{
  hidden: () => boolean;
  subscribe: (listener: () => void) => () => void;
}>;

export type ForumKhalaAssemblyDependencies = Readonly<{
  onReceipt?: (receipt: ForumKhalaAssemblyReceipt) => void;
  reducedMotion?: boolean;
  visibility?: ForumKhalaVisibility;
}>;

const browserVisibility: ForumKhalaVisibility = {
  hidden: () => typeof document !== "undefined" && document.visibilityState === "hidden",
  subscribe: (listener) => {
    if (typeof document === "undefined") {
      return () => undefined;
    }
    document.addEventListener("visibilitychange", listener);
    return () => document.removeEventListener("visibilitychange", listener);
  },
};

const browserReducedMotion = (): boolean =>
  typeof window !== "undefined" &&
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

const forumBoardDecoration = (
  container: HTMLElement,
): Effect.Effect<HTMLElement | SVGElement | null> => {
  const find = () =>
    container.querySelector<HTMLElement | SVGElement>(
      '[data-en-key="forum-index-panel"] [data-en-khala-decoration]',
    );
  const current = find();
  if (current !== null || typeof MutationObserver === "undefined") {
    return Effect.succeed(current);
  }
  return Effect.callback<HTMLElement | SVGElement | null>((resume) => {
    const observer = new MutationObserver(() => {
      const decoration = find();
      if (decoration === null) return;
      observer.disconnect();
      resume(Effect.succeed(decoration));
    });
    observer.observe(container, { childList: true, subtree: true });
    const decoration = find();
    if (decoration !== null) {
      observer.disconnect();
      resume(Effect.succeed(decoration));
    }
    return Effect.sync(() => observer.disconnect());
  });
};

/**
 * Mount one non-gating board-decoration assembly into the Forum's existing
 * Effect Scope. Semantic content is outside the animated node and is already
 * visible. Reduced/hidden hosts take a synchronous stable path with no driver,
 * timer, listener, or fallback scheduler.
 */
export const mountForumBoardAssembly = (
  container: HTMLElement,
  dependencies: ForumKhalaAssemblyDependencies = {},
): Effect.Effect<void, never, Scope.Scope> =>
  Effect.gen(function* () {
    const decoration = yield* forumBoardDecoration(container);
    if (decoration === null) {
      return;
    }

    const visibility = dependencies.visibility ?? browserVisibility;
    const hiddenAtMount = visibility.hidden();
    const reducedMotion = dependencies.reducedMotion ?? browserReducedMotion();
    const unsupportedMotion = typeof decoration.animate !== "function";
    const staticMode = hiddenAtMount || reducedMotion || unsupportedMotion;
    const choreography = yield* makeKhalaChoreography({
      reducedMotion: staticMode,
    });
    const plan = planKhalaChoreography({
      manager: "parallel",
      target: "entered",
      children: [
        {
          id: forumBoardAssemblyId,
          enterMillis: forumBoardAssemblyDurationMillis,
          exitMillis: forumBoardAssemblyDurationMillis,
        },
      ],
    });
    const stableFrame = runKhalaDomMotion(
      decoration,
      { _tag: "FrameAssembly", phase: "line" },
      "enter",
      { durationMillis: 0, reducedMotion: true },
    );
    const receipt = (phase: ForumKhalaAssemblyPhase) =>
      Effect.gen(function* () {
        const [activeDrivers, scheduledWork, state] = yield* Effect.all([
          choreography.activeDrivers,
          choreography.scheduledWork,
          choreography.state(forumBoardAssemblyId),
        ]);
        yield* Effect.sync(() => {
          decoration.dataset.khalaMotion = phase;
          dependencies.onReceipt?.({
            activeDrivers,
            phase,
            scheduledWork,
            state,
          });
        });
      });

    yield* choreography.runPlan(plan);
    if (staticMode) {
      yield* stableFrame;
      yield* receipt(
        hiddenAtMount ? "hidden-static" : reducedMotion ? "reduced-static" : "unsupported-static",
      );
      return;
    }

    yield* receipt("started");
    const hidden = yield* Deferred.make<void>();
    const driverStarted = yield* Deferred.make<void>();
    const activeMotion = Effect.all(
      [
        choreography.awaitIdle,
        runKhalaDomMotion(decoration, { _tag: "FrameAssembly", phase: "line" }, "enter", {
          durationMillis: forumBoardAssemblyDurationMillis,
        }),
      ],
      { concurrency: "unbounded", discard: true },
    );
    const hiddenSignal = Deferred.await(hidden);
    const driver = Effect.acquireUseRelease(
      Effect.sync(() =>
        visibility.subscribe(() => {
          if (visibility.hidden()) {
            Deferred.doneUnsafe(hidden, Effect.void);
          }
        }),
      ).pipe(Effect.tap(() => Deferred.succeed(driverStarted, undefined))),
      () => Effect.raceFirst(activeMotion, hiddenSignal),
      (unsubscribe) => Effect.sync(unsubscribe),
    ).pipe(
      Effect.ensuring(Effect.all([choreography.dispose, stableFrame], { discard: true })),
      Effect.andThen(receipt("settled")),
    );
    yield* Effect.forkScoped(driver);
    yield* Deferred.await(driverStarted);
  });
