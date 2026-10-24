// The house editor controller with a recording bridge and no DOM: each
// surface draws from the release's mirrors, each intent becomes one bridge
// operation, and the layout names only IDs the server's page has.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Effect, Layer, ManagedRuntime } from "effect";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import {
  ABSENT,
  EditorBridge,
  Navigator,
  bridgeIds,
  editorReport,
  makeEditorController,
  type BridgeOp,
  type EditorState,
  type Mirror,
} from "../src/aethyr/editor";
import { EDITOR_SURFACES } from "../src/aethyr/editor-layout";
import { intentFor } from "../src/aethyr/controller";
import type { Element, View } from "../src/aethyr/protocol";
import { findNode, validateView } from "../src/aethyr/validate";

const here = dirname(fileURLToPath(import.meta.url));
const shown = (more: Partial<Mirror> = {}): Mirror => ({
  ...ABSENT,
  shown: true,
  enabled: true,
  ...more,
});

/** Every control the layout reads, shown and enabled, with a few values. */
function everything(more: Record<string, Partial<Mirror>> = {}): EditorState {
  const mirror: Record<string, Mirror> = {};
  for (const surface of EDITOR_SURFACES) {
    mirror[surface.id] = shown();
    for (const id of bridgeIds(surface.children))
      mirror[id] = shown({ text: id.replace(/-/g, " ") });
  }
  for (const [id, m] of Object.entries(more)) mirror[id] = shown(m);
  return { mirror, disclosed: {}, tool: "objects", compact: false };
}

function editor(initial: EditorState) {
  const trail: Array<BridgeOp | string> = [];
  const registry = AtomRegistry.make();
  const runtime = ManagedRuntime.make(
    Layer.mergeAll(
      Layer.succeed(AtomRegistry.AtomRegistry, registry),
      Layer.succeed(EditorBridge, {
        act: (op: BridgeOp) => Effect.sync(() => void trail.push(op)),
      }),
      Layer.succeed(Navigator, {
        go: (path: string) => Effect.sync(() => void trail.push(`go ${path}`)),
      }),
    ),
  );
  const controller = runtime.runSync(
    makeEditorController(EDITOR_SURFACES, initial, "editor-test"),
  );
  const view = () => registry.get(controller.view);
  const state = () => registry.get(controller.state);
  const send = (
    node: string,
    kind: "activate" | "input" | "select" | "dismiss",
    value?: string,
  ) =>
    runtime.runPromise(
      controller.dispatch(intentFor(view(), node, kind, value)),
    );
  const report = (s: EditorState) =>
    runtime.runPromise(controller.update((old) => editorReport(old, s)));
  return { trail, view, state, send, report };
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

test("the layout names only IDs the studio page has", () => {
  const page = readFileSync(
    join(here, "../../../crates/openagents-web/src/pages/new_lander_house.rs"),
    "utf8",
  );
  // Tabs and close buttons are built from lists in the page source.
  const built = (id: string) =>
    /^tab-/.test(id) || /^close-/.test(id) || /^request-/.test(id);
  const missing = EDITOR_SURFACES.flatMap((surface) => [
    surface.id,
    ...bridgeIds(surface.children),
  ]).filter(
    (id) =>
      !page.includes(`"${id}"`) &&
      !built(id) &&
      // Synthetic groups that only lay out the release's controls.
      !/(-actions|-files|-position|-header|-windows)$/.test(id),
  );
  assert.deepEqual(missing, []);
});

test("every surface draws a valid view with all its controls", () => {
  const { view } = editor(everything());
  const checked = validateView(view());
  assert.ok(checked.ok, JSON.stringify(checked));
  for (const id of [
    "edit-panel",
    "edit-auth",
    "publish-review",
    "publish-share-card",
    "planet-settings",
  ])
    assert.ok(findNode(view(), id), id);
  // Dialogs and Jarvis close through the release's own controls.
  assert.equal(element(view(), "edit-auth", "dialog").dismissible, true);
  assert.equal(element(view(), "edit-panel", "sheet").dismissible, true);
});

test("closed surfaces draw nothing", () => {
  const state = everything();
  const mirror = { ...state.mirror };
  for (const id of [
    "edit-panel",
    "edit-auth",
    "publish-review",
    "publish-share-card",
  ])
    mirror[id] = { ...mirror[id]!, shown: false };
  const { view } = editor({ ...state, mirror });
  assert.equal(findNode(view(), "edit-panel"), undefined);
  assert.equal(findNode(view(), "edit-auth"), undefined);
  assert.ok(findNode(view(), "planet-settings"));
});

test("controls carry the release's values and send one bridge operation each", async () => {
  const { trail, view, send } = editor(
    everything({
      "face-area": {
        value: "forehead",
        options: [
          { value: "left_cheek", label: "Left cheek" },
          { value: "forehead", label: "Forehead" },
        ],
      },
      "face-brush-size": { value: "5", min: 1, max: 32, step: 1 },
      "face-brush-color": { value: "#8096c0" },
      "edit-kiln-world": { checked: false },
      "edit-x": { value: "1.5" },
      "face-import": { accept: ["application/json", ".json"] },
    }),
  );
  assert.equal(element(view(), "face-area", "select").value, "forehead");
  assert.equal(element(view(), "face-brush-size", "range").value, 5);
  assert.equal(element(view(), "face-brush-color", "color").value, "#8096c0");
  assert.equal(element(view(), "edit-x", "field").value, "1.5");
  assert.deepEqual(element(view(), "face-import", "file").accept, [
    "application/json",
    ".json",
  ]);
  assert.equal(
    element(view(), "face-canvas-slot", "canvas").surface,
    "face-canvas",
  );
  await send("face-area", "select", "left_cheek");
  await send("face-brush-size", "input", "12");
  await send("face-brush-color", "input", "#ff8800");
  await send("edit-kiln-world", "input", "true");
  await send("edit-x", "input", "2");
  await send("face-import", "input", "face.json");
  await send("edit-pick", "activate");
  await send("close-objects", "activate");
  assert.deepEqual(trail, [
    { op: "set", id: "face-area", value: "left_cheek" },
    { op: "set", id: "face-brush-size", value: "12" },
    { op: "set", id: "face-brush-color", value: "#ff8800" },
    { op: "check", id: "edit-kiln-world", checked: true },
    { op: "set", id: "edit-x", value: "2" },
    { op: "files", id: "face-import" },
    { op: "click", id: "edit-pick" },
    { op: "click", id: "close-objects" },
  ]);
});

test("a checkbox or choice shows at once, and rolls back if the release refuses it", async () => {
  const state = everything({
    "publish-confirm-rights": { checked: false },
    "face-area": {
      value: "forehead",
      options: [
        { value: "left_cheek", label: "Left cheek" },
        { value: "forehead", label: "Forehead" },
      ],
    },
  });
  const { trail, view, send, report } = editor(state);
  await send("publish-confirm-rights", "input", "true");
  await send("face-area", "select", "left_cheek");
  // Before the release's next report, the view already shows both choices.
  assert.equal(
    element(view(), "publish-confirm-rights", "checkbox").checked,
    true,
  );
  assert.equal(element(view(), "face-area", "select").value, "left_cheek");
  assert.deepEqual(trail, [
    { op: "check", id: "publish-confirm-rights", checked: true },
    { op: "set", id: "face-area", value: "left_cheek" },
  ]);
  // The release kept its own values: its report puts them back.
  await report(state);
  assert.equal(
    element(view(), "publish-confirm-rights", "checkbox").checked,
    false,
  );
  assert.equal(element(view(), "face-area", "select").value, "forehead");
});

test("disabled controls stay disabled; the renderer cannot act on them", async () => {
  const { view, send } = editor(
    everything({
      "face-generate": { text: "Generate local scars", enabled: false },
    }),
  );
  assert.equal(element(view(), "face-generate", "button").enabled, false);
  await assert.rejects(send("face-generate", "activate"));
});

test("tabs mirror the release and choose its tools; phones show one window", async () => {
  const state = everything({ "tab-request": { on: true } });
  const { trail, view, send, report } = editor(state);
  assert.equal(element(view(), "tab-request", "chip").selected, true);
  assert.equal(element(view(), "tab-objects", "chip").selected, false);
  for (const id of ["panel-objects", "panel-materials", "panel-request"])
    assert.ok(findNode(view(), id), id);
  await send("tab-history", "activate");
  assert.deepEqual(trail, [{ op: "click", id: "tab-history" }]);
  await report({ ...state, compact: true, tool: "history" });
  assert.equal(findNode(view(), "panel-objects"), undefined);
  assert.ok(findNode(view(), "panel-history"));
  // Phones close windows with the tabs, not the × buttons.
  assert.equal(findNode(view(), "close-history"), undefined);
});

test("disclosures open and close in the controller", async () => {
  const { view, state, send } = editor(
    everything({
      "planet-style": {
        value: "soft-pastels",
        options: [{ value: "soft-pastels", label: "Soft pastels" }],
      },
    }),
  );
  assert.equal(findNode(view(), "edit-material"), undefined);
  await send("material-fog-toggle", "activate");
  assert.equal(state().disclosed["material-fog"], true);
  assert.ok(findNode(view(), "edit-material"));
  // The courtyard card starts open.
  assert.ok(findNode(view(), "planet-style"));
  await send("planet-card-toggle", "activate");
  assert.equal(findNode(view(), "planet-style"), undefined);
});

test("release-built rows keep their actions", async () => {
  const { trail, view, send } = editor(
    everything({
      "project-history-rows": {
        rows: [
          {
            texts: ["Kiki", "Moved the lamp · Version 3 · 2026-10-24 UTC"],
            actions: ["Profile", "Inspect saved version"],
          },
        ],
      },
      "edit-history-entries": { items: ["Moved the lamp", "Changed fog"] },
    }),
  );
  assert.equal(
    element(view(), "project-history-rows-0-action-1", "button").label,
    "Inspect saved version",
  );
  assert.equal(
    element(view(), "edit-history-entries", "list").children.length,
    2,
  );
  await send("project-history-rows-0-action-1", "activate");
  assert.deepEqual(trail, [
    { op: "row", id: "project-history-rows", row: 0, index: 1 },
  ]);
});

test("Escape on a dialog is the release's Cancel; links stay on the site", async () => {
  const { trail, send } = editor(
    everything({
      "published-link": {
        text: "Open playable version",
        href: "/new-lander/play/w/r",
      },
      "publish-share-link": {
        text: "https://openagents.com/new-lander/play/w/r",
      },
    }),
  );
  await send("edit-auth", "dismiss");
  await send("publish-review", "dismiss");
  await send("edit-panel", "dismiss");
  await send("published-link", "activate");
  await send("publish-share-link", "activate");
  assert.deepEqual(trail, [
    { op: "click", id: "edit-cancel" },
    { op: "click", id: "publish-cancel" },
    { op: "click", id: "edit-close" },
    "go /new-lander/play/w/r",
    { op: "click", id: "publish-share-link" },
  ]);
});

test("Sign in is relayed to the release and never navigates", async () => {
  const { trail, view, send } = editor(
    everything({
      "edit-signin": {
        text: "Sign in",
        href: "/login?return_to=%2Fnew-lander%2Fhouse",
      },
    }),
  );
  // No href: the page cannot follow the link itself.
  assert.equal(element(view(), "edit-signin", "button").href, undefined);
  await send("edit-signin", "activate");
  assert.deepEqual(trail, [{ op: "click", id: "edit-signin" }]);
});

test("the editor never offers chat", () => {
  const { view } = editor(everything());
  assert.doesNotMatch(JSON.stringify(view().root), /\bchat\b/i);
});
