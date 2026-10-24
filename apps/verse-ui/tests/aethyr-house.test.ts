// The house controllers (planet setup, invitations, the player bar) with
// recording services and no DOM.
import { test } from "node:test";
import assert from "node:assert/strict";
import { Effect, Layer, ManagedRuntime } from "effect";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import {
  InviteForm,
  Navigator,
  PlanetForm,
  PlayerEngine,
  initialInvite,
  initialSetup,
  makeInviteController,
  makePlayerController,
  makeSetupController,
  playerReport,
  type InviteState,
  type PlayerState,
  type SetupState,
} from "../src/aethyr/house";
import {
  intentFor,
  type Controller,
  type InvalidView,
} from "../src/aethyr/controller";
import type { Element, View } from "../src/aethyr/protocol";
import { findNode, validateView } from "../src/aethyr/validate";

function harness<S, Svc>(
  build: (trail: string[]) => {
    layer: Layer.Layer<Svc>;
    make: Effect.Effect<
      Controller<S, never, Svc | Navigator>,
      InvalidView,
      Svc | Navigator | AtomRegistry.AtomRegistry
    >;
  },
) {
  const trail: string[] = [];
  const registry = AtomRegistry.make();
  const { layer, make } = build(trail);
  const runtime = ManagedRuntime.make(
    Layer.mergeAll(
      Layer.succeed(AtomRegistry.AtomRegistry, registry),
      Layer.succeed(Navigator, {
        go: (path: string) => Effect.sync(() => void trail.push(`go ${path}`)),
      }),
      layer,
    ),
  );
  const controller = runtime.runSync(make);
  const view = () => registry.get(controller.view);
  const state = () => registry.get(controller.state);
  const send = (
    node: string,
    kind: "activate" | "input" | "submit",
    value?: string,
  ) =>
    runtime.runPromise(
      controller.dispatch(intentFor(view(), node, kind, value)),
    );
  const update = (f: (s: S) => S) => runtime.runPromise(controller.update(f));
  return { trail, view, state, send, update };
}

const element = <K extends Element["kind"]>(
  view: View,
  id: string,
  kind: K,
) => {
  const found = findNode(view, id)?.element;
  assert.equal(found?.kind, kind, `${id} is a ${kind}`);
  return found as Extract<Element, { kind: K }>;
};
const setup = (initial: SetupState) =>
  harness((trail) => ({
    layer: Layer.succeed(PlanetForm, {
      save: (name: string) =>
        Effect.sync(() => void trail.push(`save ${name}`)),
    }),
    make: makeSetupController(initial, "setup-test"),
  }));

test("setup edits a draft and saves it through the server form once", async () => {
  const { trail, view, state, send } = setup(
    initialSetup("Planet 7", "/new-lander/planet/p7/world"),
  );
  assert.ok(validateView(view()).ok);
  assert.equal(element(view(), "setup-title", "text").value, "Planet 7");
  assert.equal(element(view(), "planet-name", "field").value, "Planet 7");
  assert.equal(
    element(view(), "planet-enter", "button").href,
    "/new-lander/planet/p7/world",
  );
  await send("planet-name", "input", "  Moon garden ");
  assert.equal(state().draft, "  Moon garden ");
  // The title shows the saved name until the server says otherwise.
  assert.equal(element(view(), "setup-title", "text").value, "Planet 7");
  await send("planet-save", "activate");
  assert.deepEqual(trail, ["save Moon garden"]);
  assert.equal(state().saving, true);
  assert.equal(element(view(), "planet-save", "button").enabled, false);
  assert.equal(element(view(), "planet-name", "field").enabled, false);
});

test("setup refuses an empty or long name and saves on Enter", async () => {
  const { trail, view, send } = setup(initialSetup("Planet 7"));
  assert.equal(findNode(view(), "planet-enter"), undefined);
  await send("planet-name", "input", "   ");
  assert.equal(element(view(), "planet-save", "button").enabled, false);
  assert.match(element(view(), "setup-problem", "text").value, /Enter a name/);
  await send("planet-name", "input", "x".repeat(81));
  assert.match(element(view(), "setup-problem", "text").value, /80 characters/);
  await send("planet-name", "submit");
  assert.deepEqual(trail, []);
  await send("planet-name", "input", "🌙".repeat(80));
  assert.equal(findNode(view(), "setup-problem"), undefined);
  await send("planet-name", "submit");
  assert.deepEqual(trail, [`save ${"🌙".repeat(80)}`]);
});

test("setup follows its links", async () => {
  const { trail, send } = setup(
    initialSetup("Planet 7", "/new-lander/planet/p7/world"),
  );
  await send("setup-back", "activate");
  await send("planet-enter", "activate");
  assert.deepEqual(trail, [
    "go /new-lander/verse",
    "go /new-lander/planet/p7/world",
  ]);
});

const PAGE = {
  version: "3",
  intro: "Invite people to this saved version.",
  accessOptions: [
    { value: "signed-in", label: "People who sign in with the link" },
    { value: "anonymous", label: "Anyone with the link · sign in to chat" },
  ],
  hoursOptions: [
    { value: "1", label: "In 1 hour" },
    { value: "24", label: "In 1 day" },
    { value: "168", label: "In 7 days" },
  ],
  invitations: [
    {
      link: "/new-lander/visit/v1",
      state: "Visit and manage chat",
      access: "Sign in to visit",
      expiry: "Expires in 1 hours",
    },
  ],
};

const invite = (initial: InviteState) =>
  harness((trail) => ({
    layer: Layer.succeed(InviteForm, {
      create: (access: string, hours: string) =>
        Effect.sync(() => void trail.push(`create ${access} ${hours}`)),
    }),
    make: makeInviteController(initial, "invite-test"),
  }));

test("invitations choose access and expiry as chips, then create once", async () => {
  const { trail, view, state, send } = invite(initialInvite(PAGE));
  assert.ok(validateView(view()).ok);
  assert.equal(element(view(), "access-signed-in", "chip").selected, true);
  assert.equal(element(view(), "hours-1", "chip").selected, true);
  await send("access-anonymous", "activate");
  await send("hours-168", "activate");
  assert.equal(element(view(), "access-signed-in", "chip").selected, false);
  assert.equal(element(view(), "access-anonymous", "chip").selected, true);
  assert.equal(element(view(), "hours-168", "chip").selected, true);
  await send("invite-create", "activate");
  assert.deepEqual(trail, ["create anonymous 168"]);
  assert.equal(state().creating, true);
  assert.equal(element(view(), "invite-create", "button").enabled, false);
  assert.equal(element(view(), "access-signed-in", "chip").enabled, false);
});

test("invitations list each link and say when there are none", async () => {
  const { trail, view, send } = invite(initialInvite(PAGE));
  assert.equal(
    element(view(), "invitation-0-open", "button").href,
    "/new-lander/visit/v1",
  );
  assert.equal(
    element(view(), "invitation-0-detail", "text").value,
    "Visit and manage chat · Sign in to visit · Expires in 1 hours",
  );
  await send("invitation-0-open", "activate");
  assert.deepEqual(trail, ["go /new-lander/visit/v1"]);
  const empty = invite(initialInvite({ ...PAGE, invitations: [] }));
  assert.ok(findNode(empty.view(), "invitations-empty"));
  assert.equal(findNode(empty.view(), "invitations"), undefined);
});

const player = (initial: PlayerState) =>
  harness((trail) => ({
    layer: Layer.succeed(PlayerEngine, {
      retry: Effect.sync(() => void trail.push("retry")),
      fullscreen: Effect.sync(() => void trail.push("fullscreen")),
    }),
    make: makePlayerController(initial, "player-test"),
  }));

test("the player bar mirrors the release's controls and drives them", async () => {
  const { trail, view, send, update } = player({
    title: "Haunted House",
    version: "2",
    retry: true,
    fullscreen: true,
  });
  assert.ok(validateView(view()).ok);
  assert.equal(element(view(), "player-title", "text").value, "Haunted House");
  assert.equal(findNode(view(), "player-version")?.style?.hide_on_phone, true);
  await send("world-retry", "activate");
  await send("world-fullscreen", "activate");
  await send("player-back", "activate");
  assert.deepEqual(trail, ["retry", "fullscreen", "go /new-lander/verse"]);
  // A visit that ends hides the release's controls; the bar follows.
  await update((s) => playerReport(s, { retry: false, fullscreen: false }));
  assert.equal(findNode(view(), "world-retry"), undefined);
  assert.equal(findNode(view(), "world-fullscreen"), undefined);
});
