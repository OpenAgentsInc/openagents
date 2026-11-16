# 7. History and the sidebar

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 6 left one chat page that forgets your conversation when you start
another. This chapter adds the sidebar: your past conversations, a new
chat, and the first routes, so the app has more than one place to be.

## Draft

The sidebar is a surface beside the chat page on a wide screen and a drawer
over it on a phone. It lists your conversations, newest first, with New
chat at the top and who is logged in at the foot. Each row is a component
with a stable ID built from the conversation's ID, `history.ID`, never its
place in the list. The list realizes only the rows on screen, as the
transcript does.

The history comes from the service, not the device. The list is a resource
with explicit states: absent, pending, ready or failed. Loading it is an
effect, like a send; the rows appear when the answer arrives as an action.
A conversation you open loads its record and draws it with chapter 3's
transcript.

With two places, the app needs routes. A route is a destination with typed
parameters, such as `chat(id)` or `chat.new`, not a flag that hides one
screen and shows another
([Coax Native, "Navigation and composed surfaces"](../../coax/native.md#navigation-and-composed-surfaces)).
The app owns the route stack and which drawer or card is open. Every way of
moving asks for the same transition: a tap on a row, the edge swipe that
closes the drawer, a deep link and a tap on a notification each become one
action. The route changes in the dispatcher, and the host follows.

A link is input, not permission. Parsing a link to `chat(id)` never logs
you in and never opens someone else's conversation. With no session, the
route opens chapter 5's login card with the conversation as its return
target. A conversation that isn't yours lands on a page that says so.

Each conversation keeps its own draft while the app runs. Switching away
and back keeps what you typed. A send that is still pending keeps its turn;
switching conversations never cancels it and never sends it twice.

Which drawer folds are shut is this device's preference. Your conversations
are your account's data and are the same on every device you log in on.

## What we build

A plan for what this chapter ships once the draft is approved:

- The sidebar surface: New chat, the conversation rows, and the account
  foot.
- The history list as a resource loaded by an effect.
- Typed routes `chat(id)` and `chat.new`, the route stack, and a deep-link
  codec for the iPhone.
- A draft per conversation, kept in memory.

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md#navigation-and-composed-surfaces)
- [`docs/app-shell.md`](../../docs/app-shell.md)
- [`components/kit/controls.coax`](../../components/kit/controls.coax) (`ctx_list`)
- [`components/kit/listing.coax`](../../components/kit/listing.coax)
- [`coax/lib/agent/session/session.coax`](../../coax/lib/agent/session/session.coax)

### The chapter's check

- **Source:** the sidebar and routes in the lean app, with their checks, and
  `openagents/examples/cases/history.ops`.
- **Trace:** after chapter 6's case, `tap side.open`, `tree` lists
  `history.new` and one `history.ID` row; `tap history.new`; send a second
  message; `tap history.ID` for the first; `state route` prints `chat(ID)`
  and `state transcript` prints the first conversation.
- **Refusal:** a deep link to `chat(ID)` with no session gives the login
  card with `chat(ID)` as its return target, not the conversation.
- **Independent check:** a `#forall` round trip, every route to a deep link
  and back, and a list of links the codec must refuse.

Planned paths: `openagents/examples/cases/history.ops`. The app is
[`apps/openagents-ios`](../../apps/openagents-ios).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): the same agent from the web and the apps
- **Also:**
  - [The app shell](../../docs/app-shell.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
