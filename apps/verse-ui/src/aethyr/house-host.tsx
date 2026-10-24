// The web host for the house controllers (`house.ts`) on the planet setup,
// invitation, and player pages.
//
// Each server page keeps its own markup in `.house-bridge`: the page for
// visitors without JavaScript, and the form or controls the server and the
// pinned engine release (`aethyrforge-host`) expect by ID. Once this host
// mounts it hides the bridge (`data-aethyr-house` on the root), draws the
// controller's tree into `#house-hud`, submits the bridge's forms, clicks its
// controls, and folds what the release writes there back into the controller.
import { createRoot } from "react-dom/client";
import { Effect, Fiber, Layer, ManagedRuntime } from "effect";
import * as AtomRegistry from "effect/reactivity/AtomRegistry";
import { RegistryContext } from "@effect/atom-react";
import { ControllerView } from "./host";
import type { Controller, InvalidView } from "./controller";
import {
  InviteForm,
  Navigator,
  PlanetForm,
  PlayerEngine,
  VisitCheck,
  initialInvite,
  initialSetup,
  makeInviteController,
  makePlayerController,
  makeSetupController,
  playerReport,
  watchVisit,
  type Choice,
  type Invitation,
  type PlayerReport,
  type PlayerState,
  type VisitStatus,
} from "./house";

export type HouseSurface = "setup" | "invite" | "player";

const byId = (id: string) => document.getElementById(id);
const textOf = (node: Element | null | undefined) =>
  node?.textContent?.trim() ?? "";
const navigator = Layer.succeed(Navigator, {
  go: (path: string) => Effect.sync(() => window.location.assign(path)),
});

/** Submit a server form as its own submit button would. */
function submit(form: HTMLFormElement | null | undefined) {
  if (!form) return;
  const button = form.querySelector<HTMLButtonElement>("button[type=submit]");
  form.requestSubmit(button ?? undefined);
}

const optionsOf = (select: HTMLSelectElement | null): Choice[] =>
  select
    ? Array.from(select.options, (option) => ({
        value: option.value,
        label: textOf(option),
      }))
    : [];

function setupController(bridge: HTMLElement) {
  const input = bridge.querySelector<HTMLInputElement>("#planet-name");
  if (!input) return undefined;
  const enter = bridge.querySelector<HTMLAnchorElement>("#planet-enter");
  const layer = Layer.succeed(PlanetForm, {
    save: (name: string) =>
      Effect.sync(() => {
        input.value = name;
        submit(input.form);
      }),
  });
  const initial = initialSetup(
    input.value || textOf(bridge.querySelector("h1")),
    enter?.getAttribute("href") ?? undefined,
  );
  return { layer, make: makeSetupController(initial) };
}

function inviteController(bridge: HTMLElement) {
  const form = bridge.querySelector("form");
  const access =
    form?.querySelector<HTMLSelectElement>("select[name=access]") ?? null;
  const hours =
    form?.querySelector<HTMLSelectElement>("select[name=hours]") ?? null;
  if (!form || !access || !hours) return undefined;
  const invitations: Invitation[] = Array.from(
    bridge.querySelectorAll<HTMLLIElement>(".invitation"),
    (row) => ({
      link: row.querySelector("a")?.getAttribute("href") ?? "",
      state: row.dataset.state ?? "",
      access: row.dataset.access ?? "",
      expiry: row.dataset.expiry ?? "",
      end:
        row.querySelector<HTMLInputElement>(".invitation-end input[name=id]")
          ?.value || undefined,
    }),
  ).filter((row) => row.link.startsWith("/"));
  const layer = Layer.succeed(InviteForm, {
    create: (chosenAccess: string, chosenHours: string) =>
      Effect.sync(() => {
        access.value = chosenAccess;
        hours.value = chosenHours;
        submit(form);
      }),
    end: (id: string) =>
      Effect.sync(() => {
        const input = Array.from(
          bridge.querySelectorAll<HTMLInputElement>(
            ".invitation-end input[name=id]",
          ),
        ).find((field) => field.value === id);
        submit(input?.form);
      }),
  });
  const initial = initialInvite({
    version: bridge.dataset.version ?? "",
    intro: textOf(bridge.querySelector(".invite-intro")),
    accessOptions: optionsOf(access),
    hoursOptions: optionsOf(hours),
    invitations,
  });
  return { layer, make: makeInviteController(initial) };
}

const shown = (id: string) => {
  const node = byId(id);
  return node !== null && !node.hidden;
};
const readPlayer = (): PlayerReport => ({
  retry: shown("world-retry"),
  fullscreen: shown("world-fullscreen"),
});
// The release listens on its own controls; a click from the visitor's
// gesture keeps the activation full screen needs.
const click = (id: string) => Effect.sync(() => byId(id)?.click());

declare global {
  interface Window {
    versePlayer?: { readonly close?: () => void };
  }
}

/** Close the pinned player and show why, as the release does for an error. */
const closePlayer = (message: string) =>
  Effect.sync(() => {
    window.versePlayer?.close?.();
    for (const id of ["engine", "world-retry", "world-fullscreen"])
      byId(id)?.setAttribute("hidden", "");
    const notice = byId("world-notice");
    if (notice) {
      notice.textContent = message;
      notice.hidden = false;
    }
  });

/**
 * An invitation the server no longer serves answers 410; one that needs
 * sign-in answers 401 once the visitor has signed out.
 */
const visitStatus = (id: string | undefined) =>
  Effect.promise(async (): Promise<VisitStatus> => {
    if (!id) return "open";
    try {
      const response = await fetch(
        `/v1/verse/studio/visits/${encodeURIComponent(id)}`,
        { credentials: "same-origin", cache: "no-store" },
      );
      if (response.status === 410) return "ended";
      if (response.status === 401) return "signed_out";
      return "open";
    } catch {
      // Offline for a moment is not an ended invitation.
      return "open";
    }
  });

function playerController(bridge: HTMLElement) {
  const layer = Layer.mergeAll(
    Layer.succeed(PlayerEngine, {
      retry: click("world-retry"),
      fullscreen: click("world-fullscreen"),
      close: closePlayer,
    }),
    Layer.succeed(VisitCheck, { status: visitStatus(bridge.dataset.visit) }),
  );
  const initial = {
    title: textOf(bridge.querySelector("h1")),
    version: bridge.dataset.version ?? "",
    ended: undefined,
    ...readPlayer(),
  };
  return { layer, make: makePlayerController(initial) };
}

/** Run one controller in `mount` with the page's services. */
function run<S, Svc>(
  mount: HTMLElement,
  layer: Layer.Layer<Svc>,
  make: Effect.Effect<
    Controller<S, never, Svc | Navigator>,
    InvalidView,
    Svc | Navigator | AtomRegistry.AtomRegistry
  >,
  watch?: (
    controller: Controller<S, never, Svc | Navigator>,
    runtime: ManagedRuntime.ManagedRuntime<
      Svc | Navigator | AtomRegistry.AtomRegistry,
      never
    >,
  ) => () => void,
) {
  const registry = AtomRegistry.make();
  const runtime = ManagedRuntime.make(
    Layer.mergeAll(
      Layer.succeed(AtomRegistry.AtomRegistry, registry),
      navigator,
      layer,
    ),
  );
  const controller = runtime.runSync(make);
  const unwatch = watch?.(controller, runtime);
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
    unwatch?.();
    root.unmount();
    void runtime.dispose();
    registry.dispose();
  };
}

/**
 * The release shows and hides its player controls; follow it. On a visit,
 * also check that its invitation still stands.
 */
const watchPlayer =
  (bridge: HTMLElement) =>
  (
    controller: Pick<Controller<PlayerState>, "update">,
    runtime: ManagedRuntime.ManagedRuntime<
      PlayerEngine | VisitCheck | Navigator | AtomRegistry.AtomRegistry,
      never
    >,
  ) => {
    let frame = 0;
    const observer = new MutationObserver(() => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        const report = readPlayer();
        runtime.runFork(
          controller.update((state) => playerReport(state, report)),
        );
      });
    });
    observer.observe(bridge, {
      subtree: true,
      attributes: true,
      attributeFilter: ["hidden"],
    });
    const visit = bridge.dataset.visit
      ? runtime.runFork(watchVisit(controller))
      : undefined;
    return () => {
      observer.disconnect();
      cancelAnimationFrame(frame);
      if (visit) runtime.runFork(Fiber.interrupt(visit));
    };
  };

export function mountHouse(surface: HouseSurface): (() => void) | undefined {
  const mount = byId("house-hud");
  const bridge = document.querySelector<HTMLElement>(".house-bridge");
  if (!mount || !bridge) return undefined;
  let dispose: (() => void) | undefined;
  if (surface === "setup") {
    const parts = setupController(bridge);
    if (parts) dispose = run(mount, parts.layer, parts.make);
  } else if (surface === "invite") {
    const parts = inviteController(bridge);
    if (parts) dispose = run(mount, parts.layer, parts.make);
  } else {
    const parts = playerController(bridge);
    dispose = run(mount, parts.layer, parts.make, watchPlayer(bridge));
  }
  if (!dispose) return undefined;
  document.documentElement.dataset.aethyrHouse = surface;
  return () => {
    dispose();
    delete document.documentElement.dataset.aethyrHouse;
  };
}
