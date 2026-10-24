// The Haunted House's form surfaces as Aethyr controllers (#12588): planet
// setup (`/new-lander/planet/{id}`), invitations
// (`/new-lander/invite/{world}/{release}`), and the player's top bar
// (`/new-lander/play/...` and `/new-lander/visit/...`). Visits have no chat.
// Each owns what the
// visitor sees. Saving, inviting, and the engine stay with the server and
// the pinned release, reached through the services below; the web host
// (`house-host.tsx`) drives the server's own forms and controls.
import { Context, Effect } from "effect";
import * as Controller from "./controller";
import { Navigator } from "./entry";
import type { Node } from "./protocol";

export { Navigator };

/** The server's planet-name form. Submitting leaves the page. */
export class PlanetForm extends Context.Service<
  PlanetForm,
  { readonly save: (name: string) => Effect.Effect<void> }
>()("aethyr/PlanetForm") {}

/** The server's invitation form. Submitting leaves the page. */
export class InviteForm extends Context.Service<
  InviteForm,
  {
    readonly create: (access: string, hours: string) => Effect.Effect<void>;
    /** End one invitation by ID. */
    readonly end: (id: string) => Effect.Effect<void>;
  }
>()("aethyr/InviteForm") {}

/** The pinned player's own controls. */
export class PlayerEngine extends Context.Service<
  PlayerEngine,
  {
    readonly retry: Effect.Effect<void>;
    readonly fullscreen: Effect.Effect<void>;
    /** Close the player and say why, after its invitation ended. */
    readonly close: (message: string) => Effect.Effect<void>;
  }
>()("aethyr/PlayerEngine") {}

/** Whether a visit may go on: its invitation stands and, if it needs one, the sign-in. */
export type VisitStatus = "open" | "ended" | "signed_out";
export class VisitCheck extends Context.Service<
  VisitCheck,
  { readonly status: Effect.Effect<VisitStatus> }
>()("aethyr/VisitCheck") {}

export const MAP_PATH = "/new-lander/verse";
/** The server's limit on a planet name, in characters. */
export const NAME_LIMIT = 80;

const text = (
  id: string,
  value: string,
  role: "title" | "body" | "caption" | "heading" | "status" = "body",
): Node => ({ id, element: { kind: "text", value, role } });
const label = (id: string, value: string, hidePhone = false): Node => ({
  id,
  ...(hidePhone && { style: { hide_on_phone: true } }),
  element: { kind: "label", value },
});
const column = (
  id: string,
  children: Node[],
  gap: "xs" | "sm" | "md" = "sm",
): Node => ({
  id,
  style: { gap },
  element: { kind: "stack", axis: "vertical", children },
});
const back = (id: string): Node => ({
  id,
  element: {
    kind: "button",
    label: "Back to planets",
    href: MAP_PATH,
    emphasis: "link",
    leading: "back",
  },
});

/** A link the controller follows; every other control is the surface's own. */
const hrefOf = (node: Node) =>
  node.element.kind === "button" ? node.element.href : undefined;

// --- Planet setup ---------------------------------------------------------

export interface SetupState {
  /** The saved name, as the server last drew it. */
  readonly name: string;
  readonly draft: string;
  /** The courtyard, when this site serves it. */
  readonly enterPath: string | undefined;
  readonly saving: boolean;
}

export const initialSetup = (name: string, enterPath?: string): SetupState => ({
  name,
  draft: name,
  enterPath,
  saving: false,
});

const nameProblem = (draft: string) => {
  if (draft.trim() === "") return "Enter a name for your planet.";
  if ([...draft].length > NAME_LIMIT)
    return `Use ${NAME_LIMIT} characters or fewer.`;
  return undefined;
};

export function renderSetup(state: SetupState): Node {
  const problem = nameProblem(state.draft);
  const children: Node[] = [
    back("setup-back"),
    label("setup-eyebrow", "Your planet"),
    text("setup-title", state.name, "title"),
    text("setup-intro", "Choose a name for your place in the Verse."),
    {
      id: "planet-name",
      element: {
        kind: "field",
        label: "Planet name",
        value: state.draft,
        // Each character can take up to four bytes.
        max_bytes: NAME_LIMIT * 4,
        enabled: !state.saving,
      },
    },
    {
      id: "planet-save",
      element: {
        kind: "button",
        label: state.saving ? "Saving…" : "Save name",
        emphasis: "primary",
        enabled: !state.saving && problem === undefined,
      },
    },
  ];
  if (problem && state.draft !== "")
    children.push({
      id: "setup-problem",
      style: { tone: "danger" },
      element: { kind: "text", value: problem, role: "status" },
    });
  children.push(
    text(
      "setup-limits",
      "64 × 64 metres · Up to 128 objects · Private to you",
      "caption",
    ),
  );
  if (state.enterPath)
    children.push({
      id: "planet-enter",
      element: {
        kind: "button",
        label: "Enter your courtyard",
        href: state.enterPath,
        emphasis: "secondary",
        trailing: "forward",
      },
    });
  return {
    id: "setup",
    style: { gap: "md" },
    a11y: { role: "region", label: "Your planet" },
    element: { kind: "panel", children },
  };
}

export const makeSetupController = (
  initial: SetupState,
  instance = `setup-${Date.now().toString(36)}`,
) =>
  Effect.gen(function* () {
    const navigator = yield* Navigator;
    const form = yield* PlanetForm;
    return yield* Controller.make<SetupState>({
      instance,
      initial,
      render: renderSetup,
      handle: (state, intent, node) =>
        Effect.gen(function* () {
          if (intent.node === "planet-name") {
            if (intent.kind === "input")
              return { ...state, draft: intent.value ?? "" };
            if (intent.kind !== "submit") return state;
          } else if (intent.node !== "planet-save") {
            const href = hrefOf(node);
            if (href) yield* navigator.go(href);
            return state;
          }
          if (state.saving || nameProblem(state.draft)) return state;
          yield* form.save(state.draft.trim());
          return { ...state, saving: true };
        }),
    });
  });

// --- Invitations ----------------------------------------------------------

export interface Choice {
  readonly value: string;
  readonly label: string;
}

/** One invitation, as the server listed it. */
export interface Invitation {
  readonly link: string;
  /** "Open", "Ended", or "Expired". */
  readonly state: string;
  readonly access: string;
  /** Empty once it has ended. */
  readonly expiry: string;
  /** The invitation's ID while the host can still end it. */
  readonly end: string | undefined;
}

export interface InviteState {
  readonly version: string;
  readonly intro: string;
  readonly accessOptions: ReadonlyArray<Choice>;
  readonly hoursOptions: ReadonlyArray<Choice>;
  readonly access: string;
  readonly hours: string;
  readonly invitations: ReadonlyArray<Invitation>;
  /** Creating or ending an invitation; the page is about to reload. */
  readonly creating: boolean;
}

export function initialInvite(
  page: Omit<InviteState, "access" | "hours" | "creating">,
): InviteState {
  return {
    ...page,
    access: page.accessOptions[0]?.value ?? "",
    hours: page.hoursOptions[0]?.value ?? "",
    creating: false,
  };
}

const choices = (
  id: string,
  prefix: string,
  title: string,
  options: ReadonlyArray<Choice>,
  selected: string,
  enabled: boolean,
): Node =>
  column(
    id,
    [
      label(`${id}-label`, title),
      {
        id: `${id}-choices`,
        style: { gap: "xs" },
        a11y: { role: "group", label: title },
        element: {
          kind: "stack",
          axis: "wrap",
          children: options.map((option) => ({
            id: `${prefix}-${option.value}`,
            element: {
              kind: "chip",
              label: option.label,
              selected: option.value === selected,
              enabled,
            },
          })),
        },
      },
    ],
    "xs",
  );

export function renderInvite(state: InviteState): Node {
  const children: Node[] = [
    back("invite-back"),
    label("invite-version", `Version ${state.version}`),
    text("invite-title", "Invite people", "title"),
    text("invite-intro", state.intro),
    choices(
      "invite-access",
      "access",
      "Who can visit?",
      state.accessOptions,
      state.access,
      !state.creating,
    ),
    choices(
      "invite-hours",
      "hours",
      "Link expires",
      state.hoursOptions,
      state.hours,
      !state.creating,
    ),
    {
      id: "invite-create",
      element: {
        kind: "button",
        label: state.creating ? "Creating…" : "Create invitation",
        emphasis: "primary",
        enabled: !state.creating && state.access !== "" && state.hours !== "",
      },
    },
  ];
  if (state.invitations.length > 0)
    children.push({
      id: "invitations",
      element: {
        kind: "list",
        label: "Invitations",
        children: state.invitations.map((row, index) =>
          column(
            `invitation-${index}`,
            [
              {
                id: `invitation-${index}-open`,
                element: {
                  kind: "button",
                  label: "Open invitation",
                  href: row.link,
                  emphasis: "link",
                  trailing: "forward",
                },
              },
              text(
                `invitation-${index}-detail`,
                [row.state, row.access, row.expiry]
                  .filter((part) => part !== "")
                  .join(" · "),
                "caption",
              ),
              ...(row.end === undefined
                ? []
                : [
                    {
                      id: `invitation-${index}-end`,
                      element: {
                        kind: "button",
                        label: "End invitation",
                        emphasis: "ghost",
                        enabled: !state.creating,
                      },
                    } satisfies Node,
                  ]),
            ],
            "xs",
          ),
        ),
      },
    });
  else
    children.push(text("invitations-empty", "No invitations yet.", "caption"));
  return {
    id: "invite",
    style: { gap: "md" },
    a11y: { role: "region", label: "Invite people" },
    element: { kind: "panel", children },
  };
}

export const makeInviteController = (
  initial: InviteState,
  instance = `invite-${Date.now().toString(36)}`,
) =>
  Effect.gen(function* () {
    const navigator = yield* Navigator;
    const form = yield* InviteForm;
    return yield* Controller.make<InviteState>({
      instance,
      initial,
      render: renderInvite,
      handle: (state, intent, node) =>
        Effect.gen(function* () {
          const id = intent.node;
          const pick = (prefix: string, options: ReadonlyArray<Choice>) => {
            const value = id.startsWith(`${prefix}-`)
              ? id.slice(prefix.length + 1)
              : undefined;
            return options.some((o) => o.value === value) ? value : undefined;
          };
          const access = pick("access", state.accessOptions);
          if (access) return { ...state, access };
          const hours = pick("hours", state.hoursOptions);
          if (hours) return { ...state, hours };
          if (id === "invite-create") {
            if (state.creating) return state;
            yield* form.create(state.access, state.hours);
            return { ...state, creating: true };
          }
          const ending = /^invitation-(\d+)-end$/.exec(id);
          if (ending) {
            const end = state.invitations[Number(ending[1])]?.end;
            if (state.creating || end === undefined) return state;
            yield* form.end(end);
            return { ...state, creating: true };
          }
          const href = hrefOf(node);
          if (href) yield* navigator.go(href);
          return state;
        }),
    });
  });

// --- Player bar -------------------------------------------------------------

export interface PlayerState {
  readonly title: string;
  readonly version: string;
  /** What the pinned player shows of its own controls. */
  readonly retry: boolean;
  readonly fullscreen: boolean;
  /** Why the visit closed while it was open; the release's notice shows it. */
  readonly ended: string | undefined;
}

export const ENDED = "This invitation has ended. Ask the host for a new link.";
export const SIGNED_OUT = "You signed out. Sign in again to keep visiting.";

/** What the player host observed on the engine's controls. */
export interface PlayerReport {
  readonly retry: boolean;
  readonly fullscreen: boolean;
}

export const playerReport = (
  state: PlayerState,
  report: PlayerReport,
): PlayerState => ({
  ...state,
  ...(state.ended === undefined ? report : { retry: false, fullscreen: false }),
});

export function renderPlayer(state: PlayerState): Node {
  const controls: Node[] = [];
  if (state.retry)
    controls.push({
      id: "world-retry",
      element: { kind: "button", label: "Retry", emphasis: "ghost" },
    });
  if (state.fullscreen)
    controls.push({
      id: "world-fullscreen",
      element: { kind: "button", label: "Full screen", emphasis: "ghost" },
    });
  return {
    id: "player-bar",
    a11y: { role: "navigation", label: "Player" },
    element: {
      kind: "panel",
      children: [
        {
          id: "player-row",
          style: { gap: "sm", align: "between" },
          element: {
            kind: "stack",
            axis: "wrap",
            children: [
              {
                id: "player-heading",
                style: { gap: "sm" },
                element: {
                  kind: "stack",
                  axis: "horizontal",
                  children: [
                    back("player-back"),
                    text("player-title", state.title, "heading"),
                    label("player-version", `Version ${state.version}`, true),
                  ],
                },
              },
              {
                id: "player-actions",
                style: { gap: "xs" },
                element: {
                  kind: "stack",
                  axis: "horizontal",
                  children: controls,
                },
              },
            ],
          },
        },
      ],
    },
  };
}

export const makePlayerController = (
  initial: PlayerState,
  instance = `player-${Date.now().toString(36)}`,
) =>
  Effect.gen(function* () {
    const navigator = yield* Navigator;
    const engine = yield* PlayerEngine;
    return yield* Controller.make<PlayerState>({
      instance,
      initial,
      render: renderPlayer,
      handle: (state, intent, node) =>
        Effect.gen(function* () {
          switch (intent.node) {
            case "world-retry":
              yield* engine.retry;
              return state;
            case "world-fullscreen":
              yield* engine.fullscreen;
              return state;
          }
          const href = hrefOf(node);
          if (href) yield* navigator.go(href);
          return state;
        }),
    });
  });

/** How often an open visit asks whether its invitation still stands. */
export const VISIT_CHECK_MS = 15_000;

/**
 * While a visit is open, check its invitation; when it has ended, or a visit
 * that needs sign-in has lost it, close the player and say why. Runs until it ends or is interrupted.
 */
export const watchVisit = (
  controller: Pick<Controller.Controller<PlayerState>, "update">,
  every = VISIT_CHECK_MS,
) =>
  Effect.gen(function* () {
    const check = yield* VisitCheck;
    const engine = yield* PlayerEngine;
    while (true) {
      yield* Effect.sleep(every);
      const status = yield* check.status;
      if (status === "open") continue;
      const message = status === "ended" ? ENDED : SIGNED_OUT;
      yield* engine.close(message);
      yield* controller.update((state) => ({
        ...state,
        ended: message,
        retry: false,
        fullscreen: false,
      }));
      return;
    }
  });
