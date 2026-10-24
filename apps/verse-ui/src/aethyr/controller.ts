// The Aethyr controller runtime (ADR 0005). A controller owns state in an
// Effect Atom registry, publishes view-tree revisions, and handles intents.
// Its dependencies are Effect services provided by layers, so a controller
// runs and is tested without a DOM. It never touches pixels.
import { Data, Effect, Semaphore } from "effect";
import * as Atom from "effect/reactivity/Atom";
import { AtomRegistry } from "effect/reactivity/AtomRegistry";
import { SCHEMA, type Intent, type Node, type View } from "./protocol";
import {
  acceptIntent,
  validateView,
  type IntentErrorCode,
  type ViewErrorCode,
} from "./validate";

/** The renderer sent an intent this revision does not accept. */
export class IntentRejected extends Data.TaggedError("IntentRejected")<{
  readonly code: IntentErrorCode;
  readonly node: string;
}> {}

/** The controller built a tree that fails the protocol's checks. */
export class InvalidView extends Data.TaggedError("InvalidView")<{
  readonly code: ViewErrorCode;
  readonly detail?: string | undefined;
}> {}

/**
 * What a controller is: its state, how the state looks as a tree, and what
 * each accepted intent does to the state. `handle` may call services (R) and
 * fail (E); the state only changes when it succeeds.
 */
export interface Definition<S, E = never, R = never> {
  /** Fresh for each controller lifetime. */
  readonly instance: string;
  readonly initial: S;
  /** Pure: the same state always gives the same tree. */
  readonly render: (state: S) => Node;
  readonly handle: (
    state: S,
    intent: Intent,
    node: Node,
  ) => Effect.Effect<S, E, R>;
}

export interface Published<S> {
  readonly state: S;
  readonly view: View;
}

export interface Controller<S, E = never, R = never> {
  readonly instance: string;
  /** State and its published view, together. */
  readonly published: Atom.Atom<Published<S>>;
  /** The latest revision, for renderers. */
  readonly view: Atom.Atom<View>;
  readonly state: Atom.Atom<S>;
  readonly current: Effect.Effect<View>;
  /** Check an intent against the current revision, then handle it. */
  readonly dispatch: (
    intent: Intent,
  ) => Effect.Effect<View, IntentRejected | InvalidView | E, R>;
  /** Change state from outside an intent, such as a service result. */
  readonly update: (f: (state: S) => S) => Effect.Effect<View, InvalidView>;
}

const sameTree = (a: Node, b: Node) => JSON.stringify(a) === JSON.stringify(b);

/**
 * Publish `state` after `previous`: the same revision when the tree did not
 * change, otherwise the next one.
 */
function publish<S, E, R>(
  definition: Definition<S, E, R>,
  state: S,
  previous: View | undefined,
): Effect.Effect<Published<S>, InvalidView> {
  return Effect.suspend(() => {
    const root = definition.render(state);
    if (previous && sameTree(previous.root, root))
      return Effect.succeed({ state, view: previous });
    const view: View = {
      schema: SCHEMA,
      instance: definition.instance,
      revision: (previous?.revision ?? 0) + 1,
      root,
    };
    const checked = validateView(view);
    return checked.ok
      ? Effect.succeed({ state, view })
      : Effect.fail(
          new InvalidView({ code: checked.code, detail: checked.detail }),
        );
  });
}

export const make = <S, E = never, R = never>(
  definition: Definition<S, E, R>,
): Effect.Effect<Controller<S, E, R>, InvalidView, AtomRegistry> =>
  Effect.gen(function* () {
    const registry = yield* AtomRegistry;
    const first = yield* publish(definition, definition.initial, undefined);
    const published = Atom.keepAlive(Atom.make(first));
    const view = Atom.keepAlive(Atom.make((get) => get(published).view));
    const state = Atom.keepAlive(Atom.make((get) => get(published).state));
    // One intent at a time, in arrival order; a later intent that names the
    // revision an earlier one replaced is refused as stale.
    const lock = yield* Semaphore.make(1);
    const commit = (next: S) =>
      Effect.gen(function* () {
        const before = registry.get(published);
        const after = yield* publish(definition, next, before.view);
        registry.set(published, after);
        return after.view;
      });
    const dispatch = (intent: Intent) =>
      lock.withPermit(
        Effect.gen(function* () {
          const { state: current, view: drawn } = registry.get(published);
          const checked = acceptIntent(drawn, intent);
          if (!checked.ok)
            return yield* new IntentRejected({
              code: checked.code,
              node: intent.node,
            });
          const next = yield* definition.handle(current, intent, checked.value);
          return yield* commit(next);
        }),
      );
    const update = (f: (state: S) => S) =>
      lock.withPermit(
        Effect.suspend(() => commit(f(registry.get(published).state))),
      );
    return {
      instance: definition.instance,
      published,
      view,
      state,
      current: Effect.sync(() => registry.get(published).view),
      dispatch,
      update,
    } satisfies Controller<S, E, R>;
  });

/** An intent for `node` in `view`, as a renderer builds it. */
export const intentFor = (
  view: View,
  node: string,
  kind: Intent["kind"],
  value?: string,
): Intent =>
  value === undefined
    ? { instance: view.instance, revision: view.revision, node, kind }
    : { instance: view.instance, revision: view.revision, node, kind, value };
