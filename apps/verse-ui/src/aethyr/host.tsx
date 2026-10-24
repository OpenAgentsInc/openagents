// Joins a controller to the renderer: the renderer reads the controller's
// view atom from the shared Effect Atom registry, and each intent runs the
// controller's `dispatch` in the host's runtime.
import { useCallback } from "react";
import { Cause, Effect, Exit } from "effect";
import { useAtomValue } from "@effect/atom-react";
import type { Controller } from "./controller";
import type { Intent, View } from "./protocol";
import { AethyrView, type Mount } from "./renderer";

/** Runs an effect in the host's runtime (a `ManagedRuntime`, usually). */
export type Run<R> = <A, E>(
  effect: Effect.Effect<A, E, R>,
) => Promise<Exit.Exit<A, E>>;

export interface ControllerViewProps<S, E, R> {
  readonly controller: Controller<S, E, R>;
  readonly run: Run<R>;
  readonly mount?: Mount;
  /** Told about failures other than stale intents. Never sees input values. */
  readonly onError?: (message: string) => void;
}

export function ControllerView<S, E, R>({
  controller,
  run,
  mount,
  onError,
}: ControllerViewProps<S, E, R>) {
  const view: View = useAtomValue(controller.view);
  const onIntent = useCallback(
    (intent: Intent) => {
      void run(controller.dispatch(intent)).then((exit) => {
        if (Exit.isSuccess(exit)) return;
        const error = Cause.squash(exit.cause);
        // A stale intent is expected after a newer revision; drop it.
        if (
          typeof error === "object" &&
          error !== null &&
          "_tag" in error &&
          error._tag === "IntentRejected" &&
          "code" in error &&
          error.code === "stale"
        )
          return;
        onError?.(
          `intent ${intent.kind} on ${intent.node} failed: ${
            typeof error === "object" && error !== null && "_tag" in error
              ? String(error._tag)
              : "error"
          }`,
        );
      });
    },
    [controller, run, onError],
  );
  return <AethyrView view={view} onIntent={onIntent} mount={mount} />;
}
