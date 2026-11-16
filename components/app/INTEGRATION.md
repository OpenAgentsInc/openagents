# Integrating the app set

## A screen that uses the components

Load the files you use; each loads what it needs:

```jai
#load "../../../components/app/task.coax";      // base and chat too
#load "../../../components/app/account.coax";
#load "../../../components/app/nav.coax";
```

A screen runs one frame as `apps/openagents-ios/chat.coax` does: a
`UiSurface` with two generations, a `KeyedStore`, then `uiframe_begin`, the
components, `uiframe_end`. Declare capacity for what the screen shows; the
gallery's (`AG_*` in `gallery/gallery.coax`) holds every component at once.

Keep the keyboard rule: right after `uiframe_begin`, store
`uiframe_focus(f)` as the screen's focused ID. When it becomes empty the host
hides the keyboard (`uiframe_keyboard(f)` is false). At touch down, also call
`uiframe_press_focus(surface, input)` so a tap shorter than a frame is not
missed. The gallery does both (`appgallery_frame`, `appgallery_pointer`).

Services: none. The components never call a host.

## The gallery in the iOS dev menu

The gallery is a page of its own, called as the chat page is:

| Call | What it does |
| --- | --- |
| `appgallery_env(s, safe_top, safe_bottom, reduced)` | the window's facts, in points |
| `appgallery_frame(s, out, width, height, dpr, dt) -> int` | one frame in device pixels; the vertex count |
| `appgallery_pointer(s, x, y, phase) -> bool` | a touch in device pixels (0 down, 1 moved, 2 up, 3 cancelled); a drag scrolls |
| `appgallery_action(s) -> int` | once: 1 asks the host to close the dev menu |
| `appgallery_moving(s) -> bool` | whether another frame is due (always, unless motion is reduced) |
| `appgallery_max_vertices()`, `appgallery_scale()` | the vertex buffer to give it, points a layout unit |
| `appgallery_activate(s, id)`, `appgallery_drive(s, op, out)`, `appgallery_rows_stamp(s)`, `appgallery_refused(s)` | VoiceOver, `coax drive` (`tree`, `counts`) and health |

The native side lives in the iOS app directory, owned by `claude-ios-host`;
see `apps/openagents-ios/DevGallery.swift` and the steps in its header.
