# 15. Plugins with owned lifecycles

> **Status: partly implemented.** Coder loads plugins from
> `~/.openagents/coder/plugins`: each one's commands become slash commands,
> its states are visible, and disabling it removes everything it
> registered. A signed plugin installs only when its Nostr signature
> verifies and its key is trusted. Still to come: profiles, capabilities
> with providers and consumers, hooks that run on their events (they are
> registered and listed, but nothing fires them yet), the draining state,
> and third-party plugins in bounded Wasm.

Extensions to the harness need clear ownership. This chapter composes the
harness from ordered profiles and plugins. Every registration belongs to the
plugin that made it and is revoked when that plugin unloads. Third-party
plugins run in bounded Wasm and are distributed as signed Nostr events.

## What this chapter covers

- Profiles: ordered lists of plugins that make up a harness configuration.
- Capabilities with three roles: a definition, a provider and consumers.
- Owned registrations. Unloading a plugin removes every tool, hook and
  capability it registered.
- Visible states: installed, enabled, admitted, active, failed and draining.
- Third-party plugins in bounded Wasm with admitted capabilities only.
  DeepSeek Harness runs host plugins with the host process's authority;
  Psionic does not.
- Distribution as signed Nostr events, verified before install.

## What runs

A plugin is `NAME.json` in `~/.openagents/coder/plugins`
([plugins.coax](../../coder/tui/plugins.coax)):

```json
{"name":"tests","commands":[{"name":"test","run":"cargo test -q"}],
 "hooks":[{"on":"turn_end","run":"git status --short"}]}
```

`/test` in the terminal UI, or `coder run "/test"`, runs `cargo test -q`
as a `$ ` command in the session's folder, so it shows in the transcript
and the log like any other command. Words after the command's name are
passed on: `/test some_case` runs `cargo test -q some_case`.

### Owned registrations

The registry holds each command and hook with the index of the plugin that
made it. Disabling a plugin removes every registration it owns, and the
check walks the registry to show none is left. Admission checks every name
before it registers any, so a refused plugin leaves nothing behind either.
A command's name must be new: not a built-in (`/help`, `/clear`, `/engine`,
`/quit`, `/exit`, `/plugins`) and not another plugin's.

### States

| State | Means |
| --- | --- |
| installed | the file is there |
| enabled | it isn't on the `disabled` list |
| admitted | its manifest reads, a signature verifies, and no name collides |
| active | its registrations are made |
| failed | admission refused it; the reason is kept and shown |
| disabled | the person turned it off; it registers nothing |

`PL_MOVES` lists the nine transitions, and `pl_move` refuses any other.
`/plugins` and `coder plugins [--json]` show each plugin's state, its
commands and hooks, or the reason it failed. `/plugin disable NAME` and
`/plugin enable NAME` change it, and the `disabled` file keeps the choice.

```text
clash         active    /hello
demo          failed    a command's name is another plugin's
```

### Signed plugins

A plugin file can be a signed Nostr event instead: kind 30078 (NIP-78,
application data) with a `d` tag `coder-plugin:NAME`, whose content is the
manifest. It installs only when `nostr_event_verify_vartime` accepts the
event and its public key is a line of `~/.openagents/coder/plugins/trusted`.
The check holds one signed with a throwaway test key, then changes one byte of the
content and sees it refused:

```sh
openagents nostr sign --kind 30078 --tags '[["d","coder-plugin:NAME"]]' --content "$(cat manifest.json)"
```

## Checks

```sh
coax test coder/tui/coder.coax    # owned registrations, transitions, refusals, signatures
python3 coder/tui/smoke.py        # a plugin's command runs; disabled, it keeps no command
```

## Dependencies

- [Chapter4](ch004-tools-as-typed-capabilities.md) and [Chapter5](ch005-a-sandbox-and-approvals.md): tools and grants that plugins register.
- [Psionic chapter 59](../../psionic/book/ch059-run-in-the-browser.md): Coax compiled to Wasm.
- `coax/lib/nostr`: signed events (see the Part IV prerequisites in the
  [plan](PLAN.md)).

## Done means

- [x] After a plugin unloads, no registration it made remains.
- [ ] A Wasm plugin that calls a capability it was not admitted is refused.
- [x] A plugin event with a bad signature does not install.
- [x] The state of each plugin is visible and changes only through the listed
  transitions.

## Sources

- `coder/tui/plugins.coax`; `coder/tui/coder.coax` (`/plugins`, `/plugin`,
  plugin commands, `coder plugins`).
- `coax/lib/nostr/event.coax` (`nostr_event_verify_vartime`).
- [Chapter plan](PLAN.md); moved from Part XXI of the [Psionic book](../../psionic/book/README.md) on 2026-11-01.
- [Harness notes from DeepSeek Harness](../../psionic/docs/harness.md).
