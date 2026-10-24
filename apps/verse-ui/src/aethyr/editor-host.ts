// The web host's half of the house editor (`editor.ts`): it reads the
// release's controls (the bridge) into mirrors, carries each intent to the
// release's own control, moves the release's canvas and capture image into
// the renderer's slots, and keeps the release's dialogs from going modal
// (the controller's dialogs take their place).
import { Effect, Layer } from "effect";
import {
  ABSENT,
  EditorBridge,
  bridgeIds,
  type BridgeOp,
  type EditorState,
  type Mirror,
  type Row,
  type Surface,
} from "./editor";
import type { Files, Mount } from "./renderer";
import { validAccept } from "./validate";

const byId = (id: string) => document.getElementById(id);
const text = (node: Element | null | undefined) =>
  (node?.textContent ?? "").replace(/\s+/g, " ").trim();

/** Phone width, as the release decides it (`studio_browser.rs`). */
const COMPACT = 800;

/** Hidden by the release itself: `hidden` on it or an ancestor below `root`. */
function hiddenWithin(element: Element, root: Element): boolean {
  for (
    let node: Element | null = element;
    node && node !== root;
    node = node.parentElement
  )
    if (node.hasAttribute("hidden")) return true;
  return false;
}

const sameSite = (href: string | null) =>
  href !== null && href.startsWith("/") && !href.startsWith("//")
    ? href
    : undefined;

function rows(list: Element): Row[] {
  return Array.from(list.children, (li) => {
    const actions: string[] = [];
    const texts: string[] = [];
    for (const child of Array.from(li.children)) {
      if (child.matches("button, a")) actions.push(text(child));
      else if (text(child)) texts.push(text(child));
    }
    if (texts.length === 0 && li.children.length === 0 && text(li))
      texts.push(text(li));
    return { texts, actions };
  });
}

function read(element: HTMLElement, root: Element): Mirror {
  const input = element as HTMLInputElement;
  const disabled =
    element.matches(":disabled") ||
    element.closest("fieldset:disabled") !== null ||
    element.getAttribute("aria-disabled") === "true";
  const flag = (name: string) => element.getAttribute(name) === "true";
  const tag = element.tagName;
  const isList = tag === "OL" || tag === "UL";
  const numeric = (name: string, fallback: number) => {
    const value = Number(element.getAttribute(name));
    return Number.isFinite(value) && element.hasAttribute(name)
      ? value
      : fallback;
  };
  return {
    shown: element === root ? !element.hidden : !hiddenWithin(element, root),
    enabled: !disabled,
    text: tag === "INPUT" ? input.value : text(element),
    value:
      tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT"
        ? (element as HTMLInputElement).value
        : "",
    checked: tag === "INPUT" && input.checked,
    options:
      tag === "SELECT"
        ? Array.from((element as HTMLSelectElement).options, (option) => ({
            value: option.value,
            label: text(option) || option.value || "None",
          }))
        : [],
    href: tag === "A" ? sameSite(element.getAttribute("href")) : undefined,
    on: flag("aria-selected") || flag("aria-expanded") || flag("aria-pressed"),
    items:
      isList && !element.querySelector("button, a")
        ? Array.from(element.children, (li) => text(li)).filter(Boolean)
        : [],
    rows: isList && element.querySelector("button, a") ? rows(element) : [],
    accept:
      tag === "INPUT" && input.type === "file"
        ? input.accept
            .split(",")
            .map((entry) => entry.trim().toLowerCase())
            .filter(validAccept)
        : [],
    multiple: tag === "INPUT" && input.multiple,
    min: numeric("min", 0),
    max: numeric("max", 100),
    step: numeric("step", 1),
  };
}

/** Everything the release shows on the editor's controls now. */
export function readEditor(
  surfaces: ReadonlyArray<Surface>,
): Pick<EditorState, "mirror" | "tool" | "compact"> {
  const mirror: Record<string, Mirror> = {};
  for (const surface of surfaces) {
    const root = byId(surface.id);
    if (!root) continue;
    mirror[surface.id] = read(root, root);
    for (const id of bridgeIds(surface.children)) {
      const element = byId(id);
      if (element) mirror[id] = read(element, root);
    }
  }
  return {
    mirror,
    tool: byId("edit-panel")?.dataset.active ?? "objects",
    compact: window.innerWidth < COMPACT,
  };
}

export const initialEditor = (
  surfaces: ReadonlyArray<Surface>,
): EditorState => ({
  ...readEditor(surfaces),
  disclosed: {},
});

const announce = (element: HTMLElement, ...events: string[]) => {
  for (const name of events)
    element.dispatchEvent(new Event(name, { bubbles: true }));
};

/** The release's dialogs open non-modally; the controller draws the modal. */
function tameDialogs(surfaces: ReadonlyArray<Surface>) {
  for (const surface of surfaces) {
    if (surface.kind !== "dialog") continue;
    const dialog = byId(surface.id);
    if (dialog instanceof HTMLDialogElement) {
      dialog.showModal = () => dialog.show();
      if (dialog.open) {
        dialog.close();
        dialog.show();
      }
    }
  }
}

export function editorHost(surfaces: ReadonlyArray<Surface>) {
  tameDialogs(surfaces);
  const files = new Map<string, ReadonlyArray<File>>();
  // Where each moved element came from, so it can go back.
  const homes = new Map<string, { parent: Node; next: Node | null }>();

  const act = (op: BridgeOp) =>
    Effect.sync(() => {
      const element = byId(op.id);
      if (!element) return;
      switch (op.op) {
        case "click":
          element.click();
          break;
        case "set": {
          const control = element as HTMLInputElement;
          control.value = op.value;
          announce(control, "input");
          if (
            control.tagName === "SELECT" ||
            ["range", "color"].includes(control.type)
          )
            announce(control, "change");
          break;
        }
        case "check":
          if ((element as HTMLInputElement).checked !== op.checked)
            element.click();
          break;
        case "files": {
          const chosen = files.get(op.id);
          if (!chosen) break;
          const transfer = new DataTransfer();
          for (const file of chosen) transfer.items.add(file);
          (element as HTMLInputElement).files = transfer.files;
          files.delete(op.id);
          announce(element, "input", "change");
          break;
        }
        case "row": {
          const row = element.children[op.row];
          const action = row?.querySelectorAll<HTMLElement>(
            ":scope > button, :scope > a",
          )[op.index];
          action?.click();
          break;
        }
      }
    });

  const mount: Mount = (kind, id, slot) => {
    if (kind === "surface") return;
    const element = byId(id);
    if (!element) return;
    if (slot) {
      if (!homes.has(id))
        homes.set(id, {
          parent: element.parentNode!,
          next: element.nextSibling,
        });
      if (element.parentNode !== slot) slot.append(element);
    } else {
      const home = homes.get(id);
      if (home && home.parent.isConnected)
        home.parent.insertBefore(element, home.next);
      homes.delete(id);
    }
  };
  const onFiles: Files = (slot, chosen) => void files.set(slot, chosen);

  return {
    layer: Layer.succeed(EditorBridge, { act }),
    mount,
    onFiles,
    restore: () => {
      for (const id of [...homes.keys()]) mount("canvas", id, null);
    },
  };
}

export { ABSENT };
