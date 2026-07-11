# CUT-15 canonical command registry receipt

- Date: 2026-07-11
- Issue: [#8695](https://github.com/OpenAgentsInc/openagents/issues/8695)
- Status: complete on `main`
- Contracts: `openagents_desktop.commands.canonical_registry.v1`,
  `openagents_desktop.commands.host_routing.v1`, and
  `openagents_desktop.commands.private_binding_store.v1`
- Implementation: `aa3183748c`, `68f581c615`, `0dfc6f44a2`, and `5d36b73ad2`

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

Overrides persist in an owner-private mode-0600 JSON file beneath Electron
`userData`; replacement is atomic and the directory is mode 0700. Settings
projects defaults, overrides, effective bindings, and conflicts without
exposing the private path. Users can edit a binding, remove an override to
recover defaults, or reset the bounded store. Conflicted commands receive no
native accelerator until the conflict is repaired.

Native menu, deep-link, second-instance, and restore sources share one closed
deferred-command envelope. Unknown commands/arguments fail schema decoding;
decoded commands still require the declared session/workspace readiness and
verified-owner gate. Main owns native-menu construction, app-protocol admission,
the single-instance lock, bounded pre-ready queuing, and duplicate suppression.
The renderer acknowledges readiness before queued commands cross the decoded
preload bridge, then resolves them to the same typed intents used by visible
controls. Restart restoration also enters that resolver instead of assigning
workspace state directly.

The built Electron receipt starts a real second Electron process against the
same isolated `userData` root. It proves `openagents://command/settings.open`
reaches the primary instance, closes Settings, repeats the exact URL, and
surfaces duplicate rejection without redispatch. The per-run root is selected
before lock acquisition, so unrelated developer instances cannot turn the
receipt into a false-positive. Signed/notarized release packaging remains
CUT-26 rather than being inferred here.

Verification:

- Full Desktop: 435 pass, 0 fail, 2,346 expectations.
- Desktop typecheck and production build: pass.
- Built Electron smoke: pass, including command-palette host routing, real
  second-instance deep-link delivery, Settings close, visible duplicate
  rejection, renderer reload restoration, and lifecycle teardown with zero
  active resources.

CUT-15 is complete. CUT-16 may extend this registry with composer and runtime
control commands; it must not create provider-specific hotkey or deep-link
authority.
