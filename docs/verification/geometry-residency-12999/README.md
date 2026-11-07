# Preserve incoming geometry-page dependencies (#12999)

The CPU residency planner already landed in `91f329e32b`, with up to 64 pages,
32 slots, four lower-ID dependencies per page, root pins, bounded fetches,
out-of-order arrivals, retries, occlusion keep requests and eviction planning.
The open-issue review found an install-order bug in that implementation.

## Reproduction and fix

Use three pages and two slots, with no pins and a zero-frame protection window.
Page 2 depends on page 0:

1. Install parent 0 and unrelated page 1.
2. Request child 2 and receive its bytes, reserving room because page 1 is old.
3. While child 2 waits to install, page 1 becomes wanted again.
4. Try to install child 2. Parent 0 is now the only ordinarily evictable page.

Previously, `res_install` checked that parent 0 was installed, then evicted it
to make the child's slot. Publishing child 2 without its parent broke the
dependency invariant. The new regression fails on that original ordering
(`red.txt`).

The fix increments the incoming page's parent dependency counts in the local
transaction **before** choosing the eviction victim. If no unrelated victim
qualifies, it returns an empty plan and publishes no counts or state. Once
page 1 ages out, it can be evicted and child 2 installs while parent 0 remains.
The existing sibling/ancestor eviction tests also check resulting counters.

An additional two-parent occlusion case checks that every parent remains
wanted without initiating a fetch. The randomized check now recomputes its
invariants after each individual arrival, failure and install, as well as
each request plan, across 12 seeded runs of 89 frames.

## Verification (2026-11-07)

```sh
coax/bin/coax test aethyr/render/geometry/residency.coax --library
coax/bin/coax mutate aethyr/render/geometry/residency.coax --library
coax/web/tool.sh tools-check
```

- **16 checks passed**, including the 12-seed randomized sequence check
  (`tests.txt`).
- **464 mutants killed, zero survivors**, with no equivalent annotations
  (`mutation.txt`).
- The targeted regression was run before changing the implementation and
  failed on the unsafe parent eviction (`red.txt`).

## Scope and follow-up

This establishes the CPU planner's dependency and capacity rules. It neither
allocates GPU objects nor changes a realm's rendered cut. `pages.coax` retains
its existing byte identity, decoding and ticket/epoch authority.

The remaining integration is already tracked by
[#13008](https://github.com/OpenAgentsInc/openagents/issues/13008), whose work
includes feeding its request array into this residency module. The concrete
residual adapter is `apps/openagents-web/valleylab.coax`: `ValleyLab.pages`
still holds the two-slot `GeometryPages`, with `page_request`/`page_arrive`
and the old `cut.coax` selection. That follow-up must compose byte/ticket
ownership with N-slot install/evict plans, apply the
`parent_too_coarse && (fine_enough || children_not_installed)` draw rule, and
qualify real transport, GPU lifetimes and missing/occluded geometry in Chrome.
It stays open. No browser, GPU streaming or image-parity qualification is
claimed by these CPU checks.
