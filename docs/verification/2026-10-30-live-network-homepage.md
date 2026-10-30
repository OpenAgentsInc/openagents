# The live network homepage — 2026-10-30

The owner requested a real-data new-land page, promoted to `/` and deployed to
production. This follows the [verified capture migration](2026-10-30-newland-coax-nostr.md)
on `37a100a4c8`; the starting checkout was `cd28d3392e`.

## What changes

`/` now runs `newland.coax`; `/new-land` remains an alias with a root canonical.
The development-only release marker is removed. HTML/Markdown twins describe
the actual data sources. No simulated people are drawn. The six featured
project identities come from their successfully captured public configurations.

The browser opens two ordinary sockets to `wss://relay.openagents.com`. Shared
Coax owns Nostr requests, original event ID/BIP-340 verification, beacon digest
and freshness admission, bounded paging, subscription matching and project
folds. The persistent watch receives Pylons, featured project changes and
provider answer/status observations. The second socket refreshes project
histories, repairs exact previously known roots and reads followups in batches
of 50 IDs. Complete projects publish individually. Incomplete histories retain
the previous view. Dirty projects refresh fairly; periodic refresh recovers
changes missed during a disconnect. The UI shows connecting, syncing,
reconnecting or incomplete-refresh status instead of asserting a live refresh
when its reader has failed.

New Pylons appear without a reload. Only newer generation/observation pairs
replace held announcements. Capacity expires against the signed deadline using
the browser wall clock, including after a suspended animation loop. Answer and
status counters deduplicate full signed event IDs and reject events predating
the tab's watch. They are observations, not proof of completed or failed jobs;
reply timing remains unknown. Project worker names are project-authored claims,
not independently authenticated worker identities. Optional beacon owner auth
is carried without granting authority. No keys, signing or public test-event
publication are involved.

Git trees are real deployment snapshots. Featured identities and layout do not
discover arbitrary new projects; the Projects page supplies that broader view.
Runtime project and Pylon reads operate independently of the warm start.
The relay server and its remaining legacy backend are unchanged.

## Integration and bounds

`newland-network.coax` coordinates the renderer with separately compiled
`landing-browser.coax` and `landing-fold.coax` units. The C byte bridge selects
ABI entries and transports immutable bytes. JavaScript adds only a generic
wall-clock import and optional read-only module probe; no Nostr SDK enters the
homepage runtime. `landing-reader.ts` continues to use the same Coax verifier.

The first linked runtime exposed a 1 MiB stack overflow under actual socket
traffic. `build-home.sh` reserves an 8 MiB stack inside the unchanged 64 MiB
maximum Wasm memory. Real relay and signed fixture runs then passed. The
renderer checks use separate phone and desktop states to stay within the
interpreter's aggregate budget, without raising that budget. Production
geometry is bounded to 400,000 vertices; the measured scenes are below 49,000 after the realm galaxy merge.

The homepage admits at most 128 Pylons and draws at most 32 claim labels.
Project roots remain bounded to 256 per fold, with the existing reader's event,
byte and page budgets. Project projections have a 256 KiB presentation buffer;
beacon projections have a 16 KiB buffer and 2 KiB display rows. Oversized or
incomplete reads refuse; this is bounded coverage, not unlimited relay history.
The work observation deduplication set is bounded by the shared reader's event
budget. It never counts an already retained event a second time.

## Checks

Checks were written with the changes. Focused results:

- `coax test apps/openagents-web/newland.coax --steps 400000000`: 59/59 after the rebase (57/57 before it).
- `coax test coax/web/project/landing-browser.coax --library --steps 250000000`:
  293/293, including the shared crypto and protocol suites.
- `bun test coax/web/tests/newland_reader.test.ts`: 7 tests, 578 assertions.
  Original signed capture folds match frozen independent rows; partial, forged
  and unavailable reads refuse. The import graph has no legacy Nostr SDK.
- `bun test coax/web/tests/home_live_wasm.test.ts`: 1 test, 13 assertions.
  The actual linked homepage accepts valid beacons, rejects altered signatures
  and digests, updates capacity without reload, refuses rollback, reconnects,
  deduplicates answers/statuses and expires capacity. Incomplete project history
  preserves the prior projection and never claims that catch-up completed.
- Actual public Wasm run: all six projects refreshed, three real Pylons,
  zero refused refreshes, two connections. A separate ordinary browser run
  completed all six projects, the watch and beacon EOSE without JS errors.
- Linked renderer: nine frames across three viewports, finite geometry,
  guarded output buffers and expiry after an animation pause.
- `golden-pages.test.ts`: 7 tests, 27 assertions, including root HTML tracking.
  The unchanged golden policy tests passed earlier in this task (the initial
  combined run was 17 tests/612 assertions); they were not repeated.
- Headless muted Chrome desktop/phone screenshots were inspected. Only the
  new homepage goldens were approved. Shell syntax, visual rules and
  `git diff --check` pass.

The owner requested changed-code checks without full mutation runs; no full
mutation or unrelated model gates were run. The existing aggregate-budget and
history-count failures in the larger project interpreter suite remain outside
this change; actual Wasm capture/fold fixtures and live traffic qualify this
integration rather than claiming that unrelated suite now passes.

`newland-golden-data.coax` freezes a real public snapshot only for deterministic
visual tests. Ordinary visitors use live reads. Release staging and production
also run `home_live_browser.ts` on the ordinary homepage, require all six
project-ready bits, watch connection and beacon EOSE, and reject JS exceptions.
This gate allows a genuine empty provider set. Production failure rolls back.

Compact evidence is in [the evidence directory](data/live-homepage-2026-10-30/).
Before pushing, rebased onto `14d53d5e20`. Kept both the public Pylon page
release changes and `b5e318420f`'s small public-realm galaxy, including its
shared Verse projection/HUD and input checks. The merged homepage passed all
59 Coax checks, linked signed fixtures, guarded geometry and another ordinary
browser catch-up (six projects, two sockets, no JS errors). Its revised desktop
and phone screenshots were inspected and only the homepage goldens updated.
The shared verifier did not change during this rebase, so its passing checks
were not repeated.

The production receipt is appended after the normal staged release completes.
