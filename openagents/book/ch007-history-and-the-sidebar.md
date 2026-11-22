# 7. History and the sidebar

> **Status: partly implemented.** The shared Coax history and sidebar index
> module ([`apps/shared/app/history/index.coax`](../../apps/shared/app/history/index.coax))
> runs on `cpu` with its runnable book example
> [`openagents/examples/history-sidebar.coax`](../examples/history-sidebar.coax)
> and committed debug/release transcript
> [`openagents/examples/history-sidebar.out`](../examples/history-sidebar.out),
> exercising signed-in canonical page fetches (`ctr_summary`/`ctr_index`), cursor
> pagination, case-insensitive title search across pages, non-advancing cursor
> refusal (`scan_limit`), and logout clearing. Remaining for native hosts:
> deep-link route codec `#forall` checks, drawer/sidebar accessibility tests, and
> `openagents/examples/cases/history.ops`.

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

- [`apps/shared/app/history/index.coax`](../../apps/shared/app/history/index.coax)
- [`coax/native.md`](../../coax/native.md#navigation-and-composed-surfaces)
- [`docs/app-shell.md`](../../docs/app-shell.md)
- [`components/kit/controls.coax`](../../components/kit/controls.coax) (`ctx_list`)
- [`components/kit/listing.coax`](../../components/kit/listing.coax)
- [`coax/lib/agent/session/session.coax`](../../coax/lib/agent/session/session.coax)

### The chapter's check

- **Source:** [`apps/shared/app/history/index.coax`](../../apps/shared/app/history/index.coax),
  [`openagents/examples/history-sidebar.coax`](../examples/history-sidebar.coax),
  and the committed stdout transcript [`openagents/examples/history-sidebar.out`](../examples/history-sidebar.out)
  (remaining: `openagents/examples/cases/history.ops` and deep-link codec checks).
- **Trace:** `hi_sync` queues no HTTP effect while logged out; after sign-in it
  queues `GET /api/chat/conversations/v2alpha1`, parses canonical page 1 (`items=2`,
  `has_next=true`), matches `"executor"` across titles (`search_matches=1`), and
  clears all loaded rows on logout (`after_logout_loaded=false items=0`).
- **Refusal:** a non-advancing pagination cursor trips `scan_limit=true` with
  `scan_done=false` rather than looping forever; remaining for native hosts is
  refusing deep links to `chat(ID)` with no session by opening the login card.
- **Independent check:** `coax/bin/coax run openagents/examples/history-sidebar.coax`
  and `--release` produce identical transcripts matching
  [`openagents/examples/history-sidebar.out`](../examples/history-sidebar.out),
  beside `coax/bin/coax test apps/shared/app/history/index.coax`.

Planned paths: `openagents/examples/cases/history.ops`. The app is
[`apps/openagents-ios`](../../apps/openagents-ios).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): the same agent from the web and the apps
- **Also:**
  - [The app shell](../../docs/app-shell.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1

## Source listing

Run the Chapter 7 example in [`openagents/examples/history-sidebar.coax`](../examples/history-sidebar.coax) (committed transcript [`openagents/examples/history-sidebar.out`](../examples/history-sidebar.out)):

<!-- source: openagents/examples/history-sidebar.coax history-sidebar -->
```jai
hs_run :: () -> bool {
  c: [1] AppCore;
  app_init(c[..]);
  h: [1] ChatIndex;
  lib: [1] ChatLibrary;

  // Logged out: syncing requests no history HTTP effect.
  c[0].foreground = true;
  hi_sync(h[..], c[..], true);
  print("logged_out_op=% loaded=%\n", h[0].op, h[0].loaded);

  // Sign in and open the sidebar: ChatIndex queues GET /api/chat/conversations/v2alpha1.
  c[0].auth = AUTH_IN;
  hi_sync(h[..], c[..], true);
  op1 := h[0].op;
  print("signed_in_op_queued=%\n", op1 > 0);

  // Build canonical page 1 (2 conversations + next cursor) via ctr_summary/ctr_index and deliver through AppCore.
  row0: [512] u8; row1: [512] u8; raw: [4096] u8;
  cv0 := [1] CtConversation.[CtConversation.{id = "cnv_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", title = "Alpha build", revision = 1, created = 1, updated = 2}];
  cv1 := [1] CtConversation.[CtConversation.{id = "cnv_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", title = "Executor settlement", revision = 1, created = 2, updated = 3}];
  n0 := ctr_summary(cv0[..], row0[..]);
  n1 := ctr_summary(cv1[..], row1[..]);
  items := [2] string.[cast(string) row0[0..<n0], cast(string) row1[0..<n1]];
  rn := ctr_index(items[..], "cnv_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", raw[..]);
  page1 := cast(string) raw[0..<rn];

  _ := fx_take(c[0].out[..], op1);
  gen := fx_generation(c[0].out[..], HI_OWNER);
  _ = app_dispatch(c[..], AppEvent.{kind = EV_DONE, op = op1, generation = gen, outcome = FXO_DONE, status = 200, text = page1});
  ok_g := hi_guest(h[..], c[..], page1);
  p1_ok := ok_g && h[0].loaded && h[0].page[0].n == 2 && h[0].page[0].next_n > 0;
  print("page1_handled=% loaded=% items=% has_next=%\n", ok_g, h[0].loaded, h[0].page[0].n, h[0].page[0].next_n > 0);

  // Case-insensitive title search scans existing pages without an unversioned wire filter.
  hi_query(h[..], c[..], "executor", lib[..]);
  h[0].held[0] = h[0].page[0];
  h[0].held_at = 0;
  hi_scan_fill(h[..]);
  search_ok := h[0].found[0].n == 1 && h[0].scan && !h[0].scan_done;
  print("search_matches=% scan_more=% scan_done=%\n", h[0].found[0].n, h[0].scan, h[0].scan_done);

  // Refusal: a non-advancing cursor terminates cleanly with scan_limit=true instead of looping.
  h[0].sent_n = h[0].held[0].next_n;
  h[0].sent = h[0].held[0].next;
  h[0].held_at = h[0].held[0].n;
  hi_scan_fill(h[..]);
  limit_ok := h[0].scan_limit && !h[0].scan_done;
  print("stalled_cursor_limit=% scan_done=%\n", h[0].scan_limit, h[0].scan_done);

  // Sign out clears loaded history immediately.
  c[0].auth = AUTH_OUT;
  hi_sync(h[..], c[..], true);
  print("after_logout_loaded=% items=%\n", h[0].loaded, h[0].page[0].n);

  return p1_ok && search_ok && limit_ok && !h[0].loaded && h[0].page[0].n == 0;
}
```

Output (`coax/bin/coax run openagents/examples/history-sidebar.coax`, identical in `--release`):

```text
logged_out_op=0 loaded=false
signed_in_op_queued=true
page1_handled=true loaded=true items=2 has_next=true
search_matches=1 scan_more=true scan_done=false
stalled_cursor_limit=true scan_done=false
after_logout_loaded=false items=0
ok=true
```
