// Browser harness for the renderer tests: bundled by the test, loaded into
// a page with the built ui.css. It renders a view tree (recording intents)
// or runs the entry controller with recording services.
import { createRoot, type Root } from "react-dom/client";
import { flushSync } from "react-dom";
import { useEffect, useRef, useState } from "react";
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
      /** A field whose handler answers at once but whose tree only shows
       *  the value on the next poll, as the house's bridge does. */
      polledField: (interval: number) => void;
      /** The polled field's host value. */
      polledValue: () => string;
      /** The slow field's controller state. */
      fieldValue: () => string;
      /**
       * The house editor's shape (#12602): one controller tree with several
       * fields, each input set on the release's hidden form control and
       * shown in the tree only when a poll reads that form back. Every
       * field shares the tree's revision.
       */
      houseForm: (interval: number) => Promise<void>;
      /** The hidden form's values now. */
      houseValues: () => Record<string, string>;
      /** Read the hidden form into the tree now, as the poll does. */
      housePoll: () => void;
      /** The release sets one of its controls in code, which no poll has
       *  read yet. */
      houseSet: (id: string, value: string) => void;
    };
  }
}

let root: Root | undefined;
let houseTimer: ReturnType<typeof setInterval> | undefined;
const container = () => {
  clearInterval(houseTimer);
  root?.unmount();
  const element = document.getElementById("root")!;
  element.replaceChildren();
  root = createRoot(element);
  return root;
};

function PolledField({ interval }: { interval: number }) {
  const host = useRef("");
  const [view, setView] = useState<View>(() => polledView("", 1));
  useEffect(() => {
    window.harness.polledValue = () => host.current;
    const timer = setInterval(() => {
      setView((v) =>
        v.root.element.kind === "field" && v.root.element.value !== host.current
          ? polledView(host.current, v.revision + 1)
          : v,
      );
    }, interval);
    return () => clearInterval(timer);
  }, [interval]);
  return (
    <AethyrView
      view={view}
      onIntent={(intent) => {
        if (intent.kind === "input") host.current = intent.value ?? "";
        return Promise.resolve();
      }}
    />
  );
}
function polledView(value: string, revision: number): View {
  return {
    schema: "aethyr.ui.v1",
    instance: "polled-field",
    revision,
    root: {
      id: "polled",
      element: {
        kind: "field",
        label: "Request",
        value,
        multiline: true,
        max_bytes: 4800,
      },
    },
  } as View;
}

window.harness = {
  intents: [],
  trail: [],
  polledField(interval) {
    const target = container();
    flushSync(() => target.render(<PolledField interval={interval} />));
  },
  polledValue: () => "",
  render(view) {
    const target = container();
    flushSync(() =>
      target.render(
        <AethyrView
          view={view}
          onIntent={(intent) => {
            window.harness.intents.push(intent);
          }}
          // A host canvas, as the engine registers one, and the files a
          // slot receives (names and sizes only).
          mount={(kind, id, element) => {
            if (kind !== "canvas" || !element || element.firstChild) return;
            const canvas = document.createElement("canvas");
            canvas.id = id;
            canvas.width = 256;
            canvas.height = 256;
            element.append(canvas);
          }}
          onFiles={(slot, files) => {
            window.harness.trail.push(
              `files ${slot} ${files.map((f) => `${f.name}:${f.size}`).join(",")}`,
            );
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
  houseValues: () => ({}),
  housePoll: () => {},
  houseSet: () => {},
  async houseForm(interval) {
    // The release's own form, hidden, with the controls studio_browser.rs
    // builds: number inputs that are required.
    document.getElementById("house-release")?.remove();
    const form = document.createElement("form");
    form.id = "house-release";
    form.hidden = true;
    for (const id of ["house-x", "house-y"]) {
      const input = document.createElement("input");
      input.id = id;
      input.type = "number";
      input.step = "any";
      input.required = true;
      input.value = "0";
      form.append(input);
    }
    const note = document.createElement("textarea");
    note.id = "house-note";
    form.append(note);
    document.body.append(form);
    const control = (id: string) =>
      document.getElementById(id) as HTMLInputElement;
    const read = () => ({
      x: control("house-x").value,
      y: control("house-y").value,
      note: control("house-note").value,
    });
    const field = (id: string, label: string, value: string) => ({
      id,
      element: { kind: "field" as const, label, value, max_bytes: 4800 },
    });
    const registry = AtomRegistry.make();
    const runtime = ManagedRuntime.make(
      Layer.succeed(AtomRegistry.AtomRegistry, registry),
    );
    let polls = 0;
    const controller = await runtime.runPromise(
      Controller.make<{ form: ReturnType<typeof read>; polls: number }>({
        instance: "house-form",
        initial: { form: read(), polls },
        render: (state) => ({
          id: "house-editor",
          element: {
            kind: "stack",
            axis: "vertical",
            children: [
              field("house-x", "X", state.form.x),
              field("house-y", "Y", state.form.y),
              field("house-note", "Note", state.form.note),
              {
                id: "house-polls",
                element: {
                  kind: "text",
                  role: "caption",
                  value: `Read ${state.polls} times`,
                },
              },
            ],
          },
        }),
        // The bridge: set the release's control and announce it. The tree
        // does not change; the next poll shows the value.
        handle: (state, intent) =>
          Effect.sync(() => {
            if (intent.kind === "input") {
              const element = control(intent.node);
              element.value = intent.value ?? "";
              element.dispatchEvent(new Event("input", { bubbles: true }));
            }
            return state;
          }),
      }),
    );
    // Each poll reads the form, and also something else the release
    // changed, so every poll draws a new revision.
    const poll = () =>
      controller.update(() => ({ form: read(), polls: ++polls }));
    window.harness.houseValues = read;
    window.harness.housePoll = () => runtime.runSync(poll());
    window.harness.houseSet = (id, value) => void (control(id).value = value);
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
    houseTimer = setInterval(() => runtime.runFork(poll()), interval);
  },
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
