# Episode 263 Production Requests

Status: requested inputs with non-blocking defaults.
Applies to: [`Forking Zed`](263.md).
Requested by: Episode 263 production packet.

Episode 263 is the technical source and architecture tour.
Do not use mock Omega product screens.

## Request 263-01: exact fork proof

Priority: required.

Provide a clean Omega checkout at the recording commit.
Record:

```sh
git remote -v
git rev-parse HEAD
git merge-base HEAD upstream/main
git diff --stat <upstream-base>..HEAD
```

Keep the Omega commit and upstream base in the edit.
Mask local paths and unrelated remotes.

Default if the source advances: update the script metadata and all overlays to
the new audited commits before recording.

## Request 263-02: clean source tour

Priority: high.

Record these paths:

- root `Cargo.toml`
- `crates/zed`
- `crates/gpui`
- `crates/workspace`
- `crates/project`
- `crates/editor`
- `crates/git` and `crates/git_ui`
- `crates/terminal` and `crates/terminal_view`
- `crates/remote` and `crates/remote_server`
- `crates/agent`, `crates/agent_ui`, and `crates/agent_servers`

Use a sanitized editor window.
Do not show a private repository in recent-project or terminal history.

Default if unavailable: use a clean local screen recording from the audited
checkout.

## Request 263-03: inherited application journey

Priority: high.

Build or open the inherited application from the audited source.
Use `cargo run --profile release-fast` from a clean checkout.
Record:

1. command, commit, toolchain, and result
2. application launch
3. project open
4. definition navigation
5. diagnostics or symbols
6. Git status and Project Diff
7. a split terminal
8. pane movement and restoration
9. the Agent Panel

Label a source build
`OMEGA FORK BUILD - UPSTREAM ZED IDENTITY`.
Also label it `UNSIGNED DEVELOPMENT BUILD` when applicable.
Do not present inherited Zed branding as completed Omega branding.

Default if a source build is not ready: use a clean stock Zed installation.
Label it `STOCK ZED - CURRENT REFERENCE`.
Do not present it as proof from the audited Omega commit.

## Request 263-04: identity and branding audit

Priority: high.

Record the current Zed values in:

- `crates/zed/Cargo.toml`
- `crates/paths/src/paths.rs`
- `crates/release_channel/src/lib.rs`
- `crates/cli/src/main.rs`
- `crates/install_cli/src/install_cli_binary.rs`
- `script/bundle-mac`
- `script/terms/terms.rtf`

Show names, IDs, paths, endpoints, and packaging assumptions.
Do not reveal a credential or private signing value.

Default if branding lands before recording: show the Git diff from the audited
Zed values to the real Omega values.

## Request 263-05: architecture graphic

Priority: high.

Produce one diagram with:

```text
OMEGA RUST + GPUI CLIENT
          |
GENERATED VERSIONED LOCAL PROTOCOL
          |
PACKAGED NODE 24 + EFFECT SERVICES
```

Add `ACCEPTED ARCHITECTURE - NOT RUNNING`.

Default if the service exists by recording time: replace the diagram only with
a real health view and exact acceptance receipt.

## Request 263-06: Agent Client Protocol demo

Priority: medium.

Record one clean external-agent thread in Zed.
Show the thread, permission boundary, edits, and review.

Do not show an existing private agent home.
Do not claim Hermes support unless the exact Hermes journey passes.

Default if unavailable: use Zed's first-party Agent Client Protocol source and
documentation with attribution.

## Request 263-07: first branded Omega build

Priority: future replacement and release gate.

When the first branded build exists, provide:

- exact source commit and upstream base
- application version and bundle ID
- clean launch recording
- package and artifact digests
- signing and notarization receipt
- storage-isolation proof
- network-endpoint audit
- license and source-delivery record

An unsigned build must use the label `UNSIGNED DEVELOPMENT BUILD`.
Do not publish it as `v0.2.0-rc1`.

Default before that milestone: end on the public source tree and the planned
release sequence.

## Delivery checklist

For each supplied asset, include:

- the absolute source path or stable URL
- the owner and rights status
- the exact source commit
- the capture date and toolchain
- the evidence label that must remain visible
- private details that the editor must mask

The editor can replace a placeholder only when the asset proves the same claim.
