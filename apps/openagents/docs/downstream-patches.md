# Coder Desktop patches

Keep fork-specific changes here so upstream updates can review each integration.

- [OpenAgents #11836](https://github.com/OpenAgentsInc/openagents/issues/11836):
  Add Coder Light and Coder Noir to `zeron-theme`, make them the default light
  and dark selections, and use them when a selected theme is missing. Map the
  public `oa-tokens` palette snapshot in `crates/theme/src/builtins.rs`; no
  sibling checkout is needed. Preserve upstream themes and saved preferences.
  See [the theme system](theme-system.md) for the source revision.
- [OpenAgents D0 (Coder Desktop identity)](../FORK.md#identity-and-branding-seams-patch-log):
  Coder name and icon, `com.openagents.coder.desktop` bundle IDs, own data
  directory, IPC port and `coder-desktop://` scheme, and Zeron's update feed,
  sign-in and edge turned off. Values live in `crates/brand`; the full seam
  list and the platform matrix are in [FORK.md](../FORK.md).
- [OpenAgents #11871](https://github.com/OpenAgentsInc/openagents/issues/11871)
  and [#11883](https://github.com/OpenAgentsInc/openagents/issues/11883): Video
  Lab.
  - Our module `crates/mcp/src/video_lab.rs` offers the running Lab's `lab_*`
    tools to every harness and publishes each result's card through the
    `show_visual` path. Hooks: one `video_lab::prime()` in `crates/mcp/src/lib.rs`,
    the list and call arms in `crates/mcp/src/tools.rs`, and one paragraph in
    `crates/mcp/src/jsonrpc.rs` `INSTRUCTIONS`.
  - Our module `crates/ui/src/lab_card.rs` (with `lab_card/player.rs`,
    AVFoundation) draws a visual whose script starts with
    `-- coder:video_lab {card}` as a native card. The card follows the render
    and plays the video inline. Hooks: the `lab` field and the branch in
    `crates/ui/src/visuals.rs`, and `set_link_ui` taking `cx` in
    `crates/ui/src/transcript.rs`.
  - This card is an interim GPUI view. It moves to a Rust Native Lab card once
    the GPUI adapter exists (AGENTS.md: cards are Rust Native).
