# Android drawer source slice

Issue #12087 adds Android Views over the existing Rust session and account-history
models. This directory is isolated from the B1 shell, transcript renderer, and
composer. It is not wired into the Android entry point.

`RustDrawerCore` forwards live search, selection, rename, archive, pin, and pin-move
intents to `CoreClient`. Saved title search, rename, pin, archive, and pin order
use the added Rust server, SDK, account, and UniFFI methods. The view never filters, ranks,
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

- Saved pin order uses `GET/PATCH /v1/history/pins` and one owner-scoped
  `history/pins.json` record. `Store::write_key` commits that record with a storage
  generation check; a move never rewrites Conversations. Requests carry the last
  order revision and exact neighbours. Stale revisions and gaps return
  `pin_order_changed` without a write. Unpin/repin starts a new placement, and
  archived, deleted, and continued chats leave the active pin order. Discovery
  scans at most 10,000 chats in 200-row pages, retains at most 4,096 pins, and has
  a 10-second deadline. On failure, Rust retains the last known order and disables
  move controls until a successful refresh. The View only forwards Rust-supplied
  move hints, for both live and saved pins.
- Live agent identity, indicator, and unseen state come from Rust `SessionRow`.
  The server `session_index::Row` owns published UUID/device/agent/status data and
  a separate copied transcript. Legacy saved Conversation IDs are hashes of
  `(owner, session string)`; `coder_sync::Upload` has computer/title/messages,
  repository, and activity, without a verified index-row/device link. A computer
  name is not that link. Therefore saved agent/status cannot safely be captured
  from the session index at save time with the current contract.
  `session_index::Row` also has no unseen field. The live read owner is the
  workspace `RegistryDoc::set_chat_seen`, reached through Rust `Client::mark_seen`;
  saved `opened_unix` only serves web work completion. Saved agent, indicator,
  and unread values remain optional until a verified source contract exists.
- Add the B1 source-set wiring and account lifecycle/listeners. The drawer already
  presents Rust-supplied projects and project paging; Rust checks shared visibility
  and keeps shared rows read-only. Move up/down menus use typed Rust hints. Keep
  #12083 as the native blocker; restoration and back gestures still need integrated
  Android validation.
- Regenerate Kotlin and Swift UniFFI bindings after building this Rust surface.
  The checked-in binding predates `file_chat`, `move_pin`, `drawer_pin_moves`, `Search`, and the new saved records;
  this bridge is source-only until regeneration and integrated compilation.

## Verification receipt

All fixtures are synthetic. No account, credential, network service, emulator, or
Android glyph metrics were used. No Cargo, Kotlin/JUnit, Gradle, Android, or
emulator check has run for this issue: free disk fell below the 100 GiB floor
(latest observed 93.324 GiB). Direct formatting of changed Rust files and
`git diff --check` passed. This is not acceptance-complete or ready to merge.

After capacity returns and the coordinator assigns the serial slot, queue:

1. Root SDK `cargo test -p openagents-sdk --test chats saved_rename` and
   `cargo test -p openagents-sdk --test history saved_` (search and pin-order
   fixtures, including a server that ignored the query and malformed move replies).
2. Root web `cargo test -p openagents-web saved_search`,
   `cargo test -p openagents-web saved_pin_moves`, and
   `cargo test -p openagents-web history_order::tests` (search/rename, owner denial,
   empty matched pages, concurrent order CAS, unchanged Conversations, and repin).
3. App workspace `cargo test -p openagents-account --test history` and focused
   `cargo test -p openagents-mobile --lib saved_filing_cancellation`,
   `cargo test -p openagents-mobile --lib saved_pin_controls`, and
   `cargo test -p openagents-mobile --lib live_move_hints` (account/query fences,
   confirmed ordering, outage retention, late account replies, and UniFFI hints).
4. UniFFI generation and integrated Kotlin compile; run the seven drawer JUnit tests.
5. Android Views tests for swipe/long-press, restoration, account change, project
   visibility, pagination, and predictive-back commit/cancel after shell wiring.

Use the repository's shared target and build-environment helper. Keep builds
serial and recheck free disk and build status before each heavy command.
