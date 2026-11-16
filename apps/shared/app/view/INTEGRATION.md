# Integrating the chat page

The chat page draws the current conversation over the composer: saved
messages, the turn in flight, its task (header line, tool calls with their
results, the permission prompt with Allow and Deny, the state line, the
result line), the reconnecting banner, notices with Try again or Reconnect,
and Jump to latest. It reads the app core and builds the screen from the app
components (`components/app/`) on the frame context. It never calls the host
and never writes the core.

| File | What it is |
| --- | --- |
| `page.coax` | Load this file. `ChatView`, `view_page`, `view_press`, `view_scroll` |
| `core.coax` | The one place that names the app core: `view_facts`, `view_code`, `view_dispatch`, `view_dirty_take` |
| `model.coax` | The page's rows, built from `DurableChat` and `ViewFacts`. Pure |
| `list.coax` | The virtualized keyed list: only rows near the viewport are measured and painted; the reading position is an anchor row |
| `model-checks.coax`, `page-checks.coax` | Checks, on example data only |

## What to load

```jai
#load "../../shared/app/view/page.coax";   // loads core/core.coax and components/app/task.coax
```

Allocate one zeroed `ChatView` next to the `AppCore`. It holds about 2 MiB
(a decoded message, one Markdown document, the measuring sheet), so keep it
on the heap, not the stack.

## One frame

Inside the root's `uiframe_begin` and `uiframe_end`:

```jai
lay := ViewLayout.{x = 0, top = safe_top, w = w, bottom = composer_bottom,
                   chips_h = chip_row_h, editing = editor_has_keyboard,
                   editor_h = editor_text_h, unit = points_per_unit};
out := view_page(f, v, c, lay);
// the environment and repository chips draw in (out.chips_x, out.chips_y, out.chips_w, chips_h)
// the platform editor goes over out.composer (its text box) while it holds the keyboard
if out.again { /* a row changed height as it was painted: draw one more frame */ }
```

- `bottom` is where the composer's bottom edge docks: over the keyboard when
  it is up, else over the safe area.
- `chips_h` is the height of the chip row the environment picker draws
  (`apps/shared/app/environment/`); 0 leaves no slot. The page only leaves
  the room; it draws no chips.
- Redraw when `view_dirty_take(c)` returns a non-zero set, after a press the
  page handled, after a scroll, while `out.again` is set, and while any
  keyed motion runs (`uiframe_moving`).

Surface capacity for a full page (one generation): about 2048 boxes, 8192
runs of text, 4096 lines, 256 hits, 256 KiB of pool and 512 rows, with 256
keyed ids. The page copies text it paints from reused memory into the pool,
so the pool needs room for every visible message's text. A frame that does
not fit is refused and the last one stays.

## Presses

Route a press through the committed rows, then:

```jai
row := surface_hit(surface, x, y);
p := view_press(v, c, keyed, surface_id(surface, row), surface_action(surface, row));
if p.kind == VIEW_P_DISPATCH { answer := view_dispatch(c, p.what, p.text, mono_ms, wall_ms); }
if p.kind == VIEW_P_FOCUS    { /* give the composer the keyboard */ }
if p.kind == VIEW_P_LOCAL    { /* draw a frame */ }
```

| Press | What `view_press` returns |
| --- | --- |
| The composer's text (`chat.composer`) | `VIEW_P_FOCUS` |
| Send (`chat.composer.send`) | `ACT_SEND`, or `ACT_STOP` while a reply runs |
| Allow, Deny (`task.<turn>.permission.allow`, `.deny`) | `ACT_ALLOW`, `ACT_DENY` |
| The prompt's Try again after an unconfirmed answer (`.retry`) | `ACT_RETRY`: the same request, byte for byte |
| A notice's Try again (`chat.unsure.retry`) | `ACT_RETRY` |
| A notice's Reconnect (`chat.note.retry`, `chat.lost.retry`) | `ACT_RECONNECT` |
| A code block's Copy (`<message>.code.N.copy`) | `ACT_COPY` with the block's text |
| A tool row or its pager (`tool.<turn>.<part>...`) | `VIEW_P_LOCAL`: the row opens, closes or turns a page, and stays where it is on screen |
| Jump to latest (`chat.jump`) | `VIEW_P_LOCAL`: the list follows the end again |

`view_dispatch` builds the `AppEvent` and calls `app_dispatch`. A logged-out
Send answers `APP_LOGIN`: show the login flow (`apps/shared/app/login/`).

A drag scrolls: `view_scroll(v, dy)`, positive toward the latest message. A
scroll away from the end stops following it; new rows then arrive without
moving what the person reads.

Draft edits come from the platform editor, not from the page: dispatch
`ACT_DRAFT` with the whole text, as the core's README says.

## Services

None. Every effect goes through the core's outbox.

## Checks

```sh
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/view/page.coax
coax/remote/remote.sh run -- coax/bin/coax test apps/shared/app/view/list.coax
```
