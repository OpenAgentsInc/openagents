// The /new-lander/verse entry dialog as an Aethyr controller: account or
// guest, then a choice of where to go, then closed. The Verse controller
// (`verse.ts`) draws this tree as part of the page; `makeEntryController`
// runs it on its own.
import { Context, Effect } from "effect";
import * as Controller from "./controller";
import type { Node } from "./protocol";

/** What the server knows when the page loads. */
export class EntryOptions extends Context.Service<
  EntryOptions,
  {
    /** Sign-in is available on this site. */
    readonly accounts: boolean;
    /** The Haunted House tutorial, when this site serves it. */
    readonly hauntPath: string | undefined;
  }
>()("aethyr/EntryOptions") {}

/** Leaves the page. The web host assigns `location`; tests record it. */
export class Navigator extends Context.Service<
  Navigator,
  { readonly go: (path: string) => Effect.Effect<void> }
>()("aethyr/Navigator") {}

/** Remembers that this browser chose to look around as a guest. */
export class GuestMemory extends Context.Service<
  GuestMemory,
  { readonly remember: Effect.Effect<void> }
>()("aethyr/GuestMemory") {}

export type EntryStep = "account" | "choice" | "closed";
export interface EntryState {
  readonly step: EntryStep;
}

export const PATHS = {
  signIn: "/login?return_to=%2Fnew-lander%2Fverse",
  home: "/new-lander",
} as const;

const text = (
  id: string,
  value: string,
  role: "title" | "body" = "body",
): Node => ({ id, element: { kind: "text", value, role } });
const label = (id: string, value: string): Node => ({
  id,
  element: { kind: "label", value },
});

export function renderEntry(
  state: EntryState,
  options: {
    readonly accounts: boolean;
    readonly hauntPath: string | undefined;
  },
): Node {
  const head = label("entry-brand", "OPENAGENTS / THE VERSE");
  const account: Node = {
    id: "verse-account",
    style: { gap: "md" },
    element: {
      kind: "stack",
      axis: "vertical",
      children: [
        label("account-label", "YOUR PLACE IN THE VERSE"),
        text("entry-title", "Your next world is waiting.", "title"),
        text(
          "entry-description",
          options.accounts
            ? "Log in, or look around as a guest."
            : "Look around as a guest.",
        ),
        {
          id: "account-actions",
          style: { gap: "sm" },
          element: {
            kind: "stack",
            axis: "vertical",
            children: [
              ...(options.accounts
                ? [
                    {
                      id: "verse-signin",
                      element: {
                        kind: "button",
                        label: "Log in",
                        href: PATHS.signIn,
                        trailing: "forward",
                      },
                    } satisfies Node,
                  ]
                : []),
              {
                id: "continue-guest",
                element: {
                  kind: "button",
                  label: "Continue as guest",
                  emphasis: "primary",
                  trailing: "forward",
                },
              },
            ],
          },
        },
      ],
    },
  };
  const choices: Node = {
    id: "verse-choices",
    style: { gap: "md" },
    element: {
      kind: "stack",
      axis: "vertical",
      children: [
        label("choice-label", "CHOOSE YOUR NEXT STEP"),
        text("choice-title", "Where shall we go?", "title"),
        text(
          "choice-description",
          "Explore the planets, or step inside the Haunted House.",
        ),
        {
          id: "choice-actions",
          style: { gap: "sm" },
          element: {
            kind: "stack",
            axis: "vertical",
            children: [
              {
                id: "verse-explore",
                element: {
                  kind: "button",
                  label: "Explore the planets",
                  trailing: "forward",
                },
              },
              ...(options.hauntPath
                ? [
                    {
                      id: "verse-haunt",
                      element: {
                        kind: "button",
                        label: "Haunted House tutorial",
                        emphasis: "primary",
                        href: options.hauntPath,
                        trailing: "forward",
                      },
                    } satisfies Node,
                  ]
                : []),
            ],
          },
        },
      ],
    },
  };
  return {
    id: "verse-entry",
    element: {
      kind: "dialog",
      label:
        state.step === "account"
          ? "Your next world is waiting."
          : "Where shall we go?",
      open: state.step !== "closed",
      children: [
        head,
        state.step === "account" ? account : choices,
        {
          id: "verse-entry-back",
          element: {
            kind: "button",
            label: "Back to OpenAgents",
            emphasis: "link",
            href: PATHS.home,
            leading: "back",
          },
        },
      ],
    },
  };
}

/** Build the entry controller from the services in scope. */
export const makeEntryController = (
  instance = `verse-entry-${Date.now().toString(36)}`,
) =>
  Effect.gen(function* () {
    const options = yield* EntryOptions;
    const navigator = yield* Navigator;
    const guest = yield* GuestMemory;
    return yield* Controller.make<EntryState>({
      instance,
      initial: { step: "account" },
      render: (state) => renderEntry(state, options),
      handle: (state, intent) =>
        Effect.gen(function* () {
          switch (intent.node) {
            case "continue-guest":
              yield* guest.remember;
              return { step: "choice" } as const;
            case "verse-explore":
              return { step: "closed" } as const;
            case "verse-signin":
              yield* navigator.go(PATHS.signIn);
              return state;
            case "verse-haunt":
              if (options.hauntPath) yield* navigator.go(options.hauntPath);
              return state;
            case "verse-entry-back":
              yield* navigator.go(PATHS.home);
              return state;
            default:
              return state;
          }
        }),
    });
  });
