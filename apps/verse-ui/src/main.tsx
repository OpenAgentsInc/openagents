import { createRoot } from "react-dom/client";
import { useEffect } from "react";
import { Effect, Fiber } from "effect";
import * as Atom from "effect/reactivity/Atom";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import { RegistryContext, useAtom, useAtomValue } from "@effect/atom-react";
import { Search, ArrowUpRight } from "lucide-react";
import { Button, buttonVariants } from "./components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogTrigger,
} from "./components/ui/dialog";
import {
  Command,
  CommandInput,
  CommandList,
  CommandEmpty,
  CommandGroup,
  CommandItem,
} from "./components/ui/command";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./components/ui/tooltip";
import {
  availableActions,
  invokeAction,
  type AvailableAction,
} from "./actions";

import { inputStyles } from "./components/ui/input";
import { textareaStyles } from "./components/ui/textarea";
import { nativeSelectStyles } from "./components/ui/native-select";

const openAtom = Atom.make(false);
const actionsAtom = Atom.make<AvailableAction[]>([]);

function QuickActions() {
  const [open, setOpen] = useAtom(openAtom);
  const actions = useAtomValue(actionsAtom);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (
        (event.metaKey || event.ctrlKey) &&
        event.key.toLowerCase() === "k" &&
        !document.querySelector("dialog[open]")
      ) {
        event.preventDefault();
        setOpen((value) => !value);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [setOpen]);
  if (actions.length === 0) return null;
  return (
    <TooltipProvider>
      <Dialog open={open} onOpenChange={setOpen}>
        <Tooltip>
          <TooltipTrigger asChild>
            <DialogTrigger asChild>
              <Button variant="outline" size="icon" aria-label="Find a tool">
                <Search className="size-4" />
              </Button>
            </DialogTrigger>
          </TooltipTrigger>
          <TooltipContent>Find a tool · Ctrl / ⌘ K</TooltipContent>
        </Tooltip>
        <DialogContent
          className="p-0 gap-0 overflow-hidden"
          onKeyDown={(event) => event.stopPropagation()}
          onEscapeKeyDown={(event) => event.stopPropagation()}
        >
          <DialogHeader className="sr-only">
            <DialogTitle>Find a tool</DialogTitle>
            <DialogDescription>
              Choose an available tool. Use the arrow keys and Enter.
            </DialogDescription>
          </DialogHeader>
          <Command>
            <CommandInput placeholder="Find a tool…" />
            <CommandList>
              <CommandEmpty>No available tools match.</CommandEmpty>
              <CommandGroup heading="In this world">
                {actions.map((action) => (
                  <CommandItem
                    key={action.id}
                    value={action.label}
                    onSelect={() => {
                      setOpen(false);
                      requestAnimationFrame(() =>
                        invokeAction(document, action.id),
                      );
                    }}
                  >
                    <span className="flex-1">{action.label}</span>
                    <ArrowUpRight className="size-4" />
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        </DialogContent>
      </Dialog>
    </TooltipProvider>
  );
}

/** Add the owned shadcn button styles without replacing a controller's DOM handles. */
function styleControls() {
  for (const node of document.querySelectorAll<HTMLElement>(
    "button, input[type=submit], .small-button",
  )) {
    if (
      // Aethyr renderer controls carry the design system's own classes.
      node.closest(
        "#verse-ui-root,[data-slot=dialog-content],[data-aethyr-root]",
      ) ||
      node.dataset.verseControl ||
      // HUD components already carry the design system's own styles.
      node.matches(".hud-button, .verse-world-chip")
    )
      continue;
    node.classList.add(
      ...buttonVariants({ variant: "secondary" }).split(" ").filter(Boolean),
    );
    node.dataset.verseControl = "true";
  }
  for (const [selector, styles] of [
    [
      "input:not([type=hidden]):not([type=checkbox]):not([type=range]):not([type=color]):not([type=submit])",
      inputStyles,
    ],
    ["textarea", textareaStyles],
    ["select", nativeSelectStyles],
  ]) {
    for (const node of document.querySelectorAll<HTMLElement>(selector)) {
      if (
        node.dataset.verseControl ||
        node.closest("[data-slot=dialog-content],[data-aethyr-root]")
      )
        continue;
      node.classList.add(...styles.split(" ").filter(Boolean));
      node.dataset.verseControl = "true";
    }
  }
}

function acquireUi() {
  const registry = AtomRegistry.make();
  const element = document.createElement("div");
  element.id = "verse-ui-root";
  document.body.append(element);
  document.documentElement.classList.add("dark");
  const root = createRoot(element);
  root.render(
    <RegistryContext.Provider value={registry}>
      <QuickActions />
    </RegistryContext.Provider>,
  );
  let frame = 0;
  let last = "";
  const refresh = () => {
    frame = 0;
    styleControls();
    const actions = availableActions(document);
    const text = JSON.stringify(actions);
    if (text !== last) {
      last = text;
      registry.set(actionsAtom, actions);
    }
  };
  const observer = new MutationObserver((records) => {
    if (
      records.some(
        (record) =>
          record.type === "attributes" ||
          Array.from(record.addedNodes).some((node) => node instanceof Element),
      ) &&
      !frame
    )
      frame = requestAnimationFrame(refresh);
  });
  observer.observe(document.body, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: [
      "hidden",
      "disabled",
      "inert",
      "aria-pressed",
      "aria-selected",
      "open",
    ],
  });
  refresh();
  const resize = () => {
    if (!frame) frame = requestAnimationFrame(refresh);
  };
  window.addEventListener("resize", resize);
  return () => {
    observer.disconnect();
    cancelAnimationFrame(frame);
    window.removeEventListener("resize", resize);
    root.unmount();
    registry.dispose();
    element.remove();
  };
}

declare global {
  interface Window {
    verseUi?: { close: () => void };
  }
}
if (document.documentElement.hasAttribute("data-verse-ui") && !window.verseUi) {
  const lifetime = Effect.scoped(
    Effect.gen(function* () {
      yield* Effect.acquireRelease(Effect.sync(acquireUi), (dispose) =>
        Effect.sync(dispose),
      );
      yield* Effect.never;
    }),
  );
  const fiber = Effect.runFork(lifetime);
  const close = () => Effect.runFork(Fiber.interrupt(fiber));
  window.verseUi = { close };
  // BFCache suspends a page; preserve the UI and subscriptions until a real unload.
  window.addEventListener("pagehide", (event) => {
    if (!event.persisted) close();
  });
}
