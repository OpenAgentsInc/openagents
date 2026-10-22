# HAUNTED HOUSE beside ICE CREAM MAN on the Grid (#12472)

The Grid takes a list of hosted arches as data: a tenant, its lettering,
and its site. `?hosted=icecream,haunt` stands both arches side by side;
`?hosted=icecream` works as before. Known tenants bring their own
lettering and site (`verse::zones::gate::KNOWN_HOSTED`), and any other
tenant stands outboard of the last arch, 7 m apart, with a plain arch.

| Arch | Site (side, forward) | World (x, z) |
| --- | --- | --- |
| ICE CREAM MAN | [20, 19] | (20, 9) |
| HAUNTED HOUSE | [27, 19] | (27, 9) |

## What was checked

These captures come from the local livehost demo
([`docs/verse/ice-cream-man-live.md`](../../ice-cream-man-live.md)): a
local relay, `icecream-live serve` with the `icecream`, `haunt`, and `grid`
tenants, and a release Grid build, in headless Chrome under
`openagents browser run`. `capture.json` has the readings.

- **Both arches stand side by side** (`both-arches.png`), drawn by the
  engine Grid from its pack: the pack gained a lettered arch for each known
  tenant and a plain one for any other (`verse::grid_pack::hosted_model`).
  Before this, the engine Grid drew no hosted arch at all.
- **Each prefetches its own tenant.** Standing in front of each arch, the
  page's `window.verse_hosted` reports that arch `prefetched`.
- **Each enters its own tenant.** Walking through opens `/t/haunt/` or
  `/t/icecream/`, one zone at a time.
- **Back to the Grid returns in front of the right arch**: 3.5 m in front
  of the arch walked through, facing away from it (`haunt-back.png`,
  `icecream-back.png`).

Tests: `the_haunted_house_arch_stands_beside_ice_cream_man_with_both_approaches_clear`
and `a_page_can_ask_for_several_hosted_arches_as_data` in
`crates/verse/src/zones/gate.rs`,
`two_hosted_arches_each_hand_over_their_own_tenant_and_return_in_front_of_it`
in `crates/verse/src/zones/tests.rs`, and
`hosted_arches_stand_as_their_own_lettered_models` in
`crates/verse/src/grid_pack.rs`. The single-arch test is unchanged.

## Limits

- Headless Chrome doesn't composite a WebGL page inside the zone's frame,
  so the zone itself isn't in these pictures; its own page is checked in
  [the house's captures](../2026-10-22-haunted-house/README.md).
- The zone opens in a full-window frame with a **Back to the Grid**
  button, as The Ice Cream Man's did (#12431). A zone page can also ask to
  go back itself (`{"type": "hosted-back"}`), which The Haunted House's
  own menu and ending card do.
- Production serves no `/t/TENANT/` yet; this is the local demo.
