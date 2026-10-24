// The studio HUD controller with a recording release and no DOM: the dock,
// the notice, the voice bar, and the world actions.
import { test } from "node:test";
import assert from "node:assert/strict";
import { Effect, Layer, ManagedRuntime } from "effect";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import {
  EMPTY_REPORT,
  MORE_ACTIONS,
  NO_GPU,
  StudioEngine,
  initialStudio,
  makeStudioController,
  studioReport,
  type StudioReport,
} from "../src/aethyr/studio";
import { Navigator } from "../src/aethyr/entry";
import { intentFor } from "../src/aethyr/controller";
import type { Element, View } from "../src/aethyr/protocol";
import { findNode, validateView } from "../src/aethyr/validate";

const OPEN: StudioReport = {
  ...EMPTY_REPORT,
  phase: "open",
  notice: "",
  mesh: { shown: true, enabled: true, pressed: false },
  save: { shown: true, enabled: true },
};
const PAGE = {
  gpu: true,
  touch: false,
  mapPath: "/new-lander/verse?world=haunted-house",
  reloadPath: "/new-lander/house",
};

function studio(report: StudioReport = OPEN, page = PAGE) {
  const trail: string[] = [];
  const registry = AtomRegistry.make();
  const runtime = ManagedRuntime.make(
    Layer.mergeAll(
      Layer.succeed(AtomRegistry.AtomRegistry, registry),
      Layer.succeed(Navigator, {
        go: (path: string) => Effect.sync(() => void trail.push(`go ${path}`)),
      }),
      Layer.succeed(StudioEngine, {
        click: (id: string) =>
          Effect.sync(() => void trail.push(`click ${id}`)),
      }),
    ),
  );
  const controller = runtime.runSync(
    makeStudioController(initialStudio(page, report), "studio-test"),
  );
  const view = () => registry.get(controller.view);
  const state = () => registry.get(controller.state);
  const act = (node: string) =>
    runtime.runPromise(
      controller.dispatch(intentFor(view(), node, "activate")),
    );
  const report_ = (r: StudioReport) =>
    runtime.runPromise(controller.update((s) => studioReport(s, r)));
  return { trail, view, state, act, report: report_ };
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

test("the dock drives the release's own controls by ID", async () => {
  const { trail, view, act } = studio();
  assert.ok(validateView(view()).ok);
  assert.equal(findNode(view(), "world-notice"), undefined);
  assert.equal(element(view(), "world-map", "button").href, PAGE.mapPath);
  assert.equal(findNode(view(), "world-map")?.a11y?.label, "Back to planets");
  assert.equal(
    findNode(view(), "world-fullscreen")?.style?.hide_on_phone,
    true,
  );
  for (const id of [
    "world-map",
    "world-retry",
    "world-fullscreen",
    "world-mesh",
    "world-edit",
    "project-save",
  ])
    await act(id);
  assert.deepEqual(trail, [
    "click world-map",
    "click world-retry",
    "click world-fullscreen",
    "click world-mesh",
    "click world-edit",
    "click project-save",
  ]);
});

test("the dock mirrors what the release shows and waits behind the editor", async () => {
  const { view, report } = studio({
    ...OPEN,
    save: { shown: false, enabled: false },
  });
  assert.equal(findNode(view(), "project-save"), undefined);
  assert.equal(element(view(), "world-mesh", "chip").selected, false);
  await report({
    ...OPEN,
    mesh: { shown: true, enabled: true, pressed: true },
  });
  assert.equal(element(view(), "world-mesh", "chip").selected, true);
  await report({ ...OPEN, editing: true, tool: "objects" });
  assert.equal(element(view(), "world-edit", "button").enabled, false);
  assert.equal(element(view(), "world-map", "button").enabled, false);
});

test("an opening world says so; an error offers reload and the planets", async () => {
  const { trail, view, act, report } = studio({
    ...EMPTY_REPORT,
    notice: "Opening the world… The first visit can take a little longer.",
  });
  assert.equal(findNode(view(), "world-notice")?.a11y?.role, "status");
  assert.equal(findNode(view(), "world-notice-retry"), undefined);
  await report({
    ...OPEN,
    phase: "error",
    notice: "The world could not open.",
  });
  assert.equal(findNode(view(), "world-notice")?.a11y?.role, "alert");
  assert.equal(
    element(view(), "world-notice-text", "text").value,
    "The world could not open.",
  );
  await act("world-notice-retry");
  await act("world-notice-map");
  assert.deepEqual(trail, ["click world-retry", `go ${PAGE.mapPath}`]);
});

test("without WebGPU the error names the reason", () => {
  const { view } = studio(
    { ...OPEN, phase: "error", notice: "The world could not open." },
    { ...PAGE, gpu: false },
  );
  assert.equal(element(view(), "world-notice-text", "text").value, NO_GPU);
});

test("the voice bar follows the release and replaces the help line", async () => {
  const { trail, view, act, report } = studio();
  assert.match(element(view(), "world-help", "label").value, /WASD/);
  assert.equal(findNode(view(), "world-voice"), undefined);
  await report({
    ...OPEN,
    voice: {
      shown: true,
      listen: { shown: true, enabled: true, label: "Listening" },
      stop: { shown: true, enabled: true },
      status: "Listening · say Stop to mute",
      heard: "show options",
      response: "",
    },
  });
  assert.equal(findNode(view(), "world-help"), undefined);
  assert.equal(
    element(view(), "edit-voice-start", "button").label,
    "Listening",
  );
  assert.equal(
    element(view(), "edit-voice-status", "text").value,
    "Listening · say Stop to mute",
  );
  assert.equal(element(view(), "voice-heard", "text").value, "show options");
  assert.equal(findNode(view(), "voice-response"), undefined);
  await act("edit-voice-stop");
  assert.deepEqual(trail, ["click edit-voice-stop"]);
});

test("touch help shows on phones; pointer help hides there", () => {
  const touch = studio(OPEN, { ...PAGE, touch: true }).view();
  assert.match(element(touch, "world-help", "label").value, /Left side/);
  assert.equal(findNode(touch, "world-help")?.style?.hide_on_phone, false);
  const pointer = studio().view();
  assert.equal(findNode(pointer, "world-help")?.style?.hide_on_phone, true);
});

test("world actions show when the release offers them, with More on request", async () => {
  const offered: StudioReport = {
    ...OPEN,
    actions: true,
    publish: { shown: true, enabled: true },
    play: { shown: true, enabled: false },
    share: { shown: false, enabled: false },
    revert: { shown: true, enabled: true },
  };
  const { trail, view, state, act, report } = studio(offered);
  assert.equal(
    element(view(), "project-publish", "button").emphasis,
    "primary",
  );
  assert.equal(element(view(), "project-play", "button").enabled, false);
  assert.equal(findNode(view(), "project-share"), undefined);
  assert.equal(findNode(view(), "project-revert"), undefined);
  await act("request-more");
  assert.equal(state().more, true);
  assert.equal(element(view(), "request-more", "chip").selected, true);
  for (const [id] of MORE_ACTIONS) assert.ok(findNode(view(), id), id);
  await act("project-revert");
  await act("request-undo");
  await act("project-publish");
  assert.deepEqual(trail, [
    "click project-revert",
    "click request-undo",
    "click project-publish",
  ]);
  // Another editor tool covers them; Request keeps them.
  await report({ ...offered, editing: true, tool: "objects" });
  assert.equal(findNode(view(), "request-choices"), undefined);
  await report({ ...offered, editing: true, tool: "request" });
  assert.ok(findNode(view(), "request-choices"));
  // When the release withdraws them, More closes.
  await report({ ...offered, actions: false });
  assert.equal(state().more, false);
});

test("the studio HUD never offers chat", () => {
  const { view } = studio({
    ...OPEN,
    actions: true,
    voice: { ...EMPTY_REPORT.voice, shown: true },
  });
  assert.doesNotMatch(JSON.stringify(view().root), /chat/i);
});
