# Desktop chat list verification

Issue: [#10012](https://github.com/OpenAgentsInc/openagents/issues/10012).

The list reimplements Zeron's search, pin, title editing, and grouped sidebar
in shared Rust and Rust Native. No Zeron source or GPUI runtime is copied.

## Durable metadata and shared projection

Pin, title, and archive state commit with each encrypted conversation record.
A custom title survives the first message. Archive retains the conversation;
restore retains the title and pin. A Coder handoff records the host's workspace
label for project grouping. Search reads titles and workspace labels without
opening transcripts. Ordering and groups live in `openagents-chat-app::chat_list`;
the phone's existing Rust list uses the same projection and metadata labels.

The store admits 512 conversations. The encrypted index keeps a warm head of
128 summaries; opening the cache recovers the rest from their self-describing
records. Local snapshots page the catalog in groups of 128 and bind subsequent
pages to the catalog revision. A changed revision requires a fresh read. This
keeps both encrypted index writes and control replies within their bounds.
The shared client joins the pages and refreshes changed metadata.

The desktop provides search, pin/unpin, rename/save/cancel, archive, and restore.
Title edits use their own native editor and cannot replace a message draft.
A delayed rename acknowledgment closes only the matching unchanged title edit.
Search preserves the selected conversation and message draft when its row is
filtered out. Archived rows live in a separate collapsed group.

## Checks

Rust 1.97.1, on an M5 Max Mac:

- Shared hosted core: 29 passed; 1 opt-in live check ignored. The new scratch
  encrypted-store test creates 512 conversations, renames, pins, archives,
  restarts, restores, and unpins. It checks page identities, revision refusal,
  title validation, and custom-title preservation.
- Shared chat application: 74 passed. New tests check search, ordering, project
  groups, all catalog pages, and equality with the host's changed metadata.
- Desktop library: 81 passed; binary: 29 passed and 4 opt-in checks ignored;
  pairing: 3 passed. Scripted controls exercise every list action against the
  shared host service and check default and minimum window bounds.
- Native adapter: 47 passed, including retained-frame equivalence.
- Phone Rust consumer: 142 passed; 19 opt-in checks ignored.
- Targeted formatting, strict all-target Clippy, and release build passed.

The [default capture](list-1200.png), [minimum capture](list-760.png), and
[512-chat capture](512-chats.png) were visually inspected. The large-list check
has fewer than 20 visible chat rows and fewer than 300 foreground operations.
The adapter keeps offscreen keyboard and scroll targets but emits no text,
surface, or button paint operations for rows outside the list viewport.
No owner host, real home, keychain, or saved chat was used.

## Timing

[The native run](native.json) uses 3,300 transcript rows, 500 chats,
1200 × 840 points, 2× rendering, and the Grid backdrop. Each active phase has
115 samples. Timings include application work and CPU frame submission, not
GPU completion or display latency.

| Phase | p50 | p99 |
| --- | ---: | ---: |
| Scroll | 2.499 ms | 4.279 ms |
| Streaming | 6.472 ms | 7.257 ms |
| Sidebar | 2.228 ms | 5.952 ms |

Idle CPU was 3.27%; peak RSS was 232.0 MiB. The earlier eight-case scale,
window-size, and backdrop matrix remains in the transcript verification.
