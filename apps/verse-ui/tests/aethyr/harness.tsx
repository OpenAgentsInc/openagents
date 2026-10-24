// Browser harness for the renderer tests: bundled by the test, loaded into
// a page with the built ui.css. It renders a view tree (recording intents)
// or runs the entry controller with recording services.
import { createRoot, type Root } from "react-dom/client";
import { flushSync } from "react-dom";
import { Duration, Effect, Layer, ManagedRuntime } from "effect";
import * as Controller from "../../src/aethyr/controller";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import { RegistryContext } from "@effect/atom-react";
import { AethyrView } from "../../src/aethyr/renderer";
import { ControllerView } from "../../src/aethyr/host";
import {
  EntryOptions,
  GuestMemory,
  Navigator,
  makeEntryController,
} from "../../src/aethyr/entry";
import type { Intent, View } from "../../src/aethyr/protocol";

declare global {
  interface Window {
    harness: {
      intents: Intent[];
      trail: string[];
      render: (view: View) => void;
      entry: (options: {
        accounts: boolean;
        hauntPath?: string;
      }) => Promise<void>;
      /** A field whose handler takes `delay` ms per keystroke. */
      slowField: (delay: number) => Promise<void>;
      /** The slow field's controller state. */
      fieldValue: () => string;
    };
  }
}

let root: Root | undefined;
const container = () => {
  root?.unmount();
  const element = document.getElementById("root")!;
  element.replaceChildren();
  root = createRoot(element);
  return root;
};

window.harness = {
  intents: [],
  trail: [],
  render(view) {
    const target = container();
    flushSync(() =>
      target.render(
        <AethyrView
          view={view}
          onIntent={(intent) => {
            window.harness.intents.push(intent);
          }}
        />,
      ),
    );
  },
  async entry(options) {
    const registry = AtomRegistry.make();
    const trail = window.harness.trail;
    const runtime = ManagedRuntime.make(
      Layer.mergeAll(
        Layer.succeed(AtomRegistry.AtomRegistry, registry),
        Layer.succeed(EntryOptions, {
          accounts: options.accounts,
          hauntPath: options.hauntPath,
        }),
        Layer.succeed(Navigator, {
          go: (path: string) =>
            Effect.sync(() => void trail.push(`go ${path}`)),
        }),
        Layer.succeed(GuestMemory, {
          remember: Effect.sync(() => void trail.push("guest")),
        }),
      ),
    );
    const controller = await runtime.runPromise(
      makeEntryController("entry-browser"),
    );
    const target = container();
    flushSync(() =>
      target.render(
        <RegistryContext.Provider value={registry}>
          <ControllerView
            controller={controller}
            run={(effect) => runtime.runPromiseExit(effect)}
            onError={(message) => trail.push(`error ${message}`)}
          />
        </RegistryContext.Provider>,
      ),
    );
  },
  fieldValue: () => "",
  async slowField(delay) {
    const registry = AtomRegistry.make();
    const runtime = ManagedRuntime.make(
      Layer.succeed(AtomRegistry.AtomRegistry, registry),
    );
    const controller = await runtime.runPromise(
      Controller.make<{ name: string }>({
        instance: "slow-field",
        initial: { name: "" },
        render: (state) => ({
          id: "slow",
          element: {
            kind: "field",
            label: "Planet name",
            value: state.name,
            max_bytes: 64,
          },
        }),
        handle: (state, intent) =>
          Effect.gen(function* () {
            yield* Effect.sleep(Duration.millis(delay));
            return intent.kind === "input"
              ? { name: intent.value ?? "" }
              : state;
          }),
      }),
    );
    window.harness.fieldValue = () => registry.get(controller.state).name;
    const target = container();
    flushSync(() =>
      target.render(
        <RegistryContext.Provider value={registry}>
          <ControllerView
            controller={controller}
            run={(effect) => runtime.runPromiseExit(effect)}
          />
        </RegistryContext.Provider>,
      ),
    );
  },
};
