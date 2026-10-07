# Brainstorm guidance

This `0.1.0` launch companion contains instructions and synthetic evidence.
The native Rust host owns the reads. The package contains no program, Wasm,
capability component, install script, background rule, authentication, or payment.
Its checked files become immutable pins in the existing signed EXT release.
No release or public profile has been published as part of this change.

## Required host

The supported source is Coder `1.0.0-rc.4` (`coder-new`) at
`d2d546b9836779e3ca463fd77b4610210fbedf7f`, which includes the private settings,
explicit commands, and admitted native functions. The exact requirement is in
[`native-host-requirement.json`](native-host-requirement.json). A newer host must
retain those contracts. Packaged availability still needs owner qualification;
a version label alone does not prove which source or features a build contains.
Inspect the release's source manifest and confirm the native **Brainstorm**
settings and operations before using it.

This requirement is descriptive. The existing `compatibility.coder` resolver
checks the older `coder` core, not `coder-new`, so this package does not use it
to claim enforcement. A host without the native binding is **unavailable**.
Do not replace a missing binding with shell HTTP or another plugin.

## Install and use

On a host with the supported Unix CLI plugin installer, install this directory:

```sh
openagents plugin install ./plugins/brainstorm
openagents plugin installed
```

Installation leaves this guidance off. If you choose to mark the companion
enabled in the local EXT inventory, run
`openagents plugin enable brainstorm-guidance`. Coder's native binding and
instructions remain separately bundled; this inventory setting does not load
a new adapter, enable **Brainstorm**, or perform a read. Read this companion as
documentation on other hosts; installing it does not add a missing binding.

In Coder, open `/plugins`, select **Brainstorm**, review the HTTPS recipient and
house perspective, save the configuration, and deliberately enable the native
plugin. Opening, saving, enabling, disabling, and guidance installation make no
service request. **Test connection** is a separate public discovery read.

```text
/brainstorm search <public query>
/brainstorm rank <exact public hex key or npub> [more public keys]
```

These explicit commands work without model credentials. Review the exact public
input before submitting it: the configured recipient receives it. Secret keys,
profile URLs, duplicate keys, and oversized inputs refuse. Esc cancels; disabling
retires pending snapshots. Demo uses labeled fixtures with no service request.

OpenRouter's `brainstorm_search_people` and `brainstorm_rank` use the same native
client. Model-proposed inputs require the host's exact recipient/input approval
desk. A missing desk refuses. In the terminal, review the full disclosure with
PgUp/PgDn, then Y confirms, N rejects, and Esc cancels. Headless callers use the
existing `--approvals stdin` desk. Opaque `input_ref` values are short-lived host
admissions, not package metadata; do not supply, invent, or persist them here.
Model-owned nested CLI prompts cannot turn themselves into deliberate user reads.

Follow [`skills/brainstorm.md`](skills/brainstorm.md) when interpreting results.
Use [`PILOT.md`](PILOT.md) only for a separately approved discoverability pilot.

## Release review

The existing packer emits one inert `guidance` component for the skill. It pins
every listed file's exact digest and size, including this requirement and the
examples; it does not emit executable native or capability registration.
The offline CLI tests pack, sign into fake stores, install off, verify file
pins, reject tampering and version rebinding, and check the synthetic pilot.
After those checks, an owner may choose their authorized publisher and use
`openagents plugin publish ./plugins/brainstorm --as <profile>` separately.
Record the returned exact publisher, release event, and manifest digest.
Raise the package version before changing a published file. Publication is not
an installation or native-enable side effect.
