// The /new-lander/verse controller with recording services and no DOM: the
// entry flow, the destination sheet, the engine's reports, and the links.
import { test } from "node:test";
import assert from "node:assert/strict";
import { Effect, Layer, ManagedRuntime } from "effect";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import {
  EntryOptions,
  GuestMemory,
  NO_PLANET,
  Navigator,
  UniverseEngine,
  VerseOptions,
  engineReport,
  initialVerse,
  makeVerseController,
  renderVerse,
  type VerseState,
  type World,
} from "../src/aethyr/verse";
import { intentFor } from "../src/aethyr/controller";
import type { Node, View } from "../src/aethyr/protocol";
import { findNode, validateView } from "../src/aethyr/validate";

const WORLDS: World[] = [
  {
    id: "openagents",
    name: "OpenAgents",
    category: "Company world · Locked",
    description: "Our home in the Verse.",
  },
  {
    id: "blue-rush",
    name: "Blue Rush",
    category: "Creator studio",
    description: "Stories, games, and worlds.",
  },
  {
    id: "haunted-house",
    name: "Haunted House",
    category: "A world by Blue Rush",
    description: "Step inside.",
  },
  {
    id: "creator-worlds",
    name: "My planet",
    category: "Locked",
    description: "Publish your own house.",
  },
  {
    id: "far-field",
    name: "The far field",
    category: "Locked",
    description: "An open horizon.",
  },
];

function setup(
  options: { signedIn?: boolean; haunt?: string; sdk?: boolean } = {},
  initial?: VerseState,
) {
  const trail: string[] = [];
  const registry = AtomRegistry.make();
  const runtime = ManagedRuntime.make(
    Layer.mergeAll(
      Layer.succeed(AtomRegistry.AtomRegistry, registry),
      Layer.succeed(EntryOptions, {
        accounts: true,
        hauntPath: options.haunt,
      }),
      Layer.succeed(VerseOptions, {
        worlds: WORLDS,
        signedIn: options.signedIn ?? false,
        blueRushPath: "/blue-rush",
        mapPath: "/new-lander/verse",
      }),
      Layer.succeed(Navigator, {
        go: (path: string) => Effect.sync(() => void trail.push(`go ${path}`)),
      }),
      Layer.succeed(GuestMemory, {
        remember: Effect.sync(() => void trail.push("guest")),
      }),
      Layer.succeed(UniverseEngine, {
        explore: Effect.sync(() => void trail.push("explore")),
        select: (world: string) =>
          Effect.sync(() => void trail.push(`select ${world}`)),
        signIn: (path: string) =>
          Effect.sync(() => void trail.push(`sign in ${path}`)),
        createPlanet: Effect.sync(() => void trail.push("create planet")),
        reload: Effect.sync(() => {
          trail.push("reload");
          return options.sdk ?? true;
        }),
        fullscreen: Effect.sync(() => void trail.push("fullscreen")),
      }),
    ),
  );
  const start =
    initial ?? initialVerse({ signedIn: options.signedIn ?? false, worlds: WORLDS });
  const controller = runtime.runSync(makeVerseController(start, "verse-test"));
  const view = () => registry.get(controller.view);
  const state = () => registry.get(controller.state);
  const act = (node: string) =>
    runtime.runPromise(controller.dispatch(intentFor(view(), node, "activate")));
  const report = (r: Parameters<typeof engineReport>[1]) =>
    runtime.runPromise(controller.update((s) => engineReport(s, r)));
  return { trail, view, state, act, report };
}

const node = (view: View, id: string) => findNode(view, id);
const element = (view: View, id: string) => node(view, id)?.element;
const open = (view: View, id: string) =>
  (element(view, id) as { open: boolean } | undefined)?.open;
const ids = (n: Node): string[] => [
  n.id,
  ...("children" in n.element ? n.element.children.flatMap(ids) : []),
];

test("a guest goes from the account step to the choice, then explores", async () => {
  const { trail, view, state, act } = setup({ haunt: "/new-lander/house" });
  assert.equal(state().entry, "account");
  assert.equal(open(view(), "verse-entry"), true);
  assert.equal(open(view(), "verse-panel"), false);
  assert.ok(node(view(), "continue-guest"));
  assert.ok(node(view(), "verse-signin"));
  // The world controls only show while exploring.
  assert.deepEqual(
    (element(view(), "verse-controls") as unknown as { children: Node[] })
      .children,
    [],
  );

  await act("continue-guest");
  assert.equal(state().entry, "choice");
  assert.ok(node(view(), "verse-explore"));
  assert.ok(node(view(), "verse-haunt"));

  await act("verse-explore");
  assert.equal(state().entry, "closed");
  assert.equal(open(view(), "verse-entry"), false);
  assert.equal(open(view(), "verse-panel"), true);
  assert.ok(node(view(), "world-retry"));
  assert.deepEqual(trail, ["guest", "explore"]);
  assert.equal(validateView(view()).ok, true);
});

test("signing in and the house tutorial leave through their services", async () => {
  const { trail, act } = setup({ haunt: "/new-lander/house" });
  await act("verse-signin");
  assert.deepEqual(trail, ["sign in /login?return_to=%2Fnew-lander%2Fverse"]);
  await act("continue-guest");
  await act("verse-haunt");
  assert.deepEqual(trail.slice(1), ["guest", "go /new-lander/house"]);
});

test("a signed-in visitor starts at the choice; a returning explorer starts on the map", () => {
  assert.equal(
    initialVerse({ signedIn: true, worlds: WORLDS }).entry,
    "choice",
  );
  const back = initialVerse(
    { signedIn: false, worlds: WORLDS },
    { entered: true, exploring: true, selected: "haunted-house" },
  );
  assert.equal(back.entry, "closed");
  assert.equal(back.selected, "haunted-house");
  // An unknown remembered world falls back to the first.
  assert.equal(
    initialVerse(
      { signedIn: false, worlds: WORLDS },
      { entered: true, exploring: true, selected: "nowhere" },
    ).selected,
    "openagents",
  );
});

test("world chips select a destination and show each world's category", async () => {
  const { trail, view, state, act } = setup(
    { haunt: "/new-lander/house" },
    { ...initialVerse({ signedIn: false, worlds: WORLDS }), entry: "closed" },
  );
  const chip = element(view(), "option-blue-rush");
  assert.deepEqual(chip, {
    kind: "chip",
    label: "Blue Rush",
    detail: "Creator studio",
    selected: false,
  });
  // OpenAgents is locked, with a link about us.
  assert.ok(node(view(), "openagents-locked"));
  assert.ok(node(view(), "openagents-about"));

  await act("option-haunted-house");
  assert.equal(state().selected, "haunted-house");
  assert.equal(
    (element(view(), "world-title") as { value: string }).value,
    "Haunted House",
  );
  assert.equal(
    (element(view(), "option-haunted-house") as { selected: boolean }).selected,
    true,
  );
  await act("haunted-house-enter");
  await act("option-blue-rush");
  await act("blue-rush-meet");
  await act("option-far-field");
  assert.ok(node(view(), "far-field-locked"));
  assert.deepEqual(trail, [
    "select haunted-house",
    "go /new-lander/house",
    "select blue-rush",
    "go /blue-rush",
    "select far-field",
  ]);
});

test("the engine's reports move the interface but never reopen the dialog", async () => {
  const { view, state, report } = setup();
  await report({ entered: true });
  assert.equal(state().entry, "choice");
  // The visitor clicked a planet in the engine.
  await report({ exploring: true, selected: "blue-rush" });
  assert.equal(state().entry, "closed");
  assert.equal(state().selected, "blue-rush");
  await report({ exploring: false, entered: false });
  assert.equal(state().entry, "closed");

  // The fallback links show until the 3D view opens.
  assert.ok(node(view(), "verse-fallback"));
  await report({ engine: "open" });
  assert.equal(node(view(), "verse-fallback"), undefined);
  await report({ engine: "error" });
  assert.ok(node(view(), "fallback-retry"));
});

test("the personal planet follows the engine's tutorial service", async () => {
  const { trail, view, state, act, report } = setup(
    {},
    {
      ...initialVerse({ signedIn: true, worlds: WORLDS }),
      entry: "closed",
      selected: "creator-worlds",
    },
  );
  assert.equal(
    (element(view(), "planet-status") as { value: string }).value,
    NO_PLANET.status,
  );
  assert.equal(node(view(), "planet-create"), undefined);

  await report({
    planet: {
      stage: "ready",
      heading: "",
      status: "Your first planet is ready to create.",
      canCreate: true,
      setupPath: undefined,
    },
  });
  assert.equal(
    (element(view(), "planet-create") as { enabled?: boolean }).enabled,
    true,
  );
  await act("planet-create");
  assert.deepEqual(trail, ["create planet"]);
  // Pressed once: it waits for the engine before it can be pressed again.
  assert.equal(
    (element(view(), "planet-create") as { enabled?: boolean }).enabled,
    false,
  );
  assert.equal(state().planet.status, "Creating your planet…");

  await report({
    planet: {
      stage: "owned",
      heading: "Congratulations",
      status: "You created your first planet. Choose a name and make it your own.",
      canCreate: false,
      setupPath: "/new-lander/planet/planet_1",
    },
    personal: {
      world: "creator-worlds",
      name: "Moon garden",
      category: "Your personal planet",
      description: "Your private place in the Verse.",
    },
  });
  assert.equal(
    (element(view(), "world-title") as { value: string }).value,
    "Moon garden",
  );
  assert.equal(
    (element(view(), "option-creator-worlds") as { label: string }).label,
    "Moon garden",
  );
  assert.equal(
    (element(view(), "planet-setup") as { href?: string }).href,
    "/new-lander/planet/planet_1",
  );
  await act("planet-setup");
  assert.deepEqual(trail.slice(1), ["go /new-lander/planet/planet_1"]);
});

test("reloading the map uses the engine, or reloads the page without it", async () => {
  const closed = {
    ...initialVerse({ signedIn: false, worlds: WORLDS }),
    entry: "closed" as const,
  };
  const withEngine = setup({ sdk: true }, closed);
  await withEngine.act("world-retry");
  await withEngine.act("world-fullscreen");
  assert.deepEqual(withEngine.trail, ["reload", "fullscreen"]);
  const without = setup({ sdk: false }, closed);
  await without.act("world-retry");
  assert.deepEqual(without.trail, ["reload", "go /new-lander/verse"]);
});

test("node IDs match the engine bridge's element IDs, and the tree is valid at every step", () => {
  const options = {
    accounts: true,
    hauntPath: "/new-lander/house",
    worlds: WORLDS,
    blueRushPath: "/blue-rush",
    mapPath: "/new-lander/verse",
  };
  for (const entry of ["account", "choice", "closed"] as const)
    for (const world of WORLDS) {
      const root = renderVerse(
        { ...initialVerse({ signedIn: false, worlds: WORLDS }), entry, selected: world.id },
        options,
      );
      const view: View = { schema: "aethyr.ui.v1", instance: "v", revision: 1, root };
      assert.equal(validateView(view).ok, true, `${entry} ${world.id}`);
      const all = ids(root);
      for (const id of ["verse-entry", "world-title", "world-category", "world-description"])
        assert.ok(all.includes(id), id);
      for (const w of WORLDS) assert.ok(all.includes(`option-${w.id}`));
    }
});
