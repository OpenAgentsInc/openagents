// The /new-lander/verse interface as an Aethyr controller (#12588): the
// entry dialog, the destination sheet with its world chips, the map
// controls, and the fallback links. It owns what the visitor sees. The
// planets themselves belong to the engine, reached through `UniverseEngine`;
// the engine reports back through `update` (see `engineReport`).
import { Context, Effect } from "effect";
import * as Controller from "./controller";
import {
  EntryOptions,
  GuestMemory,
  Navigator,
  PATHS,
  renderEntry,
  type EntryStep,
} from "./entry";
import type { Node } from "./protocol";

export { EntryOptions, GuestMemory, Navigator, PATHS };

/** One destination on the map. Names come from the server's world list. */
export interface World {
  readonly id: string;
  readonly name: string;
  readonly category: string;
  readonly description: string;
}

/** What the server knows when the page loads. */
export class VerseOptions extends Context.Service<
  VerseOptions,
  {
    readonly worlds: ReadonlyArray<World>;
    /** The account recognised this visitor; the dialog starts at the choice. */
    readonly signedIn: boolean;
    readonly blueRushPath: string;
    /** The page itself, for reloading the map without the engine. */
    readonly mapPath: string;
  }
>()("aethyr/VerseOptions") {}

/**
 * The planets. The web host drives the pinned engine release; tests record
 * calls. Every method is fire-and-forget: the engine reports what actually
 * happened through `engineReport`.
 */
export class UniverseEngine extends Context.Service<
  UniverseEngine,
  {
    readonly explore: Effect.Effect<void>;
    readonly select: (world: string) => Effect.Effect<void>;
    /** Sign in without leaving the map when the engine can; else navigate. */
    readonly signIn: (path: string) => Effect.Effect<void>;
    readonly createPlanet: Effect.Effect<void>;
    /** Reopen the 3D view; `false` when the host should reload the page. */
    readonly reload: Effect.Effect<boolean>;
    readonly fullscreen: Effect.Effect<void>;
  }
>()("aethyr/UniverseEngine") {}

export type EnginePhase = "opening" | "open" | "error";

/** The personal planet, as the engine's tutorial service reports it. */
export interface Planet {
  /** Ready to create, already owned, or neither yet. */
  readonly stage: "none" | "ready" | "owned";
  readonly heading: string;
  readonly status: string;
  readonly canCreate: boolean;
  /** Where to set it up, when it can be opened now. */
  readonly setupPath: string | undefined;
}

/** Text the engine chose for the selected world (a personal planet's name). */
export interface WorldText {
  readonly world: string;
  readonly name: string;
  readonly category: string;
  readonly description: string;
}

export interface VerseState {
  readonly entry: EntryStep;
  readonly selected: string;
  readonly engine: EnginePhase;
  readonly planet: Planet;
  /** Overrides for one world's text and chip, from the engine. */
  readonly personal: WorldText | undefined;
}

export const NO_PLANET: Planet = {
  stage: "none",
  heading: "",
  status: "Publish your own house, choose Play or Share, then return here.",
  canCreate: false,
  setupPath: undefined,
};

/** What the engine host observed; each field it knows replaces the state's. */
export interface EngineReport {
  readonly entered?: boolean;
  readonly exploring?: boolean;
  readonly selected?: string;
  readonly engine?: EnginePhase;
  readonly planet?: Planet;
  readonly personal?: WorldText | undefined;
}

/** Fold an engine report into the state. The engine never reopens a closed dialog. */
export function engineReport(state: VerseState, report: EngineReport): VerseState {
  let entry = state.entry;
  if (report.exploring) entry = "closed";
  else if (report.entered && entry === "account") entry = "choice";
  return {
    entry,
    selected: report.selected ?? state.selected,
    engine: report.engine ?? state.engine,
    planet: report.planet ?? state.planet,
    personal: "personal" in report ? report.personal : state.personal,
  };
}

export const initialVerse = (
  options: { readonly signedIn: boolean; readonly worlds: ReadonlyArray<World> },
  remembered?: {
    readonly entered: boolean;
    readonly exploring: boolean;
    readonly selected: string;
  },
): VerseState => {
  const known = (id: string | undefined) =>
    options.worlds.some((w) => w.id === id) ? id : undefined;
  const entry: EntryStep = remembered?.exploring
    ? "closed"
    : options.signedIn || remembered?.entered
      ? "choice"
      : "account";
  return {
    entry,
    selected:
      known(remembered?.selected) ?? options.worlds[0]?.id ?? "openagents",
    engine: "opening",
    planet: NO_PLANET,
    personal: undefined,
  };
};

const text = (
  id: string,
  value: string,
  role: "title" | "body" | "caption" | "heading" | "status" = "body",
  hidePhone = false,
): Node => ({
  id,
  ...(hidePhone && { style: { hide_on_phone: true } }),
  element: { kind: "text", value, role },
});
const link = (
  id: string,
  label: string,
  href: string,
  emphasis: "primary" | "secondary" | "link" | "ghost" = "link",
): Node => ({
  id,
  element: { kind: "button", label, href, emphasis, trailing: "forward" },
});
const column = (id: string, children: Node[], gap: "xs" | "sm" | "md" = "sm"): Node => ({
  id,
  style: { gap },
  element: { kind: "stack", axis: "vertical", children },
});
const lockedLabel = (id: string, value: string): Node => ({
  id,
  style: { tone: "muted" },
  element: { kind: "label", value },
});

export interface VerseView {
  readonly accounts: boolean;
  readonly hauntPath: string | undefined;
  readonly worlds: ReadonlyArray<World>;
  readonly blueRushPath: string;
  readonly mapPath: string;
}

function destination(state: VerseState, options: VerseView): Node {
  const id = state.selected;
  const children: Node[] = [];
  switch (id) {
    case "openagents":
      children.push(
        lockedLabel("openagents-locked", "◇ Locked for now"),
        link("openagents-about", "About OpenAgents", "/docs"),
      );
      break;
    case "blue-rush":
      children.push(
        link("blue-rush-meet", "Meet Blue Rush", options.blueRushPath, "primary"),
      );
      break;
    case "haunted-house":
      children.push(
        options.hauntPath
          ? link("haunted-house-enter", "Enter Haunted House", options.hauntPath, "primary")
          : text("haunted-house-closed", "The house is not open here yet.", "caption"),
      );
      break;
    case "creator-worlds": {
      const planet = state.planet;
      children.push(text("planet-status", planet.status, "status"));
      if (planet.stage === "ready")
        children.push(
          column("planet-ready", [
            text("planet-ready-title", "A place of your own", "heading"),
            text(
              "planet-ready-body",
              "Your house is published. Create your first personal planet.",
              "caption",
            ),
            text(
              "planet-ready-fine",
              "64 × 64 metres · Up to 128 objects · Private to you. Uses one account project; your account's project limit applies.",
              "caption",
            ),
            {
              id: "planet-create",
              element: {
                kind: "button",
                label: "Create my planet",
                emphasis: "primary",
                enabled: planet.canCreate,
              },
            },
          ]),
        );
      if (planet.stage === "owned")
        children.push(
          column("planet-owned", [
            text("planet-owned-title", planet.heading || "Your planet", "heading"),
            {
              id: "planet-setup",
              element: {
                kind: "button",
                label: "Set up my planet",
                emphasis: "primary",
                trailing: "forward",
                ...(planet.setupPath
                  ? { href: planet.setupPath }
                  : { enabled: false }),
              },
            },
          ]),
        );
      break;
    }
    default:
      children.push(lockedLabel(`${id}-locked`, "◇ Still taking shape"));
  }
  return column(`destination-${id}`, children);
}

/** Text for one world, with the engine's personal override applied. */
function worldText(state: VerseState, world: World) {
  const personal =
    state.personal?.world === world.id ? state.personal : undefined;
  return {
    name: personal?.name ?? world.name,
    category: personal?.category ?? world.category,
    description: personal?.description ?? world.description,
  };
}

export function renderVerse(state: VerseState, options: VerseView): Node {
  const world =
    options.worlds.find((w) => w.id === state.selected) ?? options.worlds[0]!;
  const shown = worldText(state, world);
  const exploring = state.entry === "closed";
  const panel: Node = {
    id: "verse-panel",
    element: {
      kind: "sheet",
      label: "Destination",
      open: exploring,
      children: [
        {
          id: "verse-panel-head",
          style: { gap: "xs" },
          element: {
            kind: "stack",
            axis: "vertical",
            children: [
              { id: "world-category", element: { kind: "label", value: shown.category } },
              text("world-title", shown.name, "title"),
            ],
          },
        },
        text("world-description", shown.description, "caption"),
        destination(state, options),
        {
          id: "verse-worlds",
          style: { gap: "sm" },
          a11y: { role: "group", label: "Choose a destination" },
          element: {
            kind: "stack",
            axis: "wrap",
            children: options.worlds.map((w): Node => {
              const t = worldText(state, w);
              return {
                id: `option-${w.id}`,
                element: {
                  kind: "chip",
                  label: t.name,
                  detail: t.category,
                  selected: w.id === state.selected,
                },
              };
            }),
          },
        },
        ...(state.engine === "open"
          ? []
          : [
              {
                id: "verse-fallback",
                style: { gap: "md" },
                a11y: { role: "navigation", label: "Other ways to explore" },
                element: {
                  kind: "stack",
                  axis: "wrap",
                  children: [
                    {
                      id: "fallback-retry",
                      element: {
                        kind: "button",
                        label: "Try the 3D view again",
                        emphasis: "link",
                        href: options.mapPath,
                      },
                    },
                    {
                      id: "fallback-home",
                      element: {
                        kind: "button",
                        label: "Back to OpenAgents",
                        emphasis: "link",
                        href: PATHS.home,
                      },
                    },
                    ...(options.hauntPath
                      ? [
                          {
                            id: "fallback-haunt",
                            // The sheet already offers these on a phone.
                            style: { hide_on_phone: true },
                            element: {
                              kind: "button",
                              label: "Enter Haunted House",
                              emphasis: "link",
                              href: options.hauntPath,
                            },
                          } satisfies Node,
                        ]
                      : []),
                    {
                      id: "fallback-blue-rush",
                      style: { hide_on_phone: true },
                      element: {
                        kind: "button",
                        label: "Meet Blue Rush",
                        emphasis: "link",
                        href: options.blueRushPath,
                      },
                    },
                  ],
                },
              } satisfies Node,
            ]),
      ],
    },
  };
  const controls: Node = {
    id: "verse-controls",
    style: { gap: "sm", hide_on_phone: true },
    element: {
      kind: "stack",
      axis: "horizontal",
      children: exploring
        ? [
            {
              id: "universe-status",
              element: {
                kind: "label",
                value: "Drag to orbit · Scroll to zoom · Select a world",
              },
            },
            {
              id: "world-retry",
              element: {
                kind: "button",
                label: "Reload map",
                emphasis: "ghost",
                href: options.mapPath,
              },
            },
            {
              id: "world-fullscreen",
              element: { kind: "button", label: "Full screen", emphasis: "ghost" },
            },
          ]
        : [],
    },
  };
  return {
    id: "verse-hud",
    element: {
      kind: "stack",
      axis: "vertical",
      children: [
        panel,
        controls,
        renderEntry(
          { step: state.entry },
          { accounts: options.accounts, hauntPath: options.hauntPath },
        ),
      ],
    },
  };
}

const LINKS: Record<string, (o: VerseView) => string | undefined> = {
  "openagents-about": () => "/docs",
  "blue-rush-meet": (o) => o.blueRushPath,
  "fallback-blue-rush": (o) => o.blueRushPath,
  "haunted-house-enter": (o) => o.hauntPath,
  "fallback-haunt": (o) => o.hauntPath,
  "verse-haunt": (o) => o.hauntPath,
  "fallback-retry": (o) => o.mapPath,
  "fallback-home": () => PATHS.home,
  "verse-entry-back": () => PATHS.home,
};

/** Build the Verse controller from the services in scope. */
export const makeVerseController = (
  initial: VerseState,
  instance = `verse-${Date.now().toString(36)}`,
) =>
  Effect.gen(function* () {
    const entry = yield* EntryOptions;
    const verse = yield* VerseOptions;
    const navigator = yield* Navigator;
    const guest = yield* GuestMemory;
    const engine = yield* UniverseEngine;
    const options: VerseView = {
      accounts: entry.accounts,
      hauntPath: entry.hauntPath,
      worlds: verse.worlds,
      blueRushPath: verse.blueRushPath,
      mapPath: verse.mapPath,
    };
    return yield* Controller.make<VerseState>({
      instance,
      initial,
      render: (state) => renderVerse(state, options),
      handle: (state, intent, node) =>
        Effect.gen(function* () {
          const id = intent.node;
          if (id.startsWith("option-")) {
            const world = id.slice("option-".length);
            if (!options.worlds.some((w) => w.id === world)) return state;
            yield* engine.select(world);
            return { ...state, selected: world };
          }
          const path = LINKS[id]?.(options);
          if (path) {
            yield* navigator.go(path);
            return state;
          }
          switch (id) {
            case "continue-guest":
              yield* guest.remember;
              return { ...state, entry: "choice" } as const;
            case "verse-explore":
              yield* engine.explore;
              return { ...state, entry: "closed" } as const;
            case "verse-signin":
              yield* engine.signIn(PATHS.signIn);
              return state;
            case "planet-create":
              yield* engine.createPlanet;
              return {
                ...state,
                planet: {
                  ...state.planet,
                  canCreate: false,
                  status: "Creating your planet…",
                },
              };
            case "planet-setup":
              if (state.planet.setupPath) yield* navigator.go(state.planet.setupPath);
              return state;
            case "world-retry":
              if (!(yield* engine.reload)) yield* navigator.go(options.mapPath);
              return { ...state, engine: "opening" } as const;
            case "world-fullscreen":
              yield* engine.fullscreen;
              return state;
            default:
              void node;
              return state;
          }
        }),
    });
  });
