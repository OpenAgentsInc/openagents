# Grok terminal animation inventory

This document inventories visible, time-varying behavior in the Grok terminal
app. The audit covers `xai-grok-pager` and the Mermaid renderer in `grok-build`
commit `07b2f7144fd5c5c9d3dd1966937a87852d2dbdb8` on August 26, 2026.

The inventory groups call sites that use the same animation. It lists separate
surfaces when their trigger, timing, or purpose differs.

## Animation clock

The event loop schedules animation frames only while a visible surface needs
them. Most animations use the configured frame rate, with approximately 30
frames per second as the reference cadence. The welcome logo requests about 12
frames per second. Touchpad scrolling uses a separate 16 ms clock. The hidden
`/gboom` game requests a 33 ms ceiling, and the minimal transcript can request
a 16 ms ceiling.

Primary scheduling code:

- `crates/codegen/xai-grok-pager/src/app/event_loop.rs`
- `crates/codegen/xai-grok-pager/src/app/app_view.rs`
- `crates/codegen/xai-grok-pager/src/scrollback/state/mod.rs`

## Animation list

| Animation | Surface and behavior | Cadence or duration | Source |
| --- | --- | --- | --- |
| Welcome logo shimmer | A raised-cosine shine band travels diagonally from the lower-left to the upper-right of the braille logo. A low-amplitude brightness pulse continues beneath it. The animation stops when the logo is hidden. | 12 fps; 4 s shine cycle with about 1.3 s of travel; 5 s breathing cycle | `crates/codegen/xai-grok-pager/src/views/welcome/logo.rs` |
| Running-block accent wave | Brightness travels down the vertical accent rail and its bullet. Pending user-input blocks freeze the rail at a static color so waiting does not look like work. | `0.15` radians per animation tick; wavelength comes from `appearance.animation.wave_rows` | `crates/codegen/xai-grok-pager/src/scrollback/wrappers/entry_renderer.rs` |
| Active turn spinner | A braille spinner prefixes thinking, responding, tool, cancellation, and startup-session status. | One frame per 4 ticks, about 7.5 fps | `crates/codegen/xai-grok-pager/src/views/turn_status.rs` |
| Waiting-on-you pulse | A filled diamond smoothly fades between 30% and 100% of the user or plan accent. It marks permission prompts, questions, drain-blocked turns, and plan approval. | Sine-squared pulse, about 1.31 s per cycle | `crates/codegen/xai-grok-pager/src/views/turn_status.rs`; `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` |
| Background-work monitor pulse | The watcher icon cycles through `○`, `◎`, `◉`, and `◎` while commands, monitors, loops, subagents, or workflows continue after the foreground turn becomes idle. | One frame per 8 ticks, about 3.75 fps and 1.07 s per loop | `crates/codegen/xai-grok-pager/src/views/turn_status.rs` |
| Agent status spinners | Dot spinners mark active tasks and an active goal. A braille spinner reports MCP connection progress. | One frame per 4 ticks | `crates/codegen/xai-grok-pager/src/views/agent_status.rs` |
| Task pane spinners | Dot spinners identify running task groups, rows, subagents, background tasks, monitors, and loops in the task pane. | One frame per 4 ticks | `crates/codegen/xai-grok-pager/src/views/tasks_pane.rs` |
| Workflow spinners | Dot spinners mark an active workflow run and each running workflow agent. | One frame per 4 ticks | `crates/codegen/xai-grok-pager/src/views/workflows.rs` |
| Dashboard work spinners | Dot spinners mark working rows and the aggregate **working** status chip. | One frame per 4 ticks | `crates/codegen/xai-grok-pager/src/views/dashboard/render.rs` |
| Dashboard input blink | A needs-input bullet alternates between full warning color and a 50% background blend. | One phase per 10 ticks; about 0.67 s per full cycle | `crates/codegen/xai-grok-pager/src/views/dashboard/render.rs` |
| Goal detail spinner | A dot spinner marks a running goal in the goal detail view. | One frame per 4 ticks | `crates/codegen/xai-grok-pager/src/views/goal_detail.rs` |
| Session and picker loading spinners | Centered dot spinners cover session loading, deep search, and generic picker content while results load. | One frame per 4 ticks | `crates/codegen/xai-grok-pager/src/views/session_picker.rs`; `crates/codegen/xai-grok-pager/src/views/picker.rs` |
| Extension and workflow loading spinner | A centered braille spinner covers an extension modal during an untargeted pending action. | One frame per 4 ticks | `crates/codegen/xai-grok-pager/src/views/extensions_modal.rs` |
| Plugin setup spinner | A braille spinner precedes **Installing** and **Setting up** in the inline plugin call to action. | One frame per 4 ticks | `crates/codegen/xai-grok-pager/src/app/agent_view/cta.rs` |
| BTW answer spinner | A braille spinner appears beside **Answering…** while the BTW overlay waits for its answer. | One frame per 4 ticks | `crates/codegen/xai-grok-pager/src/views/btw_overlay.rs` |
| Subagent and media loading spinners | Dot or braille spinners appear in the subagent viewer, image viewer, block viewer, and unloaded inline-media placeholder. | Usually one frame per 4 ticks; inline media advances on each current scrollback tick | `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` |
| Mode-switch banner fade | The **Switched to mode** banner remains opaque, then fades its text into the background. | 2 s hold followed by a 9-tick, about 0.3 s fade | `crates/codegen/xai-grok-pager/src/app/agent_view/notices.rs`; `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` |
| Voice recording pulse | The recording glyph alternates between fisheye and bullseye forms while its red color breathes from 40% to 100% brightness. | Wall-clock sine pulse, 0.7 s per cycle | `crates/codegen/xai-grok-pager/src/app/agent_view/mod.rs`; `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` |
| Terminal-title busy spinner | A braille spinner animates in the terminal or tab title while a turn or command is active. | One frame per 8 ticks, about 264 ms per frame | `crates/codegen/xai-grok-pager/src/notifications/title.rs` |
| Terminal-title attention blink | **Action Required** alternates between visible and hidden when the terminal is unfocused. It remains visible without blinking while focused. | 500 ms visible and 500 ms hidden | `crates/codegen/xai-grok-pager/src/notifications/title.rs` |
| Touchpad scroll stream | Fractional high-resolution wheel deltas accumulate and dispatch complete lines on a dedicated clock. The stream flushes residual movement after input stops. | 16 ms redraw clock; 80 ms stream-gap finalization | `crates/codegen/xai-grok-pager/src/input/mouse.rs`; `crates/codegen/xai-grok-pager/src/app/app_view.rs` |
| Selection drag autoscroll | Dragging a selection beyond the scrollback edge moves the viewport by a computed number of rows and re-resolves the selection head each frame. | Animation tick while the drag remains outside the viewport | `crates/codegen/xai-grok-pager/src/app/agent_view/selection.rs`; `crates/codegen/xai-grok-pager/src/scrollback/text_selection.rs` |
| Sticky-header push and fade | A pinned header moves as the following block pushes it away. Its content and selection border fade according to the visible fraction. | Driven directly by scroll position | `crates/codegen/xai-grok-pager/src/scrollback/sticky.rs`; `crates/codegen/xai-grok-pager/src/scrollback/scrollback_pane.rs` |
| Inline video playback | Decoded frames replace the inline terminal image at the video's source frame rate. Playback stops at the final frame instead of looping. | `1 / fps` seconds per frame | `crates/codegen/xai-grok-pager/src/app/app_view.rs`; `crates/codegen/xai-grok-pager/src/app/agent_view/media.rs` |
| `/gboom` game | The hidden command opens a continuously rendered raycaster. Movement and simulation use wall-clock delta time, and rendered PNG frames update the terminal image. | Targets about 30 fps | `crates/codegen/xai-grok-pager/src/gboom`; `crates/codegen/xai-grok-pager/src/app/agent_view/render.rs` |
| Mermaid dashed-edge motion | Generated Mermaid SVG can assign slow or fast infinite dash-offset motion to animated edges. This motion belongs to the generated SVG; a PNG rendered for the terminal is static. | 50 s slow cycle or 20 s fast cycle | `third_party/mermaid-to-svg/src/info_diagram.rs`; `third_party/mermaid-to-svg/src/block_diagram.rs`; `third_party/mermaid-to-svg/src/c4_diagram.rs` |

## Running-block users

The shared running-block accent wave appears on these scrollback entries:

- Thinking
- Command execution
- Web search and web fetch
- Memory search and search tools
- Generic and named tool use
- Background tasks
- Subagents
- Workflows
- Loading recap and session events

The call sites live under
`crates/codegen/xai-grok-pager/src/scrollback/blocks/`. Completed, failed,
cancelled, and user-blocked entries use static accents.

## Time-based changes that are not animations

The app also redraws some state after a deadline without interpolating it:

- Copy, welcome, and ephemeral-tip messages disappear after their time to live.
- Selection highlights disappear after their configured duration unless the
  user enables persistent selection.
- Turn, goal, task, and workflow elapsed-time labels update as time passes.
- Mermaid work polling requests ticks while a background render is pending,
  but the resulting terminal image does not animate.
- Hover, focus, and pressed states change immediately when input changes.
- The text cursor blink comes from the terminal. Grok avoids unnecessary
  redraws so the terminal can preserve that blink.

These behaviors use time or repeated redraws, but they do not create a motion,
fade, pulse, or frame sequence on their own.
