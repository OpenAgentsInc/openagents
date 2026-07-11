# CUT-15 canonical command registry receipt

- Date: 2026-07-11
- Issue: [#8695](https://github.com/OpenAgentsInc/openagents/issues/8695)
- Status: canonical registry/conflict/deferred-open contract active at
  `aa3183748c`; host menu, deep-link, and single-instance routing remain open
- Contract: `openagents_desktop.commands.canonical_registry.v1`

OpenAgents Desktop now has one provider-neutral command registry. Every entry
declares its stable id, existing typed intent, scope, availability gate,
authorization gate, argument and result shape, default arguments/bindings, and
palette visibility. The existing Effect Native palette is derived from that
registry, so its buttons retain the same intents as visible controls without a
second command list.

User binding aliases normalize to one bounded modifier/key grammar. If two
commands claim the same chord, that chord is removed from the dispatchable set
and returned as a deterministic conflict until the user removes the override.
Malformed chords and unknown command ids cannot enter the active map.

Native menu, deep-link, second-instance, and restore sources share one closed
deferred-command envelope. Unknown commands/arguments fail schema decoding;
decoded commands still require the declared session/workspace readiness and
verified-owner gate.

Verification:

- Command contract: 3 pass, 0 fail, 66 expectations.
- Command contract + existing shell: 50 pass, 0 fail, 317 expectations.
- Full Desktop: 385 pass, 0 fail, 2,090 expectations.
- Desktop typecheck and production build: pass.

CUT-15 remains open for host-owned native menus, exact deep-link and
second-instance admission, readiness-queued dispatch into the renderer, user
keybinding persistence/settings recovery, and the packaged-host receipt.
