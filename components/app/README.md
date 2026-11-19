# The app set

GitHub #13645 (for #13620). The components the native apps' screens are
built from, drawn on the GPU by the shared kit. Each is one procedure on the
frame context (`components/kit/frame.coax`): it takes the frame, a stable
ID, plain inputs and a position, paints in the skin's roles and measures
(`skin.app`, `UiApp` in `components/skin/skin.coax`), records its semantic
row and hit region under its ID, keeps what outlives a frame in the keyed
store, and moves with the kit's motion. A component never calls a host and
never changes app state: a press reaches the app as the row's action.

| File | Components |
| --- | --- |
| `base.coax` | text styles (`app_text`), buttons (`app_button`: default, primary, quiet, destructive; busy and disabled), `app_icon_button`, `app_chip`, `app_field` (plain, secure, error, disabled), `app_notice`, `app_divider`, `app_card`, `app_sheet`, `app_menu_row`, `app_switch`, `app_progress`, `app_ring`; helpers `app_fit`, `app_id`, scroll anchoring (`app_anchor_take`, `app_anchor_scroll`) |
| `chat.coax` | `app_composer` (Send, Stop while a reply runs, locked), `app_composer_chips` (environment and repository), `app_message_user`, `app_message_agent` (Markdown through `kit/markdown.coax`; `app_message_agent_linked` makes inline code that names a file a link), `app_code` (the language by name and Copy, as Zeron's), `app_typing`, `app_empty`, `app_retry`, `app_jump`, `app_meta` |
| `task.coax` | `app_task_header`, `app_task_state`, `app_tool` (opens onto paged input and result), `app_tool_group`, `app_tool_press`, `app_permission` (Deny, Allow, sending, uncertain, answered, expired), `app_task_result`, `app_banner` (reconnecting, offline) |
| `diff.coax` | a unified diff read into files and hunks (`app_diff_files`, `app_diff_step`, `app_diff_stats`) and drawn on Zeron's measures: `app_diff_view`, `app_diff_hunks` (both line numbers, the sign, add/remove washes), `app_diff_file_row`, `app_diff_head`, `app_diff_press` |
| `account.coax` | `app_logged_out_card`; the login card is the login flow's `lf_card` (`apps/shared/app/login/card.coax`), and the gallery shows it |
| `identity.coax` | Public identity, copy, backup/rotate/restore confirmations, native private-field slot, unresolved-change retry/discard; no private bytes or account effects |
| `followups.coax` | the follow-ups tray over a composer (`app_fu_tray`: waiting steers, steers not taken, the queue with Move up, Edit, Remove and Steer or Send now, a paused head with Resume and Clear; `app_fu_op` reads a press back) and `app_message_steer` (a steer the reply took in, in the timeline) |
| `nav.coax` | `app_nav_button`, `app_side_head`, `app_side_row`, `app_side_list`, `app_side_window` (virtualized, single-line history) |
| `settings.coax` | Zeron's desktop settings screen parts in the skin's `UiSettings` measures (`skin.set`): `set_nav_head`, `set_nav_row`, `set_search`, `set_account_row`, `set_nav_edge`, `set_column`, `set_page_head`, `set_section_head`, `set_row` (plain, link, button, stepper, choice), `set_group`, `set_button`, `set_member_card` |
| `ioshost-sidebar.coax` | the phone sidebar that pushes the page: `app_sidebar`, `app_sidebar_shift`, `app_sidebar_ease`, `app_sidebar_head`, `app_sidebar_item`, `app_sidebar_rule`, `app_sidebar_new`, `app_sidebar_avatar`, `app_initials` |
| `gallery/gallery.coax` | every component above on one scrolling page, with example data; the iOS dev menu opens it |

One call:

```jai
h := app_button(f, "chat.retry", x, y, w, -1, "Try again", APP_BUTTON_PRIMARY, 0, ACT_RETRY);
y += app_tool(f, tool_id, x, y + h, w, AppTool.{icon = ICON_TERMINAL, title = "Ran a command", arg = "ls", state = APP_TOOL_DONE, input = "ls", output = "a\nb"}, false, ACT_TOOL);
// on a press: if app_tool_press(keyed, tool_id, pressed_id) { /* the row opened or turned a page */ }
```

Rules the set keeps:

- The inputs are plain records (`AppTask`, `AppTool`, `AppPermission`,
  `AppComposer`, `AppSideRow`); a screen copies what it shows
  into them. Nothing here loads the task protocol, a service or app state.
- The mono skin shows state by brightness and line, never hue.
- Text a component makes up (a cut label, a count) is kept in the sheet's
  pool, so a surface declares pool bytes for it.
- One clip at a time: a tool row's reveal holds it, so a code block and a
  long line are cut with an ellipsis, not clipped.
- Focus follows a press (`kit/frame.coax`): a press on a field takes the
  keyboard, any other press gives it up.

Another area that needs a component the set lacks adds it as a new file
here with its area's prefix (`view-*.coax`, `login-*.coax`, `env-*.coax`).

Checks: `coax/bin/coax test components/app/task.coax` (it loads chat and
base), `account.coax`, `nav.coax`, `settings.coax`, `gallery/gallery.coax`, `together-check.coax` (the set loads beside the shared app core, `apps/shared/app/core/core.coax`: no top-level name may clash with one there); and
`coax/web/tool.sh components-check`. Run them on a build host
(`coax/remote/remote.sh run -- ...`). How a host shows the gallery is in
[INTEGRATION.md](INTEGRATION.md).
