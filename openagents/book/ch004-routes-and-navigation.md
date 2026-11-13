# 4. Routes and navigation

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

The app has one screen. This chapter gives it places to go: typed routes,
a back stack, a modal and browser URLs, all owned by shared state so the web
and the iPhone agree on where you are.

## Draft

A route is a destination with typed parameters, such as `home`, `login`,
`conversation(id)` or `settings(section)`. It is not a flag that hides one
screen and shows another. The shared app owns the route stack, the tab
stacks and which modal is open
([Coax Native, "Navigation and composed surfaces"](../../coax/native.md#navigation-and-composed-surfaces)).

Every way of moving asks for the same transition. A tap on Back, the
browser's back button, an iPhone swipe from the edge, a deep link and a tap
on a notification each become one action. The route changes in the
dispatcher, and the host follows.

On the web, a route has a URL. `/login` is the login screen, and
`/chat/ID` is a conversation. The address bar, the history and a shared link
all work, because the app parses the URL into a route and writes the route
back as a URL. Use "Log in" and `/login`, never "Sign in".

A URL is input, not permission. Parsing a link never logs you in, never opens
someone else's conversation and never admits you to a room. The route asks
the account and room policies, which already exist, and a refused route
lands on a page that says why.

A modal sits over a route and owns input while it is open. Closing it gives
focus back to where it was, and the tap that closed it never reaches the
screen underneath.

## What we build

A plan for what this chapter ships once the draft is approved:

- The app's routes: home, login, the logged-in shell and a conversation.
- A URL codec for the web and a deep-link codec for the iPhone, both over the
  same route type.
- The back stack, with browser history and the iPhone's back gesture both
  sending `route.back`.

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md)
- [`coax/web/drive.coax`](../../coax/web/drive.coax)
- [`apps/openagents-web/login`](../../apps/openagents-web/login)
- [`docs/app-shell.md`](../../docs/app-shell.md)

### The chapter's check

- **Source:** `openagents/examples/routes.coax`.
- **Trace:** parse `/login`, open the login modal, go back, then parse
  `/chat/5e55a0e1`, printing the route stack after each step.
- **Refusal:** `/chat/5e55a0e1` with no session gives the `login` route with
  the conversation as its return target, not the conversation.
- **Independent check:** a `#forall` round trip, every route to a URL and
  back, and a list of URLs the codec must refuse.

Planned paths: `openagents/examples/routes.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): the same agent from the web and the apps
- **Also:**
  - [Coax Native](../../coax/native.md)
  - [The app shell](../../docs/app-shell.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
