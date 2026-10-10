# Desktop image draft verification

Issue: [#10011](https://github.com/OpenAgentsInc/openagents/issues/10011).

This slice reimplements Zeron's image-draft design in Rust Native. Shared Rust
owns decoding, draft limits, removal, and the hosted-route refusal. Native
adapters provide a file picker, clipboard pixels, and file-drop events.
No Zeron source or GPUI runtime is copied.

## Behavior

PNG and JPEG inputs are decoded on an input worker, with limits of 8 MiB per
image and 4096 × 4096 pixels. A conversation holds at most four images; all
conversation drafts together hold at most 16 MiB of encoded images. Previews
hold at most 96 × 60 pixels. Only the basename is retained. Images stay in
memory until removed or the app exits; they are not saved or uploaded.

Imports bind to the conversation that started them. Switching chats cannot
attach an image to another chat. Clipboard text uses the composer's revision
stamp, so delayed input cannot overwrite a newer edit. The file picker runs
outside the rendering thread. Linux uses the desktop portal; clipboard support
includes X11 and Wayland. Unsupported clipboard or picker paths report a reason.

The current hosted NIP-CJ conversation contract carries text only. Send with
an attached image preserves the images and caption and explains: “Hosted chat
accepts text only. Remove the images to send.” Removing the last image enables
the existing text send. Suggestions use the same refusal.

Later the same day [#10070](https://github.com/OpenAgentsInc/openagents/issues/10070)
replaced that refusal: a send with images sends only its words to the router
and keeps the images in the draft, bound to that message. A coding reply's
Coder start (at once, or **Run Coder**) carries them; any other reply keeps
them and says images go only to Coder. The refusal remains only on routes
that carry words only (a running Coder task's chat, a computer's own thread),
with the line "This message can't carry images. Remove them to send it;
images go to Coder when it starts."

On 2026-10-01 [#10093](https://github.com/OpenAgentsInc/openagents/issues/10093)
turned attachments off on the phone: its chat is text only, with no attach
control or photo picker, behind `coder_tab::ATTACHMENTS_ENABLED`. The same day
[#10095](https://github.com/OpenAgentsInc/openagents/issues/10095) turned them
off on the desktop behind the same switch: no attach control or image picker,
a pasted or dropped image is dropped, and a draft sends its words only. The
behavior on this page is kept in code and returns when the switch is turned
back on.

On 2026-10-10 [#11174](https://github.com/OpenAgentsInc/openagents/issues/11174)
turned the switch back on: the phone and the desktop show their attach control
again and the behavior on this page applies. The tests and the `ui-no-attach`
gate that check the text-only mode turn the switch off for themselves.

## Checks

On macOS 26.4, M5 Max, Rust 1.97.1:

- Targeted formatting and strict all-target Clippy passed.
- Shared chat application: 72 tests passed, including PNG, JPEG, malformed and
  oversized input, pixel validation, draft bounds, and no silent image loss.
- Desktop library: 81 passed; binary: 27 passed and 4 opt-in checks ignored;
  pairing: 3 passed. New tests exercise dropped files, switching conversations
  during import, stale clipboard text, four previews, removal, and refusal.
- Native adapter: 47 passed.
- Phone Rust consumer: 142 passed; 19 opt-in checks ignored.
- Release build passed. Dependency review adds only two version-specific
  Boost license exceptions for Windows clipboard helpers. The dependency gate
  reports the same 29 unrelated errors as the preceding slice.
- The real macOS file picker selected a generated temporary PNG in an isolated
  fake-host app. Its preview appeared; Send preserved the caption and image and
  displayed the text-only reason; Remove preserved the caption. The fixture
  app was closed afterward. No owner host or saved chat was used.

The scripted [default capture](default.png) and [minimum-size capture](minimum.png)
show four images and all Remove controls. Actual system image clipboard and
Linux portal checks are recorded in `NEEDS_OWNER.md`; they do not block code
completion.

## Timing

[The native run](native.json) repeats the 3,300-row, 500-chat fixture at
1200 × 840 points, 2× rendering, with the Grid backdrop. Each active phase has
115 samples. These are application work plus CPU frame submission, not GPU
completion or display latency.

| Phase | p50 | p99 |
| --- | ---: | ---: |
| Scroll | 2.213 ms | 2.588 ms |
| Streaming | 6.498 ms | 7.588 ms |
| Sidebar | 2.291 ms | 3.098 ms |

Idle CPU was 3.20%; peak RSS was 202.8 MiB. Image decoding and thumbnailing
never run in these frame callbacks.
