// Controllers run with no DOM: services come from layers, state lives in an
// Effect Atom registry, and each step is checked through published views.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Context, Effect, Layer } from "effect";
import {
  AtomRegistry,
  layer as registryLayer,
} from "effect/reactivity/AtomRegistry";
import * as Controller from "../src/aethyr/controller";
import {
  EntryOptions,
  GuestMemory,
  Navigator,
  PATHS,
  makeEntryController,
  renderEntry,
} from "../src/aethyr/entry";
import type { View } from "../src/aethyr/protocol";
import { findNode, parseView } from "../src/aethyr/validate";

const run = <A, E>(effect: Effect.Effect<A, E, never>) =>
  Effect.runPromise(effect);

class Clicks extends Context.Service<Clicks, { readonly log: string[] }>()(
  "test/Clicks",
) {}

const counter = Effect.gen(function* () {
  const clicks = yield* Clicks;
  return yield* Controller.make<{ count: number; note: string }, string>({
    instance: "counter",
    initial: { count: 0, note: "" },
    render: (state) => ({
      id: "root",
      element: {
        kind: "stack",
        axis: "vertical",
        children: [
          {
            id: "count",
            element: {
              kind: "text",
              value: `Count ${state.count}`,
              role: "status",
            },
          },
          {
            id: "more",
            element: { kind: "button", label: "More", emphasis: "primary" },
          },
          { id: "break", element: { kind: "button", label: "Break" } },
          { id: "same", element: { kind: "button", label: "Same" } },
          {
            id: "note",
            element: {
              kind: "field",
              label: "Note",
              value: state.note,
              max_bytes: 8,
            },
          },
        ],
      },
    }),
    handle: (state, intent) =>
      Effect.gen(function* () {
        clicks.log.push(intent.node);
        if (intent.node === "more") return { ...state, count: state.count + 1 };
        if (intent.node === "break")
          return yield* Effect.fail("refused by service");
        if (intent.node === "note")
          return { ...state, note: intent.value ?? "" };
        return state;
      }),
  });
});

const codeOf = (error: unknown) =>
  error instanceof Controller.IntentRejected ? error.code : String(error);

const activate = (view: View, node: string) =>
  Controller.intentFor(view, node, "activate");

test("a controller publishes revisions and handles intents without a DOM", async () => {
  const log: string[] = [];
  const layer = Layer.mergeAll(registryLayer, Layer.succeed(Clicks, { log }));
  await run(
    Effect.gen(function* () {
      const registry = yield* AtomRegistry;
      const controller = yield* counter;
      const first = yield* controller.current;
      assert.equal(first.revision, 1);
      assert.equal(first.instance, "counter");

      // The renderer reads the same atom.
      const seen: number[] = [];
      const stop = registry.subscribe(
        controller.view,
        (view) => seen.push(view.revision),
        { immediate: true },
      );

      const second = yield* controller.dispatch(activate(first, "more"));
      assert.equal(second.revision, 2);
      const count = findNode(second, "count")!.element;
      assert.equal(count.kind === "text" && count.value, "Count 1");

      // An intent for the replaced revision is stale and changes nothing.
      const stale = yield* Effect.flip(
        controller.dispatch(activate(first, "more")),
      );
      assert.equal(codeOf(stale), "stale");

      // A handled intent that leaves the tree alone keeps the revision.
      const same = yield* controller.dispatch(activate(second, "same"));
      assert.equal(same.revision, 2);

      // A failing service leaves the state and revision unchanged.
      const failed = yield* Effect.flip(
        controller.dispatch(activate(second, "break")),
      );
      assert.equal(failed, "refused by service");
      assert.equal((yield* controller.current).revision, 2);

      // Values are bounded by the tree, in UTF-8 bytes.
      const tooLong = yield* Effect.flip(
        controller.dispatch(
          Controller.intentFor(second, "note", "input", "ééééé"),
        ),
      );
      assert.equal(codeOf(tooLong), "bad_value");
      const typed = yield* controller.dispatch(
        Controller.intentFor(second, "note", "input", "hi"),
      );
      assert.equal(typed.revision, 3);
      assert.equal(registry.get(controller.state).note, "hi");

      // External updates publish too.
      const bumped = yield* controller.update((s) => ({ ...s, count: 10 }));
      assert.equal(bumped.revision, 4);

      // Subscribers hear about it on the registry's schedule, newest last.
      yield* Effect.promise(() => new Promise((done) => setTimeout(done, 10)));
      stop();
      assert.equal(seen[0], 1);
      assert.equal(seen.at(-1), 4);
      assert.deepEqual(
        seen,
        [...seen].sort((a, b) => a - b),
      );
      assert.deepEqual(log, ["more", "same", "break", "note"]);
    }).pipe(Effect.provide(layer)),
  );
});

test("a controller refuses to publish an invalid tree", async () => {
  const result = await run(
    Effect.flip(
      Controller.make({
        instance: "bad",
        initial: null,
        render: () => ({
          id: "root",
          element: { kind: "button", label: " " },
        }),
        handle: (state) => Effect.succeed(state),
      }),
    ).pipe(Effect.provide(registryLayer)),
  );
  assert.equal(result.code, "missing_label");
});

// ---------------------------------------------------------------------------
// The /new-lander/verse entry dialog: account or guest, choice, closed.

function entryLayer(options: { accounts: boolean; hauntPath?: string }) {
  const trail: string[] = [];
  const layer = Layer.mergeAll(
    registryLayer,
    Layer.succeed(EntryOptions, {
      accounts: options.accounts,
      hauntPath: options.hauntPath,
    }),
    Layer.succeed(Navigator, {
      go: (path: string) => Effect.sync(() => void trail.push(`go ${path}`)),
    }),
    Layer.succeed(GuestMemory, {
      remember: Effect.sync(() => void trail.push("guest")),
    }),
  );
  return { trail, layer };
}

const dialogOf = (view: View) => {
  const element = view.root.element;
  assert.equal(element.kind, "dialog");
  return element as Extract<typeof element, { kind: "dialog" }>;
};

test("the entry dialog goes from account to choice to closed", async () => {
  const { trail, layer } = entryLayer({
    accounts: true,
    hauntPath: "/haunted-house",
  });
  await run(
    Effect.gen(function* () {
      const entry = yield* makeEntryController("entry-test");
      const account = yield* entry.current;
      assert.equal(dialogOf(account).open, true);
      assert.ok(findNode(account, "verse-signin"));
      assert.ok(findNode(account, "continue-guest"));
      assert.equal(findNode(account, "verse-choices"), undefined);

      // The dialog cannot be dismissed; the person picks a way in.
      const dismiss = yield* Effect.flip(
        entry.dispatch(Controller.intentFor(account, "verse-entry", "dismiss")),
      );
      assert.equal(codeOf(dismiss), "not_accepted");

      const choice = yield* entry.dispatch(activate(account, "continue-guest"));
      assert.equal(choice.revision, 2);
      assert.equal(dialogOf(choice).open, true);
      assert.ok(findNode(choice, "verse-explore"));
      assert.ok(findNode(choice, "verse-haunt"));
      assert.equal(findNode(choice, "verse-account"), undefined);

      const closed = yield* entry.dispatch(activate(choice, "verse-explore"));
      assert.equal(closed.revision, 3);
      assert.equal(dialogOf(closed).open, false);
      assert.deepEqual(trail, ["guest"]);
    }).pipe(Effect.provide(layer)),
  );
});

test("sign-in and the Haunted House leave through the navigator", async () => {
  const { trail, layer } = entryLayer({
    accounts: true,
    hauntPath: "/haunted-house",
  });
  await run(
    Effect.gen(function* () {
      const entry = yield* makeEntryController("entry-nav");
      const account = yield* entry.current;
      const after = yield* entry.dispatch(activate(account, "verse-signin"));
      assert.equal(after.revision, 1);
      const choice = yield* entry.dispatch(activate(after, "continue-guest"));
      yield* entry.dispatch(activate(choice, "verse-haunt"));
      yield* entry.dispatch(activate(choice, "verse-entry-back"));
      assert.deepEqual(trail, [
        `go ${PATHS.signIn}`,
        "guest",
        "go /haunted-house",
        `go ${PATHS.home}`,
      ]);
    }).pipe(Effect.provide(layer)),
  );
});

test("without accounts or the house, those controls are absent", async () => {
  const { layer } = entryLayer({ accounts: false });
  await run(
    Effect.gen(function* () {
      const entry = yield* makeEntryController("entry-guest");
      const account = yield* entry.current;
      assert.equal(findNode(account, "verse-signin"), undefined);
      const signin = yield* Effect.flip(
        entry.dispatch(activate(account, "verse-signin")),
      );
      assert.equal(codeOf(signin), "unknown_node");
      const choice = yield* entry.dispatch(activate(account, "continue-guest"));
      assert.equal(findNode(choice, "verse-haunt"), undefined);
    }).pipe(Effect.provide(layer)),
  );
});

// The controller's views are fixtures for the Rust side of the round trip
// (crates/aethyr-ui, `controller_views_from_typescript_round_trip`).
test("entry views round-trip to Rust as fixtures", () => {
  const dir = join(
    import.meta.dirname,
    "../../../crates/aethyr-ui/fixtures/ts",
  );
  const options = { accounts: true, hauntPath: "/haunted-house" };
  const steps = ["account", "choice", "closed"] as const;
  const write = process.env.AETHYR_WRITE_FIXTURES === "1";
  if (write) mkdirSync(dir, { recursive: true });
  steps.forEach((step, index) => {
    const view: View = {
      schema: "aethyr.ui.v1",
      instance: "verse-entry-fixture",
      revision: index + 1,
      root: renderEntry({ step }, options),
    };
    assert.ok(parseView(JSON.stringify(view)).ok);
    const text = JSON.stringify(view, null, 2) + "\n";
    const file = join(dir, `entry-${step}.json`);
    if (write) writeFileSync(file, text);
    assert.ok(
      existsSync(file),
      `${file} is missing; run with AETHYR_WRITE_FIXTURES=1`,
    );
    assert.equal(
      readFileSync(file, "utf8"),
      text,
      `${file} is stale; run with AETHYR_WRITE_FIXTURES=1`,
    );
  });
});
