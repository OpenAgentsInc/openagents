// The house editor as an Aethyr controller (#12595): Jarvis (its tool
// windows), the sign-in and confirmation dialog, publish review, the share
// card, and courtyard settings. The pinned release still owns what each
// control does: it drives its own copy of these controls by ID (the bridge),
// the web host reads that copy into `Mirror`s, and this controller draws
// them from a declarative layout (`editor-layout.ts`). Every intent becomes
// one bridge operation (`EditorBridge`), so the release sees the same
// clicks, values, and files it always did.
import { Context, Effect } from "effect";
import * as Controller from "./controller";
import { Navigator } from "./entry";
import {
  MAX_FIELD_BYTES,
  type Choice,
  type Node,
  type TextRole,
} from "./protocol";

export { Navigator };

/** One action on the release's own control, by its ID. */
export type BridgeOp =
  | { readonly op: "click"; readonly id: string }
  /** Set a text, select, range, or color value and announce it. */
  | { readonly op: "set"; readonly id: string; readonly value: string }
  /** Make a checkbox match (by clicking it when it differs). */
  | { readonly op: "check"; readonly id: string; readonly checked: boolean }
  /** Give an upload input the files the host received for it. */
  | { readonly op: "files"; readonly id: string }
  /** Click the `index`th action (button or link) of row `row` in a list. */
  | {
      readonly op: "row";
      readonly id: string;
      readonly row: number;
      readonly index: number;
    };

export class EditorBridge extends Context.Service<
  EditorBridge,
  { readonly act: (op: BridgeOp) => Effect.Effect<void> }
>()("aethyr/EditorBridge") {}

/** A release-built list row: its text lines and its actions. */
export interface Row {
  readonly texts: ReadonlyArray<string>;
  readonly actions: ReadonlyArray<string>;
}

/** What the release shows on one of its controls now. */
export interface Mirror {
  readonly shown: boolean;
  readonly enabled: boolean;
  readonly text: string;
  readonly value: string;
  readonly checked: boolean;
  readonly options: ReadonlyArray<Choice>;
  /** A same-site link target, when the release set one. */
  readonly href: string | undefined;
  /** Pressed, selected, or expanded (tabs and toggles). */
  readonly on: boolean;
  readonly items: ReadonlyArray<string>;
  readonly rows: ReadonlyArray<Row>;
  readonly accept: ReadonlyArray<string>;
  readonly multiple: boolean;
  readonly min: number;
  readonly max: number;
  readonly step: number;
}

export const ABSENT: Mirror = {
  shown: false,
  enabled: false,
  text: "",
  value: "",
  checked: false,
  options: [],
  href: undefined,
  on: false,
  items: [],
  rows: [],
  accept: [],
  multiple: false,
  min: 0,
  max: 1,
  step: 1,
};

type Gap = "xs" | "sm" | "md";

/** The editor's layout: what to draw for each release control. */
export type Spec =
  /** Children, shown while `id` (when given) is shown in the release. */
  | {
      readonly t: "group";
      readonly id: string;
      readonly children: ReadonlyArray<Spec>;
      readonly axis?: "vertical" | "horizontal" | "wrap";
      readonly gap?: Gap;
      readonly bridged?: boolean;
    }
  /** A tool window: the release's `panel-*` section with its close button. */
  | {
      readonly t: "window";
      readonly id: string;
      readonly tool: string;
      readonly title: string;
      readonly close: string;
      readonly children: ReadonlyArray<Spec>;
    }
  /** Fixed copy (not the release's). */
  | {
      readonly t: "static";
      readonly id: string;
      readonly text: string;
      readonly role?: TextRole;
    }
  /** The release's text, omitted while empty or hidden. */
  | { readonly t: "text"; readonly id: string; readonly role?: TextRole }
  | {
      readonly t: "button";
      readonly id: string;
      readonly label?: string;
      readonly emphasis?: "primary" | "secondary" | "ghost";
    }
  /** A tool tab: a chip, selected while the release marks it so. */
  | { readonly t: "tab"; readonly id: string; readonly label: string }
  /** The release's link: same-site targets keep their `href`. */
  | {
      readonly t: "link";
      readonly id: string;
      readonly label?: string;
      readonly emphasis?: "primary";
    }
  | {
      readonly t: "field";
      readonly id: string;
      readonly label: string;
      readonly max: number;
      readonly multiline?: boolean;
    }
  | { readonly t: "select"; readonly id: string; readonly label: string }
  | { readonly t: "check"; readonly id: string; readonly label: string }
  | { readonly t: "range"; readonly id: string; readonly label: string }
  | { readonly t: "color"; readonly id: string; readonly label: string }
  | {
      readonly t: "file";
      readonly id: string;
      readonly label: string;
      readonly max: number;
    }
  | {
      readonly t: "canvas";
      readonly id: string;
      readonly label: string;
      readonly width: number;
      readonly height: number;
    }
  | { readonly t: "media"; readonly id: string; readonly label: string }
  /** A release list of plain lines. */
  | { readonly t: "items"; readonly id: string; readonly label: string }
  /** A release list whose rows carry buttons and links. */
  | { readonly t: "rows"; readonly id: string; readonly label: string }
  /** A disclosure: closed until the visitor opens it, unless `open`. */
  | {
      readonly t: "more";
      readonly id: string;
      readonly label: string;
      readonly open?: boolean;
      readonly children: ReadonlyArray<Spec>;
    };

/** The bridge IDs a layout reads. */
export function bridgeIds(specs: ReadonlyArray<Spec>): string[] {
  const ids: string[] = [];
  const walk = (spec: Spec) => {
    if (spec.t !== "static" && spec.t !== "more") ids.push(spec.id);
    if (spec.t === "window") ids.push(spec.close);
    if ("children" in spec) spec.children.forEach(walk);
  };
  specs.forEach(walk);
  return ids;
}

export interface EditorState {
  readonly mirror: Readonly<Record<string, Mirror>>;
  /** Disclosures the visitor opened or closed, by spec ID. */
  readonly disclosed: Readonly<Record<string, boolean>>;
  /** The editor's active tool (`objects`, `request`, ...). */
  readonly tool: string;
  /** Phone width: one tool window at a time. */
  readonly compact: boolean;
}

/** One surface the editor draws: where it reads and how it shows. */
export interface Surface {
  readonly id: string;
  readonly kind: "sheet" | "dialog" | "panel";
  readonly label: string;
  /** The release control that closes it (Escape, ×). */
  readonly dismiss?: string;
  readonly children: ReadonlyArray<Spec>;
}

const mirrorOf = (state: EditorState, id: string): Mirror =>
  state.mirror[id] ?? ABSENT;

const MAX_LINE = 4000;
const clip = (text: string) =>
  text.length > MAX_LINE ? text.slice(0, MAX_LINE) : text;

function render(spec: Spec, state: EditorState): Node[] {
  const m = mirrorOf(state, "id" in spec ? spec.id : "");
  switch (spec.t) {
    case "static":
      return [
        {
          id: spec.id,
          element: {
            kind: "text",
            value: spec.text,
            role: spec.role ?? "body",
          },
        },
      ];
    case "text":
      if (!m.shown || m.text === "") return [];
      return [
        {
          id: spec.id,
          ...(spec.role === "status" && { a11y: { live: "polite" } }),
          element: {
            kind: "text",
            value: clip(m.text),
            role: spec.role ?? "body",
          },
        },
      ];
    case "button": {
      if (!m.shown) return [];
      const label = spec.label ?? m.text;
      if (label.trim() === "") return [];
      return [
        {
          id: spec.id,
          element: {
            kind: "button",
            label,
            emphasis: spec.emphasis ?? "secondary",
            ...(!m.enabled && { enabled: false }),
          },
        },
      ];
    }
    case "tab":
      if (!m.shown) return [];
      return [
        {
          id: spec.id,
          element: { kind: "chip", label: spec.label, selected: m.on },
        },
      ];
    case "link": {
      if (!m.shown) return [];
      const label = spec.label ?? m.text;
      if (label.trim() === "") return [];
      return [
        {
          id: spec.id,
          element: {
            kind: "button",
            label,
            emphasis: spec.emphasis ?? "link",
            trailing: "forward",
            ...(m.href && { href: m.href }),
          },
        },
      ];
    }
    case "field":
      if (!m.shown) return [];
      return [
        {
          id: spec.id,
          element: {
            kind: "field",
            label: spec.label,
            value: m.value.slice(0, spec.max),
            // Up to four bytes a character, within the protocol's field bound.
            max_bytes: Math.min(spec.max * 4, MAX_FIELD_BYTES),
            ...(spec.multiline && { multiline: true }),
            ...(!m.enabled && { enabled: false }),
          },
        },
      ];
    case "select":
      if (!m.shown || m.options.length === 0) return [];
      return [
        {
          id: spec.id,
          element: {
            kind: "select",
            label: spec.label,
            value: m.options.some((o) => o.value === m.value)
              ? m.value
              : m.options[0]!.value,
            options: m.options,
            ...(!m.enabled && { enabled: false }),
          },
        },
      ];
    case "check":
      if (!m.shown) return [];
      return [
        {
          id: spec.id,
          element: {
            kind: "checkbox",
            label: spec.label,
            checked: m.checked,
            ...(!m.enabled && { enabled: false }),
          },
        },
      ];
    case "range": {
      if (!m.shown || !(m.min < m.max) || !(m.step > 0)) return [];
      const value = Math.min(m.max, Math.max(m.min, Number(m.value) || m.min));
      return [
        {
          id: spec.id,
          element: {
            kind: "range",
            label: spec.label,
            value,
            min: m.min,
            max: m.max,
            step: m.step,
            ...(!m.enabled && { enabled: false }),
          },
        },
      ];
    }
    case "color":
      if (!m.shown) return [];
      return [
        {
          id: spec.id,
          element: {
            kind: "color",
            label: spec.label,
            value: /^#[0-9a-f]{6}$/.test(m.value) ? m.value : "#000000",
            ...(!m.enabled && { enabled: false }),
          },
        },
      ];
    case "file":
      if (!m.shown) return [];
      return [
        {
          id: spec.id,
          element: {
            kind: "file",
            label: spec.label,
            slot: spec.id,
            ...(m.accept.length > 0 && { accept: m.accept }),
            max_bytes: spec.max,
            ...(m.multiple && { multiple: true }),
            ...(!m.enabled && { enabled: false }),
          },
        },
      ];
    case "canvas":
      if (!m.shown) return [];
      return [
        {
          id: `${spec.id}-slot`,
          ...(!m.enabled && {
            a11y: { description: "Choose Edit selected werewolf face first." },
          }),
          element: {
            kind: "canvas",
            surface: spec.id,
            label: spec.label,
            width: spec.width,
            height: spec.height,
          },
        },
      ];
    case "media":
      if (!m.shown) return [];
      return [
        {
          id: `${spec.id}-slot`,
          element: { kind: "media_slot", slot: spec.id, label: spec.label },
        },
      ];
    case "items":
      if (!m.shown || m.items.length === 0) return [];
      return [
        {
          id: spec.id,
          element: {
            kind: "list",
            label: spec.label,
            children: m.items.map((item, index) => ({
              id: `${spec.id}-${index}`,
              element: { kind: "text", value: clip(item), role: "caption" },
            })),
          },
        },
      ];
    case "rows":
      if (!m.shown || m.rows.length === 0) return [];
      return [
        {
          id: spec.id,
          element: {
            kind: "list",
            label: spec.label,
            children: m.rows.map((row, index) => ({
              id: `${spec.id}-${index}`,
              style: { gap: "xs" },
              element: {
                kind: "stack",
                axis: "vertical",
                children: [
                  ...row.texts.map((text, line): Node => ({
                    id: `${spec.id}-${index}-text-${line}`,
                    element: {
                      kind: "text",
                      value: clip(text),
                      role: line === 0 ? "body" : "caption",
                    },
                  })),
                  ...(row.actions.length > 0
                    ? [
                        {
                          id: `${spec.id}-${index}-actions`,
                          style: { gap: "xs" },
                          element: {
                            kind: "stack",
                            axis: "wrap",
                            children: row.actions.map(
                              (label, action): Node => ({
                                id: `${spec.id}-${index}-action-${action}`,
                                element: {
                                  kind: "button",
                                  label,
                                  emphasis: "ghost",
                                },
                              }),
                            ),
                          },
                        } satisfies Node,
                      ]
                    : []),
                ],
              },
            })),
          },
        },
      ];
    case "more": {
      const open = state.disclosed[spec.id] ?? spec.open ?? false;
      const inner = spec.children.flatMap((child) => render(child, state));
      if (inner.length === 0) return [];
      return [
        {
          id: spec.id,
          style: { gap: "sm" },
          element: {
            kind: "stack",
            axis: "vertical",
            children: [
              {
                id: `${spec.id}-toggle`,
                a11y: { description: open ? "Shown" : "Hidden" },
                element: { kind: "chip", label: spec.label, selected: open },
              },
              ...(open ? inner : []),
            ],
          },
        },
      ];
    }
    case "group": {
      if (spec.bridged !== false && !m.shown && state.mirror[spec.id])
        return [];
      const inner = spec.children.flatMap((child) => render(child, state));
      if (inner.length === 0) return [];
      return [
        {
          id: spec.id,
          style: { gap: spec.gap ?? "sm" },
          element: {
            kind: "stack",
            axis: spec.axis ?? "vertical",
            children: inner,
          },
        },
      ];
    }
    case "window": {
      if (!m.shown) return [];
      if (state.compact && state.tool !== spec.tool) return [];
      const close = mirrorOf(state, spec.close);
      return [
        {
          id: spec.id,
          style: { gap: "sm" },
          a11y: { role: "region", label: spec.title },
          element: {
            kind: "panel",
            title: spec.title,
            children: [
              ...(close.shown && !state.compact
                ? [
                    {
                      id: spec.close,
                      style: { align: "end" },
                      a11y: { label: `Close ${spec.title}` },
                      element: {
                        kind: "button",
                        label: "Close",
                        emphasis: "ghost",
                        leading: "close",
                      },
                    } satisfies Node,
                  ]
                : []),
              ...spec.children.flatMap((child) => render(child, state)),
            ],
          },
        },
      ];
    }
  }
}

/** Draw one surface, or nothing while the release keeps it closed. */
export function renderSurface(
  surface: Surface,
  state: EditorState,
): Node | undefined {
  const m = mirrorOf(state, surface.id);
  if (!m.shown) return undefined;
  const children = surface.children.flatMap((spec) => render(spec, state));
  switch (surface.kind) {
    case "panel":
      return {
        id: surface.id,
        style: { gap: "sm" },
        a11y: { role: "region", label: surface.label },
        element: { kind: "panel", children },
      };
    case "sheet":
      return {
        id: surface.id,
        element: {
          kind: "sheet",
          label: surface.label,
          open: true,
          dismissible: surface.dismiss !== undefined,
          children,
        },
      };
    case "dialog":
      return {
        id: surface.id,
        element: {
          kind: "dialog",
          label: surface.label,
          open: true,
          dismissible: surface.dismiss !== undefined,
          children,
        },
      };
  }
}

export function renderEditor(
  surfaces: ReadonlyArray<Surface>,
  state: EditorState,
): Node {
  return {
    id: "house-editor",
    element: {
      kind: "stack",
      axis: "vertical",
      children: surfaces.flatMap((surface) => {
        const node = renderSurface(surface, state);
        return node ? [node] : [];
      }),
    },
  };
}

/** Fold what the release shows into the state; the visitor's disclosures stay. */
export const editorReport = (
  state: EditorState,
  report: Pick<EditorState, "mirror" | "tool" | "compact">,
): EditorState => ({ ...state, ...report });

/** Find the layout entry an intent names. */
function find(surfaces: ReadonlyArray<Surface>, id: string): Spec | undefined {
  const pending: Spec[] = surfaces.flatMap((s) => [...s.children]);
  while (pending.length > 0) {
    const spec = pending.pop()!;
    if ("id" in spec && spec.id === id) return spec;
    if ("children" in spec) pending.push(...spec.children);
  }
  return undefined;
}

const ROW_ACTION = /^(.+)-(\d+)-action-(\d+)$/;

export const makeEditorController = (
  surfaces: ReadonlyArray<Surface>,
  initial: EditorState,
  instance = `editor-${Date.now().toString(36)}`,
) =>
  Effect.gen(function* () {
    const bridge = yield* EditorBridge;
    const navigator = yield* Navigator;
    const closes = new Set(
      surfaces
        .flatMap((surface) => bridgeIds(surface.children))
        .filter((id) => /^close-/.test(id)),
    );
    return yield* Controller.make<EditorState>({
      instance,
      initial,
      render: (state) => renderEditor(surfaces, state),
      handle: (state, intent, node) =>
        Effect.gen(function* () {
          const id = intent.node;
          // Escape or × on a surface: the release's own close control.
          const surface = surfaces.find((s) => s.id === id);
          if (surface) {
            if (intent.kind === "dismiss" && surface.dismiss)
              yield* bridge.act({ op: "click", id: surface.dismiss });
            return state;
          }
          if (id.endsWith("-toggle")) {
            const more = find(surfaces, id.slice(0, -"-toggle".length));
            if (more?.t === "more") {
              const open = state.disclosed[more.id] ?? more.open ?? false;
              return {
                ...state,
                disclosed: { ...state.disclosed, [more.id]: !open },
              };
            }
          }
          const row = ROW_ACTION.exec(id);
          if (row && find(surfaces, row[1]!)?.t === "rows") {
            yield* bridge.act({
              op: "row",
              id: row[1]!,
              row: Number(row[2]),
              index: Number(row[3]),
            });
            return state;
          }
          // A window's close button is the release's.
          if (closes.has(id)) {
            yield* bridge.act({ op: "click", id });
            return state;
          }
          const spec = find(surfaces, id);
          if (!spec) return state;
          switch (spec.t) {
            case "button":
            case "tab":
              yield* bridge.act({ op: "click", id });
              break;
            case "link": {
              const href =
                node.element.kind === "button" ? node.element.href : undefined;
              // Same-site links go through the controller; others are the
              // release's (a download or a new tab).
              if (href) yield* navigator.go(href);
              else yield* bridge.act({ op: "click", id });
              break;
            }
            case "field":
              if (intent.kind === "input")
                yield* bridge.act({ op: "set", id, value: intent.value ?? "" });
              break;
            case "select":
            case "range":
            case "color":
              yield* bridge.act({ op: "set", id, value: intent.value ?? "" });
              break;
            case "check":
              yield* bridge.act({
                op: "check",
                id,
                checked: intent.value === "true",
              });
              break;
            case "file":
              yield* bridge.act({ op: "files", id });
              break;
          }
          return state;
        }),
    });
  });
