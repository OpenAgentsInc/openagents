# Android drawer source slice

Issue #12087 adds Android Views over the existing Rust session and account-history
models. This directory is isolated from the B1 shell, transcript renderer, and
composer. It is not wired into the Android entry point.

`RustDrawerCore` forwards live search, selection, rename, archive, pin, and pin-move
intents to `CoreClient`. Saved title search, rename, pin, and archive use the added
Rust server, SDK, account, and UniFFI methods. The view never filters, ranks,
reorders, or mutates session rows locally. Origin-qualified keys keep live rooms
and saved copies distinct. Shared-project saved rows remain read-only.

The shell must provide an account-bound core, a UI coroutine scope, and the
Rust-selected navigation key. Route core change notifications to `show`; replace
and detach the presenter on account changes. Keep the returned live handle in the
existing navigation owner. Restore only an existing key with `restore`; never use
session creation for restoration. Install `DrawerBack` only while the drawer is
open, unregister it at teardown, and call `handleBack` on older Android versions.
The view saves the search, tab, scroll position, and selected opaque key, not chats.

Search reads at most 200 authorized stored chats per request. A page with no
matches can still have more pages; **Load more** continues the server cursor.
Changing a query cancels the old account-history query. Saved ordering changes
only after the server confirms a mutation and Rust refreshes the list.

## Remaining code and integration work

- Saved pin reorder needs durable positions. `Conversation` currently stores only
  `pinned_unix`. `Store::compare_and_swap` checks one owner, chat ID, revision, and
  storage generation; it cannot atomically rewrite several chats. `pin_stamp`
  allocates adjacent integers. A neighbour insertion can have no integer slot.
  Add a durable ordered-position contract with deterministic ID tie-breaks before
  exposing saved reorder. No multi-row timestamp rewrite is implemented here.
- Live agent identity, indicator, and unseen state come from Rust `SessionRow`.
  The server session index owns published computer-session agent names and status.
  Saved `Conversation` has no agent identity or generic unread field. Its
  `opened_unix` serves pending work completion; it is not a general unread flag.
  Saved agent, indicator, and unread values stay explicitly optional. A separate
  Rust owner/source must supply these before Android can display them.
- Add the B1 source-set wiring, account lifecycle/listeners, shared-project picker,
  and live drag target controls. The typed move intent exists, but the view does
  not yet expose a reorder gesture. Keep #12083 as the native blocker.
- Regenerate Kotlin and Swift UniFFI bindings after building this Rust surface.
  The checked-in binding predates `file_chat`, `Search`, and the new saved records;
  this bridge is source-only until regeneration and integrated compilation.

## Verification receipt

All fixtures are synthetic. No account, credential, network service, emulator, or
Android glyph metrics were used. No Cargo, Kotlin/JUnit, Gradle, Android, or
emulator check has run for this issue: free disk fell below the 100 GiB floor
(latest observed 97.153 GiB). Direct formatting of changed Rust files and
`git diff --check` passed. This is not acceptance-complete or ready to merge.

After capacity returns and the coordinator assigns the serial slot, queue:

1. Root SDK `--test chats saved_rename` and `--test history saved_title_search`.
2. Web `saved_search` fixtures, including owner denial, rename idempotence, and an
   empty matched page with a continuing cursor.
3. App-workspace `saved_filing_cancellation` plus existing account history fixtures.
4. UniFFI generation and integrated Kotlin compile; run the six drawer JUnit tests.
5. Android Views tests for swipe/long-press, restoration, account change, project
   visibility, pagination, and predictive-back commit/cancel after shell wiring.

Use the repository's shared target and build-environment helper. Keep builds
serial and recheck free disk and build status before each heavy command.
