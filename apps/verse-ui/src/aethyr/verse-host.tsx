// The web host for the Verse controller on /new-lander/verse.
//
// The server page is a shell: the stage (`#engine`, `#world-notice`) inside
// `#universe-exploration`, the `#verse-hud` mount point, and the engine
// bridge. The pinned engine release (`aethyrforge-host`'s
// `mount_universe`) still expects its original controls by ID, so the
// server keeps them in `.verse-bridge`, which is hidden once this host
// mounts. The host turns intents into clicks on that bridge and reads the
// engine's state back from it; visitors only see the controller's tree.
// Without JavaScript the bridge is the page, and its links still work.
import { createRoot } from "react-dom/client";
import { Effect, Layer, ManagedRuntime } from "effect";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import { RegistryContext } from "@effect/atom-react";
import { ControllerView } from "./host";
import {
  EntryOptions,
  GuestMemory,
  Navigator,
  NO_PLANET,
  UniverseEngine,
  VerseOptions,
  engineReport,
  initialVerse,
  makeVerseController,
  type EngineReport,
  type EnginePhase,
  type Planet,
  type World,
} from "./verse";

/** The server's page configuration (`#verse-config`). */
interface VerseConfig {
  readonly accounts: boolean;
  readonly signedIn: boolean;
  readonly hauntPath: string | null;
  readonly blueRushPath: string;
  readonly mapPath: string;
  readonly worlds: ReadonlyArray<World>;
}

/** What the pinned engine release exposes once mounted. */
interface UniverseDemo {
  readonly visit: () => string;
}

declare global {
  interface Window {
    universeDemo?: UniverseDemo;
    aethyrVerse?: { readonly close: () => void };
  }
}

// The engine release stores the visit under this key (aethyrforge-host).
const VISIT = "openagents.verse-map.v1";

const byId = (id: string) => document.getElementById(id);
const shown = (id: string) => {
  const node = byId(id);
  return node !== null && !node.hidden;
};
const textOf = (selector: string) =>
  document.querySelector(selector)?.textContent?.trim() ?? "";

function readConfig(): VerseConfig | undefined {
  try {
    const text = byId("verse-config")?.textContent;
    return text ? (JSON.parse(text) as VerseConfig) : undefined;
  } catch {
    return undefined;
  }
}

function parseVisit(text: string | null | undefined) {
  try {
    const visit = JSON.parse(text ?? "") as {
      choice?: string | null;
      exploring?: boolean;
      selected?: string;
    };
    return {
      entered: typeof visit.choice === "string",
      exploring: visit.exploring === true && typeof visit.choice === "string",
      selected: typeof visit.selected === "string" ? visit.selected : "",
    };
  } catch {
    return undefined;
  }
}

function remembered() {
  try {
    return parseVisit(window.sessionStorage.getItem(VISIT));
  } catch {
    return undefined;
  }
}

function enginePhase(): EnginePhase {
  const notice = byId("world-notice");
  if (!notice) return "error";
  if (notice.dataset.state === "error") return "error";
  return notice.hidden ? "open" : "opening";
}

function readPlanet(): Planet {
  const owned = shown("planet-owned");
  const ready = !owned && shown("planet-ready");
  const create = byId("planet-create") as HTMLButtonElement | null;
  return {
    stage: owned ? "owned" : ready ? "ready" : "none",
    heading: textOf("#planet-owned h3"),
    status: textOf("#planet-status") || NO_PLANET.status,
    canCreate: create !== null && !create.disabled,
    setupPath: byId("planet-setup")?.getAttribute("href") ?? undefined,
  };
}

/** Everything the engine bridge says now. */
function readEngine(): EngineReport {
  const engine = enginePhase();
  const demo = window.universeDemo;
  if (!demo) return { engine };
  const visit = parseVisit(demo.visit());
  const planet = readPlanet();
  const report: EngineReport = {
    engine,
    planet,
    personal:
      planet.stage === "owned"
        ? {
            world: "creator-worlds",
            name: textOf("#option-creator-worlds strong") || "My planet",
            category: "Your personal planet",
            description:
              "Your private place in the Verse. Choose a name and make it your own.",
          }
        : undefined,
  };
  return visit
    ? {
        ...report,
        entered: visit.entered,
        exploring: visit.exploring,
        ...(visit.exploring && visit.selected && { selected: visit.selected }),
      }
    : report;
}

const click = (id: string) => {
  const node = byId(id);
  if (node && !(node as HTMLButtonElement).disabled) node.click();
};

export function mountVerse(): (() => void) | undefined {
  const config = readConfig();
  const mount = byId("verse-hud");
  const exploration = byId("universe-exploration");
  const legacy = byId("verse-entry") as HTMLDialogElement | null;
  if (!config || !mount || !exploration) return undefined;

  // The engine release opens its own entry dialog as a modal, which would
  // make this interface inert. Keep that dialog closed; the controller's
  // dialog replaces it.
  if (legacy) {
    legacy.showModal = () => {};
    if (legacy.open) legacy.close();
  }
  document.documentElement.dataset.aethyrVerse = "on";

  const registry = AtomRegistry.make();
  const sdk = () => window.universeDemo !== undefined;
  const runtime = ManagedRuntime.make(
    Layer.mergeAll(
      Layer.succeed(AtomRegistry.AtomRegistry, registry),
      Layer.succeed(EntryOptions, {
        accounts: config.accounts,
        hauntPath: config.hauntPath ?? undefined,
      }),
      Layer.succeed(VerseOptions, {
        worlds: config.worlds,
        signedIn: config.signedIn,
        blueRushPath: config.blueRushPath,
        mapPath: config.mapPath,
      }),
      Layer.succeed(Navigator, {
        go: (path: string) => Effect.sync(() => window.location.assign(path)),
      }),
      Layer.succeed(GuestMemory, {
        remember: Effect.sync(() => click("continue-guest")),
      }),
      Layer.succeed(UniverseEngine, {
        explore: Effect.sync(() => click("verse-explore")),
        select: (world: string) => Effect.sync(() => click(`option-${world}`)),
        // The engine opens a sign-in window and stays on the map; without
        // it, the bridge's link navigates.
        signIn: (path: string) =>
          Effect.sync(() => {
            const link = byId("verse-signin");
            if (link) link.click();
            else window.location.assign(path);
          }),
        createPlanet: Effect.sync(() => click("planet-create")),
        reload: Effect.sync(() => {
          if (!sdk()) return false;
          click("world-retry");
          return true;
        }),
        fullscreen: Effect.sync(() => {
          void document.documentElement.requestFullscreen?.().catch(() => {});
        }),
      }),
    ),
  );

  const start = initialVerse(
    { signedIn: config.signedIn, worlds: config.worlds },
    remembered(),
  );
  const controller = runtime.runSync(makeVerseController(start));

  // The engine stays inert until the visitor chooses to explore. Focus
  // follows the step: the next choice, then the selected world.
  let step = registry.get(controller.state).entry;
  const syncInert = () => {
    const state = registry.get(controller.state);
    const closed = state.entry === "closed";
    if (closed) exploration.removeAttribute("inert");
    else exploration.setAttribute("inert", "");
    if (state.entry === step) return;
    step = state.entry;
    const target =
      state.entry === "choice"
        ? "verse-explore"
        : state.entry === "closed"
          ? `option-${state.selected}`
          : undefined;
    if (target)
      requestAnimationFrame(() =>
        mount
          .querySelector<HTMLElement>(`[data-aethyr-id="${target}"]`)
          ?.focus({ preventScroll: true }),
      );
  };
  syncInert();
  const unsubscribe = registry.subscribe(controller.state, syncInert);

  // Once the engine mounts, give it the choices made before it arrived.
  let reconciled = false;
  const reconcile = () => {
    const visit = parseVisit(window.universeDemo?.visit());
    if (!visit) return;
    reconciled = true;
    const state = registry.get(controller.state);
    if (state.entry !== "account" && !visit.entered) click("continue-guest");
    if (state.entry === "closed" && !visit.exploring) click("verse-explore");
    if (state.entry === "closed" && visit.selected !== state.selected)
      click(`option-${state.selected}`);
  };

  let frame = 0;
  const refresh = () => {
    frame = 0;
    if (sdk() && !reconciled) reconcile();
    const report = readEngine();
    runtime.runFork(controller.update((state) => engineReport(state, report)));
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(refresh);
  };
  const observer = new MutationObserver(schedule);
  for (const target of [
    byId("verse-bridge"),
    byId("world-notice"),
    exploration,
  ])
    if (target)
      observer.observe(target, {
        subtree: true,
        childList: true,
        characterData: true,
        attributes: true,
        attributeFilter: ["hidden", "disabled", "href", "aria-pressed", "inert", "data-state"],
      });
  // The engine mounts after a network import; look for it until it does.
  const waiting = window.setInterval(() => {
    if (sdk() || enginePhase() === "error") {
      window.clearInterval(waiting);
      schedule();
    }
  }, 200);
  refresh();

  const root = createRoot(mount);
  root.render(
    <RegistryContext.Provider value={registry}>
      <ControllerView
        controller={controller}
        run={(effect) => runtime.runPromiseExit(effect)}
      />
    </RegistryContext.Provider>,
  );

  return () => {
    window.clearInterval(waiting);
    cancelAnimationFrame(frame);
    observer.disconnect();
    unsubscribe();
    root.unmount();
    void runtime.dispose();
    registry.dispose();
    delete document.documentElement.dataset.aethyrVerse;
  };
}
