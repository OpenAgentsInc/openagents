// The Haunted House studio's HUD as an Aethyr controller (#12588): the world
// notice, the dock (planets, reload, full screen, mesh, edit, save), the
// voice bar, and the world actions. The pinned release still owns the world,
// the editor (Jarvis), and its review dialogs; it drives its own copy of these
// controls (`.studio-bridge`), and the web host (`house-host.tsx`) reports
// what it shows there through `studioReport` and clicks it for each intent.
import { Context, Effect } from "effect";
import * as Controller from "./controller";
import { Navigator } from "./entry";
import type { Node } from "./protocol";

/** The release's own studio controls, by the IDs it listens on. */
export class StudioEngine extends Context.Service<
  StudioEngine,
  { readonly click: (id: string) => Effect.Effect<void> }
>()("aethyr/StudioEngine") {}

/** One release control as the release shows it. */
export interface Control {
  readonly shown: boolean;
  readonly enabled: boolean;
}

export type WorldPhase = "opening" | "open" | "error";

export interface StudioReport {
  readonly phase: WorldPhase;
  readonly notice: string;
  /** The editor is open; the dock waits behind it. */
  readonly editing: boolean;
  /** The editor's active tool, when it is open. */
  readonly tool: string;
  readonly mesh: Control & { readonly pressed: boolean };
  readonly save: Control;
  readonly voice: {
    readonly shown: boolean;
    readonly listen: Control & { readonly label: string };
    readonly stop: Control;
    readonly status: string;
    readonly heard: string;
    readonly response: string;
  };
  /** The release offers world actions now. */
  readonly actions: boolean;
  readonly publish: Control;
  readonly play: Control;
  readonly share: Control;
  readonly revert: Control;
}

export interface StudioState extends StudioReport {
  /** This browser can draw 3D worlds (WebGPU). */
  readonly gpu: boolean;
  /** Touch is the main input: the help says so. */
  readonly touch: boolean;
  readonly mapPath: string;
  readonly reloadPath: string;
  /** The world actions show their longer list. */
  readonly more: boolean;
}

const hidden: Control = { shown: false, enabled: false };

export const initialStudio = (
  page: Pick<StudioState, "gpu" | "touch" | "mapPath" | "reloadPath">,
  report: StudioReport,
): StudioState => ({ ...page, ...report, more: false });

/** Fold what the release shows into the state; the visitor's own choices stay. */
export const studioReport = (
  state: StudioState,
  report: StudioReport,
): StudioState => ({
  ...state,
  ...report,
  more: report.actions ? state.more : false,
});

export const EMPTY_REPORT: StudioReport = {
  phase: "opening",
  notice: "Opening the house…",
  editing: false,
  tool: "",
  mesh: { ...hidden, pressed: false },
  save: hidden,
  voice: {
    shown: false,
    listen: { ...hidden, label: "Listen" },
    stop: hidden,
    status: "",
    heard: "",
    response: "",
  },
  actions: false,
  publish: hidden,
  play: hidden,
  share: hidden,
  revert: hidden,
};

export const NO_GPU =
  "This browser can't show 3D worlds. Try a recent version of Chrome, Edge, or Safari.";

/** World actions under More; the release listens on each by ID. */
export const MORE_ACTIONS: ReadonlyArray<readonly [string, string]> = [
  ["request-inspect", "Inspect world"],
  ["request-mesh", "Show mesh"],
  ["request-map", "Back to planets"],
  ["request-compile", "Compile world effect"],
  ["request-undo", "Undo"],
];

const button = (
  id: string,
  label: string,
  options: {
    readonly enabled?: boolean;
    readonly emphasis?: "primary" | "secondary" | "ghost" | "link";
    readonly href?: string;
    readonly name?: string;
    readonly phone?: boolean;
  } = {},
): Node => ({
  id,
  ...(options.phone === false && { style: { hide_on_phone: true } }),
  ...(options.name && { a11y: { label: options.name } }),
  element: {
    kind: "button",
    label,
    emphasis: options.emphasis ?? "ghost",
    ...(options.enabled === false && { enabled: false }),
    ...(options.href && { href: options.href }),
  },
});
const row = (id: string, children: Node[], gap: "xs" | "sm" = "xs"): Node => ({
  id,
  style: { gap },
  element: { kind: "stack", axis: "horizontal", children },
});
const text = (
  id: string,
  value: string,
  role: "body" | "caption" | "status" = "body",
): Node => ({ id, element: { kind: "text", value, role } });

function notice(state: StudioState): Node | undefined {
  if (state.phase === "open") return undefined;
  const error = state.phase === "error";
  const children: Node[] = [
    {
      id: "world-notice-text",
      a11y: { live: error ? "assertive" : "polite" },
      element: {
        kind: "text",
        value: error && !state.gpu ? NO_GPU : state.notice,
        role: "status",
      },
    },
  ];
  if (error)
    children.push(
      row(
        "world-notice-actions",
        [
          button("world-notice-retry", "Reload world", {
            emphasis: "primary",
            href: state.reloadPath,
          }),
          button("world-notice-map", "Back to planets", {
            emphasis: "secondary",
            href: state.mapPath,
          }),
        ],
        "sm",
      ),
    );
  return {
    id: "world-notice",
    style: { gap: "md", ...(error && { tone: "danger" }) },
    a11y: { role: error ? "alert" : "status" },
    element: { kind: "panel", children },
  };
}

function dock(state: StudioState): Node {
  const inert = state.editing;
  // After an engine failure, Reload is the way forward, on every width.
  const failed = state.phase === "error";
  const navigation = row("world-navigation", [
    button("world-map", "Planets", {
      href: state.mapPath,
      name: "Back to planets",
      enabled: !inert,
    }),
    button("world-retry", failed ? "Reload world" : "Reload", {
      href: state.reloadPath,
      name: "Reload world",
      enabled: !inert,
      emphasis: failed ? "primary" : "ghost",
      phone: failed,
    }),
    button("world-fullscreen", "Full screen", {
      enabled: !inert,
      phone: false,
    }),
  ]);
  const authoring: Node[] = [];
  if (state.mesh.shown)
    authoring.push({
      id: "world-mesh",
      a11y: { label: "Mesh overlay" },
      element: {
        kind: "chip",
        label: "Mesh",
        selected: state.mesh.pressed,
        enabled: !inert && state.mesh.enabled,
      },
    });
  authoring.push(
    button("world-edit", "Edit", {
      emphasis: failed ? "ghost" : "primary",
      enabled: !inert,
    }),
  );
  if (state.save.shown)
    authoring.push(
      button("project-save", "Save", {
        name: "Save project",
        enabled: !inert && state.save.enabled,
      }),
    );
  return {
    id: "world-controls",
    a11y: { role: "navigation", label: "World controls" },
    element: {
      kind: "panel",
      children: [
        {
          id: "world-controls-row",
          style: { gap: "sm", align: "between" },
          element: {
            kind: "stack",
            axis: "horizontal",
            children: [navigation, row("world-authoring", authoring)],
          },
        },
      ],
    },
  };
}

function voice(state: StudioState): Node | undefined {
  const v = state.voice;
  if (!v.shown) return undefined;
  const controls: Node[] = [];
  if (v.listen.shown)
    controls.push(
      button("edit-voice-start", v.listen.label, { enabled: v.listen.enabled }),
    );
  if (v.stop.shown)
    controls.push(
      button("edit-voice-stop", "Stop / mute", { enabled: v.stop.enabled }),
    );
  const children: Node[] = [
    {
      id: "world-voice-row",
      style: { gap: "sm", align: "between" },
      element: {
        kind: "stack",
        axis: "wrap",
        children: [
          row("world-voice-controls", controls),
          {
            id: "edit-voice-status",
            a11y: { live: "polite" },
            element: { kind: "text", value: v.status, role: "status" },
          },
        ],
      },
    },
  ];
  // The release's answer first; what it heard is secondary when space is short.
  if (v.response)
    children.push({
      id: "voice-response",
      a11y: { live: "polite" },
      element: { kind: "text", value: v.response, role: "status" },
    });
  if (v.heard) children.push(text("voice-heard", v.heard, "caption"));
  return {
    id: "world-voice",
    style: { gap: "xs" },
    a11y: { role: "region", label: "Voice controls" },
    element: { kind: "panel", children },
  };
}

function actions(state: StudioState): Node | undefined {
  // The editor's other tools cover the actions; Request keeps them.
  if (!state.actions || (state.editing && state.tool !== "request"))
    return undefined;
  const main: Node[] = [];
  const offer = (id: string, label: string, c: Control, primary = false) => {
    if (c.shown)
      main.push(
        button(id, label, {
          enabled: c.enabled,
          emphasis: primary ? "primary" : "secondary",
        }),
      );
  };
  offer("project-publish", "Publish", state.publish, true);
  offer("project-play", "Play", state.play);
  offer("project-share", "Share", state.share);
  const children: Node[] = [
    {
      id: "request-choices-label",
      element: { kind: "label", value: "World actions" },
    },
    {
      id: "request-choices-main",
      style: { gap: "xs" },
      element: { kind: "stack", axis: "wrap", children: main },
    },
    {
      id: "request-more",
      element: {
        kind: "chip",
        label: "More world actions",
        selected: state.more,
      },
    },
  ];
  if (state.more) {
    const more: Node[] = [];
    if (state.revert.shown)
      more.push(
        button("project-revert", "Revert to Starting house", {
          enabled: state.revert.enabled,
        }),
      );
    for (const [id, label] of MORE_ACTIONS) more.push(button(id, label));
    children.push({
      id: "request-more-actions",
      style: { gap: "xs" },
      element: { kind: "stack", axis: "wrap", children: more },
    });
  }
  return {
    id: "request-choices",
    style: { gap: "sm" },
    a11y: { role: "region", label: "World actions" },
    element: { kind: "panel", children },
  };
}

export function renderStudio(state: StudioState): Node {
  const help: Node = {
    id: "world-help",
    style: { hide_on_phone: !state.touch },
    element: {
      kind: "label",
      value: state.touch
        ? "Left side: move · Right side: look"
        : "Click to look · WASD to walk · Escape to release the pointer",
    },
  };
  const dockChildren: Node[] = [];
  const a = actions(state);
  if (a) dockChildren.push(a);
  dockChildren.push(voice(state) ?? help, dock(state));
  const n = notice(state);
  return {
    id: "studio-hud",
    element: {
      kind: "stack",
      axis: "vertical",
      children: [
        ...(n ? [n] : []),
        {
          id: "world-dock",
          style: { gap: "sm" },
          element: { kind: "stack", axis: "vertical", children: dockChildren },
        },
      ],
    },
  };
}

/** Controls the release listens on, by the same ID. */
const RELEASE = new Set([
  "world-map",
  "world-retry",
  "world-fullscreen",
  "world-mesh",
  "world-edit",
  "project-save",
  "edit-voice-start",
  "edit-voice-stop",
  "project-publish",
  "project-play",
  "project-share",
  "project-revert",
  ...MORE_ACTIONS.map(([id]) => id),
]);

export const makeStudioController = (
  initial: StudioState,
  instance = `studio-${Date.now().toString(36)}`,
) =>
  Effect.gen(function* () {
    const navigator = yield* Navigator;
    const engine = yield* StudioEngine;
    return yield* Controller.make<StudioState>({
      instance,
      initial,
      render: renderStudio,
      handle: (state, intent) =>
        Effect.gen(function* () {
          const id = intent.node;
          if (id === "request-more") return { ...state, more: !state.more };
          if (id === "world-notice-retry") {
            yield* engine.click("world-retry");
            return state;
          }
          if (id === "world-notice-map") {
            yield* navigator.go(state.mapPath);
            return state;
          }
          if (RELEASE.has(id)) yield* engine.click(id);
          return state;
        }),
    });
  });
