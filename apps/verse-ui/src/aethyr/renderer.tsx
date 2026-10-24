// The React DOM renderer for Aethyr view trees (ADR 0005). It draws any
// valid tree with the design system (HUD classes and shadcn primitives) and
// sends intents back. It keeps no state of its own beyond DOM focus and
// hover: every value it shows comes from the tree.
import {
  createContext,
  useCallback,
  useContext,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
} from "react";
import { Input } from "../components/ui/input";
import { Textarea } from "../components/ui/textarea";
import type {
  A11y,
  Element,
  Glyph,
  Intent,
  IntentKind,
  Node,
  Style,
  View,
} from "./protocol";
import { byteLength } from "./validate";

/** Where the host attaches what it owns: media and the engine surface. */
export type Mount = (
  kind: "media" | "surface",
  id: string,
  element: HTMLElement | null,
) => void;

export interface RendererProps {
  readonly view: View;
  /**
   * Handles one intent. A host that returns a promise lets fields know when
   * their last value has been handled (see `FieldView`).
   */
  readonly onIntent: (intent: Intent) => void | PromiseLike<unknown>;
  readonly mount?: Mount;
}

interface Scope {
  readonly send: (
    node: string,
    kind: IntentKind,
    value?: string,
  ) => void | PromiseLike<unknown>;
  /** The revision drawn now. */
  readonly revision: () => number;
  readonly mount: Mount | undefined;
}

const ScopeContext = createContext<Scope>({
  send: () => {},
  revision: () => 0,
  mount: undefined,
});

const GLYPHS: Record<Glyph, string> = { forward: "↗", back: "←", close: "×" };

const classes = (...names: Array<string | false | undefined>) =>
  names.filter(Boolean).join(" ");

function styleClasses(style: Style | undefined) {
  if (!style) return [];
  return [
    style.gap && `aui-gap-${style.gap}`,
    style.padding && `aui-pad-${style.padding}`,
    style.align && `aui-align-${style.align}`,
    style.tone && `aui-tone-${style.tone}`,
    style.hide_on_phone && "aui-phone-hide",
  ];
}

function a11yProps(a11y: A11y | undefined) {
  if (!a11y) return {};
  return {
    ...(a11y.role && { role: a11y.role }),
    ...(a11y.label !== undefined && { "aria-label": a11y.label }),
    ...(a11y.description !== undefined && {
      "aria-description": a11y.description,
    }),
    ...(a11y.live && { "aria-live": a11y.live }),
    ...(a11y.hidden && { "aria-hidden": true }),
  };
}

/** A plain primary click: the controller decides; others follow the link. */
const plainClick = (event: MouseEvent) =>
  event.button === 0 &&
  !event.metaKey &&
  !event.ctrlKey &&
  !event.shiftKey &&
  !event.altKey;

export function AethyrView({ view, onIntent, mount }: RendererProps) {
  // The tree, not a closure, decides the revision an intent names.
  const latest = useRef(view);
  latest.current = view;
  const send = useCallback(
    (node: string, kind: IntentKind, value?: string) => {
      const { instance, revision } = latest.current;
      return onIntent(
        value === undefined
          ? { instance, revision, node, kind }
          : { instance, revision, node, kind, value },
      );
    },
    [onIntent],
  );
  const revision = useCallback(() => latest.current.revision, []);
  return (
    <ScopeContext.Provider value={{ send, revision, mount }}>
      <div
        className="aui-root"
        data-aethyr-root={view.instance}
        data-aethyr-revision={view.revision}
      >
        <NodeView node={view.root} />
      </div>
    </ScopeContext.Provider>
  );
}

function NodeView({ node }: { node: Node }): ReactNode {
  const common = {
    "data-aethyr-id": node.id,
    "data-aethyr-kind": node.element.kind,
    ...a11yProps(node.a11y),
  };
  const extra = styleClasses(node.style);
  return <ElementView node={node} common={common} extra={extra} />;
}

const Children = ({ nodes }: { nodes: ReadonlyArray<Node> }) =>
  nodes.map((child) => <NodeView key={child.id} node={child} />);

type ElementProps = {
  node: Node;
  common: Record<string, unknown>;
  extra: Array<string | false | undefined>;
};

function ElementView({ node, common, extra }: ElementProps): ReactNode {
  const element = node.element;
  switch (element.kind) {
    case "panel":
      return (
        <section
          className={classes("hud-panel", "aui-panel", ...extra)}
          aria-label={element.title}
          {...common}
        >
          {element.title !== undefined && (
            <p className="hud-label" aria-hidden="true">
              {element.title}
            </p>
          )}
          <Children nodes={element.children} />
        </section>
      );
    case "stack":
      return (
        <div
          className={classes("aui-stack", `aui-axis-${element.axis}`, ...extra)}
          {...common}
        >
          <Children nodes={element.children} />
        </div>
      );
    case "text":
      return <TextView element={element} common={common} extra={extra} />;
    case "label":
      return (
        <p className={classes("hud-label", ...extra)} {...common}>
          {element.signal && <span className="hud-signal" aria-hidden="true" />}
          {element.value}
        </p>
      );
    case "button":
      return <ButtonView node={node} common={common} extra={extra} />;
    case "chip":
      return <ChipView node={node} common={common} extra={extra} />;
    case "list":
      return <ListView node={node} common={common} extra={extra} />;
    case "field":
      return <FieldView node={node} common={common} extra={extra} />;
    case "sheet":
      return <SheetView node={node} common={common} extra={extra} />;
    case "dialog":
      return <DialogView node={node} common={common} extra={extra} />;
    case "toast":
      return <ToastView node={node} common={common} extra={extra} />;
    case "media_slot":
      return <MountView node={node} common={common} extra={extra} />;
    case "engine_surface":
      return <MountView node={node} common={common} extra={extra} />;
  }
}

function TextView({
  element,
  common,
  extra,
}: {
  element: Extract<Element, { kind: "text" }>;
  common: Record<string, unknown>;
  extra: ElementProps["extra"];
}) {
  const className = (base: string) => classes(base, ...extra);
  switch (element.role) {
    case "title":
      return (
        <h2 className={className("aui-title")} {...common}>
          {element.value}
        </h2>
      );
    case "heading":
      return (
        <h3 className={className("aui-heading")} {...common}>
          {element.value}
        </h3>
      );
    case "caption":
      return (
        <p className={className("aui-caption")} {...common}>
          {element.value}
        </p>
      );
    case "code":
      return (
        <code className={className("aui-code")} {...common}>
          {element.value}
        </code>
      );
    case "status":
      return (
        <p
          className={className("aui-status")}
          role="status"
          aria-live="polite"
          {...common}
        >
          {element.value}
        </p>
      );
    case "body":
      return (
        <p className={className("aui-body")} {...common}>
          {element.value}
        </p>
      );
  }
}

function ButtonView({ node, common, extra }: ElementProps) {
  const { send } = useContext(ScopeContext);
  const element = node.element as Extract<Element, { kind: "button" }>;
  const enabled = element.enabled !== false;
  const emphasis = element.emphasis ?? "secondary";
  const className = classes(
    emphasis === "link" ? "hud-link" : "hud-button",
    emphasis === "primary" && "hud-primary",
    emphasis === "ghost" && "hud-ghost",
    "aui-button",
    ...extra,
  );
  const body = (
    <>
      {element.leading && (
        <span className="aui-glyph" aria-hidden="true">
          {GLYPHS[element.leading]}
        </span>
      )}
      <span>{element.label}</span>
      {element.trailing && (
        <span className="aui-glyph" aria-hidden="true">
          {GLYPHS[element.trailing]}
        </span>
      )}
    </>
  );
  if (element.href !== undefined)
    return (
      <a
        className={className}
        href={enabled ? element.href : undefined}
        aria-disabled={enabled ? undefined : true}
        onClick={(event) => {
          if (!enabled) return event.preventDefault();
          if (!plainClick(event)) return;
          event.preventDefault();
          send(node.id, "activate");
        }}
        {...common}
      >
        {body}
      </a>
    );
  return (
    <button
      type="button"
      className={className}
      disabled={!enabled}
      onClick={() => send(node.id, "activate")}
      {...common}
    >
      {body}
    </button>
  );
}

function ChipView({ node, common, extra }: ElementProps) {
  const { send } = useContext(ScopeContext);
  const element = node.element as Extract<Element, { kind: "chip" }>;
  return (
    <button
      type="button"
      className={classes(
        "aui-chip",
        element.detail !== undefined && "aui-chip-detailed",
        ...extra,
      )}
      aria-pressed={element.selected}
      disabled={element.enabled === false}
      onClick={() => send(node.id, "activate")}
      {...common}
    >
      {element.detail === undefined ? (
        element.label
      ) : (
        <>
          <span className="aui-chip-label">{element.label}</span>
          <small className="aui-chip-detail">{element.detail}</small>
        </>
      )}
    </button>
  );
}

function ListView({ node, common, extra }: ElementProps) {
  const { send } = useContext(ScopeContext);
  const element = node.element as Extract<Element, { kind: "list" }>;
  if (!element.selectable)
    return (
      <ul
        className={classes("aui-list", ...extra)}
        aria-label={element.label}
        {...common}
      >
        {element.children.map((child) => (
          <li key={child.id}>
            <NodeView node={child} />
          </li>
        ))}
      </ul>
    );
  const focusable = element.selected ?? element.children[0]?.id ?? undefined;
  const move = (event: KeyboardEvent<HTMLDivElement>, index: number) => {
    const step =
      event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      send(node.id, "select", element.children[index]!.id);
    } else if (step) {
      event.preventDefault();
      const options =
        event.currentTarget.parentElement?.querySelectorAll<HTMLElement>(
          ":scope > [role=option]",
        );
      options?.[
        Math.max(0, Math.min(index + step, options.length - 1))
      ]?.focus();
    }
  };
  return (
    <div
      role="listbox"
      className={classes("aui-list", ...extra)}
      aria-label={element.label}
      {...common}
    >
      {element.children.map((child, index) => (
        <div
          key={child.id}
          role="option"
          className="aui-option"
          aria-selected={element.selected === child.id}
          tabIndex={child.id === focusable ? 0 : -1}
          onClick={() => send(node.id, "select", child.id)}
          onKeyDown={(event) => move(event, index)}
        >
          <NodeView node={child} />
        </div>
      ))}
    </div>
  );
}

/**
 * A field keeps a local draft while its `input` intents are handled, so a
 * slow handler never drops keystrokes or moves the caret back. One input is
 * in flight at a time; when it settles, the newest draft is sent against the
 * newest revision (an intent for a replaced revision would be refused as
 * stale). The draft ends when the tree shows it, or when the controller
 * replaces the value with one this field never sent.
 */
function FieldView({ node, common, extra }: ElementProps) {
  const { send, revision } = useContext(ScopeContext);
  const element = node.element as Extract<Element, { kind: "field" }>;
  const enabled = element.enabled !== false;
  const [draft, setDraft] = useState<string | undefined>(undefined);
  const [settled, setSettled] = useState(0);
  const flight = useRef(false);
  const sent = useRef<{ value: string; revision: number } | undefined>(
    undefined,
  );
  const values = useRef(new Set<string>());
  const push = useCallback(
    (value: string) => {
      const at = revision();
      if (sent.current?.value === value && sent.current.revision === at) return;
      sent.current = { value, revision: at };
      values.current.add(value);
      const done = send(node.id, "input", value);
      if (done && typeof done.then === "function") {
        flight.current = true;
        const finish = () => {
          flight.current = false;
          setSettled((n) => n + 1);
        };
        done.then(finish, finish);
      }
    },
    [send, revision, node.id],
  );
  useLayoutEffect(() => {
    if (draft === undefined || flight.current) return;
    if (draft === element.value) {
      setDraft(undefined);
      values.current.clear();
    } else if (!values.current.has(element.value) && sent.current) {
      // The controller chose another value (it cleared or reset the field).
      const pendingUserEdit = draft !== sent.current.value;
      if (!pendingUserEdit) {
        setDraft(undefined);
        values.current.clear();
        sent.current = undefined;
        return;
      }
      push(draft);
    } else push(draft);
  }, [draft, element.value, settled, push]);
  const change = (value: string) => {
    if (byteLength(value) > element.max_bytes) return;
    setDraft(value);
    if (!flight.current) push(value);
  };
  const value = draft ?? element.value;
  return (
    <label className={classes("aui-field", ...extra)} {...common}>
      <span className="aui-field-label">{element.label}</span>
      {element.multiline ? (
        <Textarea
          value={value}
          placeholder={element.placeholder}
          disabled={!enabled}
          onChange={(event) => change(event.target.value)}
        />
      ) : (
        <Input
          type={element.secret ? "password" : "text"}
          value={value}
          placeholder={element.placeholder}
          disabled={!enabled}
          autoComplete={element.secret ? "off" : undefined}
          onChange={(event) => change(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.nativeEvent.isComposing) {
              event.preventDefault();
              send(node.id, "submit");
            }
          }}
        />
      )}
    </label>
  );
}

function CloseButton({ node, label }: { node: string; label: string }) {
  const { send } = useContext(ScopeContext);
  return (
    <button
      type="button"
      className="hud-button hud-ghost aui-close"
      aria-label={`Close ${label}`}
      onClick={() => send(node, "dismiss")}
    >
      <span aria-hidden="true">{GLYPHS.close}</span>
    </button>
  );
}

function SheetView({ node, common, extra }: ElementProps) {
  const { send } = useContext(ScopeContext);
  const element = node.element as Extract<Element, { kind: "sheet" }>;
  return (
    <section
      role="dialog"
      aria-modal="false"
      aria-label={element.label}
      className={classes("hud-panel", "aui-sheet", ...extra)}
      hidden={!element.open}
      onKeyDown={(event) => {
        if (event.key === "Escape" && element.dismissible) {
          event.stopPropagation();
          send(node.id, "dismiss");
        }
      }}
      {...common}
    >
      {element.dismissible && (
        <CloseButton node={node.id} label={element.label} />
      )}
      <Children nodes={element.children} />
    </section>
  );
}

function DialogView({ node, common, extra }: ElementProps) {
  const { send } = useContext(ScopeContext);
  const element = node.element as Extract<Element, { kind: "dialog" }>;
  const ref = useRef<HTMLDialogElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (element.open && !dialog.open) {
      opener.current = document.activeElement as HTMLElement | null;
      dialog.showModal();
    } else if (!element.open && dialog.open) {
      dialog.close();
      if (opener.current?.isConnected) opener.current.focus();
    }
  }, [element.open]);
  return (
    <dialog
      ref={ref}
      className={classes("hud-panel", "hud-dialog", "aui-dialog", ...extra)}
      aria-label={element.label}
      onCancel={(event) => {
        // The controller decides whether Escape closes it.
        event.preventDefault();
        if (element.dismissible) send(node.id, "dismiss");
      }}
      {...common}
    >
      <div className="aui-stack aui-gap-md">
        <Children nodes={element.children} />
      </div>
      {element.dismissible && (
        <CloseButton node={node.id} label={element.label} />
      )}
    </dialog>
  );
}

function ToastView({ node, common, extra }: ElementProps) {
  const element = node.element as Extract<Element, { kind: "toast" }>;
  const tone = element.tone ?? "default";
  return (
    <div
      className={classes(
        "hud-toast",
        "aui-toast",
        tone !== "default" && `aui-tone-${tone}`,
        ...extra,
      )}
      role={tone === "danger" ? "alert" : "status"}
      {...common}
    >
      <span>{element.message}</span>
      {element.dismissible && <CloseButton node={node.id} label="message" />}
    </div>
  );
}

function MountView({ node, common, extra }: ElementProps) {
  const { mount } = useContext(ScopeContext);
  const element = node.element as Extract<
    Element,
    { kind: "media_slot" } | { kind: "engine_surface" }
  >;
  const media = element.kind === "media_slot";
  const id = media ? element.slot : element.surface;
  // Stable per slot, so React keeps the host's content mounted across
  // revisions instead of detaching and reattaching it.
  const attach = useCallback(
    (el: HTMLElement | null) => mount?.(media ? "media" : "surface", id, el),
    [mount, media, id],
  );
  return (
    <div
      ref={attach}
      className={classes(
        media ? "aui-media" : "aui-surface",
        media && element.aspect && `aui-aspect-${element.aspect}`,
        ...extra,
      )}
      role={media ? "img" : "application"}
      aria-label={element.label}
      data-aethyr-slot={id}
      {...common}
    />
  );
}
