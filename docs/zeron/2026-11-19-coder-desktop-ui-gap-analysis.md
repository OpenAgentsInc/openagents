# Coder Desktop and Zeron: UI gap analysis

2026-11-19. This analysis compares the Coder Desktop app (`apps/coder-desktop`,
OpenAgents `main` at `7f6a973d51`) with Zeron's desktop app (`apps/zgx` in
`zeronsh/zeron`, local clone `~/work/projects/repos/zeron` at
`e393c4a84009df122bf048909a653c21efc9437d`). The goal is to bring Coder
Desktop in line with Zeron's UI patterns, drawn by GPU components that match
Zeron to the pixel.

This is a source comparison. No side-by-side captures were taken for this
document. Each finding cites the file that sets the value, so it can be
checked against a capture later.

## What we already documented

| Document | What it covers | What it leaves open |
| --- | --- | --- |
| [README.md](README.md), [features.md](features.md), [architecture.md](architecture.md), [audit.md](audit.md) | Zeron's repository, features, engine and edge architecture, and a code audit | They describe the engine and edge. They don't give UI measures. |
| [ported-components.md](ported-components.md) | Zeron components already in Coax (`components/zeron/`, `components/skin/zeron.coax`) and their use in `apps/openagents-desktop` and `/chat` | Coder Desktop isn't covered. The skin was ported from Zeron's older GPUI theme (`crates/theme/src/builtins.rs`), not from the zgx theme Zeron ships now. |
| [../research/zeron/2026-11-14-current-gap-analysis.md](../research/zeron/2026-11-14-current-gap-analysis.md) | Engine, session, device and account gaps | It excludes `apps/zgx`, so it has no UI findings. |
| [../coder/activity-groups/README.md](../coder/activity-groups/README.md) | Activity groups (tool rows) next to Zeron, with captures | Lists remaining differences: no 12pt face, loader, per-character fades, pane rows. |
| [../coder/settings/README.md](../coder/settings/README.md) | Settings built to Zeron's settings measures | Coder captures only. |
| [../coder/claude-desktop-parity-process.md](../coder/claude-desktop-parity-process.md) | The earlier rule: Claude Desktop's structure, Coder's own monochrome palette | `7f6a973d51` moved the palette and composer to Zeron Dark. The sidebar still uses Claude Desktop's proportions. This document replaces that rule for Coder Desktop. |

## Summary

Coder Desktop already draws most of its UI on the GPU through Coax, and
several parts are close to Zeron: the activity groups, the user bubble, code
blocks, settings and the pill composer's colours. The biggest gaps are not
individual widgets. They are the following:

1. **Scale.** The page lays out in units drawn at 1.2× (`ARCADE_PHONE_SCALE`).
   Values written as `N/u` come out at N pt. Bare constants and every font
   size come out 20% larger. So prose is 14.4pt on a 24pt line, tool text is
   13.2pt, and the transcript column is 864pt wide. Zeron uses 15/24 prose,
   12/16 tool text and a 660pt column.
2. **Two token sources.** The skin starts from `arcade_phone_skin()` and adds
   Zeron values on top. The composer hard-codes zgx colours (`#ededef`), the
   rest of the page uses the GPUI-era Zeron palette (ink `#e8e8ea`, accent
   `#8b7cf6`), and the sidebar uses its own `#16171a`. Zeron ships one theme
   file, `zeron-dark.json`.
3. **Native pieces inside the GPU UI.** These are the opaque AppKit title bar,
   the NSTextView composer overlay, NSAlert dialogs and NSMenu context menus.
   Zeron draws all of these itself. It uses native views only for terminals
   and web views.
4. **Renderer quality.** Coder's renderer has no blur, so shadows are faked
   with fills. It redraws the full frame, runs a 60 Hz timer, and has no
   scroll momentum, overscroll or scrollbars. Its glyphs come from an unhinted
   TTF rasterizer. Zeron has backdrop blur, real shadows, damage regions,
   display-linked frames at 120 Hz, elastic scrolling and a cosmic-text and
   swash atlas.
5. **Missing surfaces.** Coder has no command palette, session tabs, panel
   tabs (files, terminal, browser, diff), composer deck (queue and running
   items), composer footer chips (access, project and branch, context ring),
   mentions, attachments, dictation, in-composer approvals, toasts or
   "Jump to latest" pill.

Fix 1 and 2 first. They change every measure, so captures taken before them
won't hold. Then fix 3 and 4, then build the missing surfaces on the
corrected base.

## 1. Rendering model

| Concern | Zeron (`apps/zgx`, zgx framework rev `b8f298b6`) | Coder Desktop | Gap |
| --- | --- | --- | --- |
| GPU backend | wgpu 29. Rounded quads, borders, shadows, gradients, backdrop blur, SVG, images and custom WGSL fills, merged into instanced draws | One `CAMetalLayer` (`apps/shared/host/apple/ui-host.m:1155`). One vertex buffer per frame from `coder_page_frame`, at most 200,000 vertices (`page/state.coax`), using `components/backend/gpu-shader.coax` | No blur primitive and no shadow primitive. Add both to the shared shader so composer, menus and dialogs can use Zeron's real elevation. |
| Redraw | Rectangular damage regions on whole pixels. More than 64 regions become one full update. Output texture kept between frames; subtrees culled by bounds | Full redraw on each presented frame. A frame is skipped when the vertex digest doesn't change (`ui-host.m:1236-1269`) | Full redraws are fine at today's sizes. Add damage regions before long transcripts and the panel arrive. |
| Frame pacing | CVDisplayLink per display. The refresh period is measured, so 120 Hz is detected. 250 ms watchdog | 60 Hz `Timer` on the main run loop (`Window.swift:199`), with 3 settle frames after a change | Motion runs at half rate on ProMotion displays. Move to CVDisplayLink (or `CADisplayLink` on macOS 14+). |
| Text | Vendored cosmic-text 0.19 with harfrust shaping and swash rasterization. Below 2x, glyphs snap to whole pixels and coverage is firmed ×1.25. At 2x, glyphs keep subpixel positions. Grayscale masks | Coax rasterizer (`components/raster/atlas.coax`, `ttf.coax`). No hinting, one dilation level, GPOS kerning only, no colour glyphs. Rasterized at backing scale × 1.2 × zoom | Match Zeron's rule: snap and firm below 2x, subpixel positions at 2x. Compare glyph coverage against a Zeron capture at 1x and 2x. Add colour glyphs for emoji. |
| Composer text | zgx's own GPU editor, with undo, spellcheck, bidi and IME (`zgx-desktop/src/native_ime.rs`). The placeholder follows IME composition | `KitMacEditor`, an NSTextView placed over the GPU composer every tick (`Window.swift:357-367`, `apps/shared/native/KitEditorMac.swift`) | Two text renderers on one surface: CoreText in the field and the Coax atlas elsewhere, so weight and spacing differ. Move the composer to `components/zeron/editor.coax` (`UiEditor`), which already has undo, selection and IME composition, with an NSTextInputClient bridge. |
| Title bar | Drawn by the app. 38pt tall with 4pt top padding; controls centred at y=20. Traffic lights at x=21/44/67, then sidebar toggle at x=88 and back/forward. With the sidebar collapsed, a glass island sits behind the controls (`ui/ds/title_bar.luau`, `screens/shell/page.luau:47-76`) | Standard `.titled` window without `fullSizeContentView`, so an opaque native bar of about 28-31pt sits above the canvas (`Window.swift:142-146`, `CP_TITLEBAR_PT 28`) | Use `fullSizeContentView` with a transparent title bar. Move the traffic lights to Zeron's positions. Draw the toggle and back/forward in Coax. |
| Scrolling | Custom momentum (quartic ease-out, K=1000 pt/s⁴, at most 2 s, whole points). Rubber-band overscroll (1/20 of the push) with springback; mouse wheels don't stretch. Overlay scrollbars with no track | Raw deltas, clamped (`page/input.coax:277`). Mouse lines ×22.75 (`ui-host.m:189-196`). No overscroll, no easing, no scrollbar | Port `coast` and `elastic` into a shared Coax scroll module (`components/core` or `components/kit`), and add overlay scrollbars. |
| Native views | Ghostty terminals (macOS) and web views for the panel's browser tabs only | NSAlert dialogs (`Window+Dialogs.swift`, `Window+Update.swift`), NSMenu context menus (`Window+Menus.swift`), NSOpenPanel, menu bar and status item | Draw dialogs and context menus in Coax (see section 6). Keep NSOpenPanel, the menu bar and the status item native, as Zeron does. |

## 2. Units and scale

Coder's page multiplies every bare unit by 1.2 (`page/skin.coax:4`,
`components/skin/arcade.coax:26`). Zeron works in points. Writing some values
as `N/u` and leaving others bare causes the drift below.

| Measure | Zeron | Coder now | Source |
| --- | --- | --- | --- |
| Prose | 15/24 | 14.4pt on a 24pt line (body 12u, line 20u) | `page/skin.coax:55` |
| Code block text | 13/20 | 14.4pt on 24pt (code 12u) | `page/skin.coax:57` |
| Tool rows, labels | 12/16 (sm), 13/18 (md) | 13.2pt (11u) | `page/skin.coax` |
| Composer text | 14/22 | 14.0pt on a 20pt line (NSTextView) | `page/checks/layout-checks.coax` |
| Conversation column | 660 (settable 560-1200 in steps of 20) | 864pt (`CP_COLUMN 720`). The comment calls this Zeron's `prose.measure`, which is 660 | `page/state.coax:54`; Zeron `ds/tokens.luau:344` |
| Page margin | 32 side room for the composer, 8 bottom gap | 19.2pt / 9.6pt (`CP_PAD 16`, `CP_BOTTOM 8`) | `page/state.coax:41` |
| Sidebar width | 272 default (242-400) | 280pt (`CP_SIDE 234`), 216-576pt drag range | `page/state.coax:45` |
| Right pane | 520 default, at least 360 | 528pt (`CP_PANE 440`), 384-1080pt | `page/state.coax:23,47` |

**Fix:** set Coder Desktop's layout scale to 1.0 and write every page value in
points. Keep text zoom (⌘+ / ⌘-) as a separate multiplier, as it is now. Add
a check that each font slot comes out at Zeron's size at zoom 1.0. The
settings faces already bypass the scale (`page/host-api.coax:58-66`); after
this change that workaround can go.

## 3. Tokens

Use one Zeron Dark source taken from `apps/zgx/ui/ds/themes/zeron-dark.json`
and `ui/ds/tokens.luau`. `components/skin/zeron.coax` is still the GPUI-era
palette. Update it in place, so `/chat`, `apps/openagents-desktop` and the
derived skins move with it. Golden hashes will change and must be
re-recorded in the same change.

| Token | Zeron zgx | Coder now | Action |
| --- | --- | --- | --- |
| Window | `#0e0e10` | canvas `#0d0d0d` (`zeron.coax:88`) | Update `zeron_dark()`. |
| Page and panel cards | inset cards: surface.raised `#ffffff0f`, border.subtle `#ffffff0a`, radius 12, gap 4 | Flat canvas, no card | Draw the transcript and panel as inset cards on the window colour. |
| Sidebar | Glass over the window. Selected row `#ebebeb24`, sidebar tint `#ebebeb0d` | Opaque `#16171a` (`page/skin.coax:14`) | Remove the override and use the zgx sidebar roles. |
| Text tiers | primary `#ededef`; secondary, tertiary, disabled at alpha 0.6 / 0.42 / 0.28 | ink `#e8e8ea`, muted `#a9a9ae`, faint `#85858a`. The composer alone hard-codes `#ededef` with its own alphas (`conversation.coax:329-330, 492-494`) | One set of text roles in the skin. Remove the hard-coded values from `conversation.coax`. |
| Borders and washes | border `#ffffff12`, subtle `#ffffff0a`, strong `#ffffff2e`; hover `#ffffff0d`, pressed `#ffffff17`, selection `#ffffff1a` | hairline white 10%, hover white 8%, raised and hover overridden to opaque greys (`page/skin.coax:16-17`) | Use the zgx values. |
| Accent | `#9277b0` (hover `#9e85bb`, active `#83699f`). Focus ring is accent at 0.6, 2pt wide, 1pt offset | `#8b7cf6` | Update. Use the accent for the caret and focus ring, as Zeron does. |
| Status | danger `#f87171`, warning `#facc15`, success `#34d399`, info `#60a5fa`. Agent states use palette 400 steps | warning `#fbbf24`; the others match | Update warning. Add agent-state roles (connecting, input, approval, plan, done, failed, limited). |
| Primary button | bg `#ececee`, hover `#fff`, active `#d2d2d7`, text `#0b0b0c` | Send button `#ededef` / `#ffffff` / `#d4d4d8`, icon `#141416` | Use the zgx values. |
| Composer | tint `#08080826`, solid `#18181b`, edge `#c7d2de17`, shadow `0 12 32 #00000066` | `#18181b`, `#c7d2de17`, shadow faked with two fills (`conversation.coax:500-502`) | Real shadow once the shader has one; then add the frosted tint. |
| Menus | solid `#1c1c1f`, shadow `0 20 56 #00000052`, blur 16. Popover `#2a2a2f` | Model popover `#1b1b1c` at 96%, shadow black 35% offset 2 | Use the zgx values, with blur. |
| Shadows | sm `0 1 2 #00000059`, md `0 4 12 #00000066`, lg `0 12 32 #00000080`; dialog `0 24 64 #00000080` | None | Add shadow tokens with the shader primitive. |
| Type ramp | Geist and Geist Mono. xs 11/14, sm 12/16, md 13/18, lg 15/20, xl 20/26, xxl 28/34. Prose 15/24, h1 22/30, h2 18/26, h3 15/24. Code 13/20, inline code 13.5 with radius 4.5. Medium is the main UI weight | Slots sized by `FONT_SIZES` × 1.2. There's no 12pt face, which `docs/coder/activity-groups` names as its largest remaining difference | Add the full ramp as font slots in points. Add Geist Bold, Italic and Mono Medium to the atlas. |
| Radii and spacing | Spacing 4, 8, 12, 16, 24, 32, 48, 64 (half steps 2, 6, 10). Radii 4, 6, 8, 12; composer 26; dialogs 16 | Mixed: composer 20, code block 10, tool rows 10, chips 8 | Use Zeron's scale. Set the composer to 26 and code blocks to 12. |
| Motion | Ease `(.25,.1,.25,1)`. Fade and hover 150 ms. Popovers open 250 / close 200. Panel 280 ms. Dialogs 200 in / 130 out. Composer 220 ms. Springs for tabs. Reduce Motion everywhere | Message entrance 120 ms with a 3pt rise. Loaders honour Reduce Motion | Add a shared motion module with Zeron's durations, easings and springs. Use it for every surface below. |

Zeron fails a build when any hard-coded colour, spacing, radius, type size or
shadow appears (`tests/design_lint.rs`, `ui.theme.strict`). Add the same
check for Coder Desktop's page: no literal `rgba(` or `hex(` outside the skin
module.

## 4. Window layout

| Area | Zeron | Coder now | Gap |
| --- | --- | --- | --- |
| Shell | Window colour, a glass sidebar, and the page and panel as inset cards (radius 12, gap 4) | Flat canvas. Sidebar, centred column and optional right pane (`apps/shared/app/view/shell.coax`) | Add the card treatment. |
| Sidebar top | New chat (⌘N), Search | New chat, then a fixed Foreman row with its tier at the right | Add Search, which opens the command palette scoped to sessions. Keep Foreman as a pinned item. |
| Sidebar rows | 30pt compact, 46pt two-line (17pt title line plus 14pt detail). Section title 24, list gap 2, inset 8 | 32pt, 48pt, header 26, inset 8, radius 8 (Claude Desktop proportions) | Move to Zeron's 30/46/24 and gap 2. |
| Row contents | Harness mark (14), status icon (14) or dot (6), time, PR badge, subagents pill (5pt working dot plus count). ⌘-held shows ⌘N hints. Hover cards. Drag reorder over 150 ms | Title, " · F" for managed chats, ring loader when busy, accent unread dot. Drag reorder for pinned rows | Add the model mark, status icon set, relative time, subagent count pill and ⌘ hints. Replace " · F" with a mark. |
| Grouping | Pinned, your own sections, then projects or devices (sorted by last updated or created). 5 rows per group, then "Show N more". Group tree with 1pt hairlines and 6pt elbows. Project tiles show their most urgent state. Archived | Pinned and Recents. At most 50 chats, then "N older chats are not shown." | Group by project (working folder) with "Show N more". Add Archived. Lift the 50-chat limit with a virtualized list (Zeron is tested at 1,000 sessions). |
| Sidebar footer | Settings | Update pill and account row | Keep the update pill. Match Zeron's footer row metrics. |
| Session tabs | 42pt top bar, tabs 112 wide with gap 3 and 11.5pt text (`screens/shell/session_tabs.luau`) | None. ⌘1-9 jump to chats | Optional. Decide whether Coder wants tabs; ⌃Tab cycling is worth adding either way. |
| Top corners | No cost display | Cost chip "$3.85 / $50.00", goal chip, copy-chat button | Keep the cost and goal chips; they're Coder features. Style them as Zeron footer chips and move them under the composer, next to the context ring. |
| Split chats | Panes at least 288 wide; the unfocused pane is dimmed to 0.4 (`screens/shell/split.luau`) | None | Later. |

## 5. Composer

| Part | Zeron (`ui/ds/composer.luau`, `docs/composer.md`) | Coder now (`cp_zeron_composer`, `conversation.coax:437-548`) | Gap |
| --- | --- | --- | --- |
| Shape | Frosted pill, radius 26, 1pt border | Solid pill, radius 20, 1pt border, faked shadow | Radius 26, real shadow, frosted tint. |
| Compact mode | 49pt tall (47 inner). 22pt text line between a 28pt round + button and 32pt trailing controls (mic, Send/Stop) | Text plus a bottom strip with the folder chip, model chip and a 28pt Send | Restructure: + on the left, text in the middle, mic and Send on the right, all on one line. |
| Expanded mode | Forced on a new chat and with attachments, a question or dictation. Text box 76-260 (viewport 56-240), then scrolls. Insets 16 at the sides, 16 top, 4 bottom. Contracts with 32pt hysteresis; animates over 220 ms `out(3)` | Grows from 1 to 8 lines (160pt) with no animation | Port the two modes, hysteresis and animation. |
| Caret and placeholder | Accent caret, 2pt. "Do anything…" | NSTextView caret. "Ask anything..." | Comes with the GPU editor. Use Zeron's placeholder text, which is copy we're asked to match. |
| Footer chips | Below the pill: access (Supervised / Auto-accept edits / Full access) and a Plan switch; where (project › branch, device, checkout or worktree); model (⌘/); context-usage ring with a hover card. Folds when narrow | Folder chip and model chip inside the pill | Move the chips below the pill. Add the context ring (we have token counts), access mode and branch. Add the cost and goal chips here. |
| Deck | Grows up from the pill as one surface. Tabs: Queue, Running (tasks, terminals, subagents), Plan/todos. Spring tab thumb; header and rows 32, body at most 216. Queue rows: grip, thumbnails, first line, Edit, Remove, and Send now / Steer / Send next. Drag or ⌥↑/↓ to reorder | The steer, queue and interrupt plumbing exists (`page/followups.coax`, `Window+FollowUps.swift`). Nothing is drawn (#13717) | Build the deck with the Queue tab first, on the existing follow-up state. Then the Running tab, listing delegated subagents. |
| Mentions | `@` files, `/` commands, `$` skills. Popover at the caret. Tokens styled by type, not chips | Only `/goal`, parsed as text (`page/goal.coax:190`) | Add the mention popover and token styling. Route command and file matching through the shared selector rules in `INVARIANTS.md`, not ad hoc string matching. |
| Attachments | Chips 40 tall, glyph 18, thumbnails 56, at most 214 wide | None (`components/zeron/attachments.coax` exists unused here) | Wire `attachments.coax` into the composer. |
| Dictation | ⌘D. A track unrolls with "Listening", a wave and a clock | None (`apps/openagents-desktop/zeron/dictation*.coax` exists) | Reuse the openagents-desktop dictation guard. |
| Approvals and questions | Inside the pill. Options 36pt, keys 1-9 pick, a ring marks the choice. Deny / Allow / Always allow | Budget and goal use NSAlert sheets | Draw questions and approvals in the pill. |

## 6. Transcript, panel and other surfaces

### Transcript

| Part | Zeron | Coder now | Gap |
| --- | --- | --- | --- |
| List | Virtualized, row estimate 96. Turn gap 16, seam gap 6 | Rows outside the view plus a 200-unit margin are skipped (`conversation.coax:93`). A 24-message cap notice | Virtualize fully and remove the 24-message cap. |
| User bubble | Right-aligned, at most 80%, padding 16×10, radius 16, `#ffffff14`. Folds past 5 lines with a 1.5-line fade and "Show more". Thumbnails 120×84 | Same padding and radius; 85% share (`page/skin.coax`), raised fill. No fold | Set 80%, the zgx fill, and the 5-line fold. |
| Assistant text | Markdown at 15/24, no bubble. Streaming "veil": text is laid out at once and fades in by colour over 120-400 ms, paced by arrival speed, so nothing reflows | Markdown at 14.4pt. No veil | Fix the size (section 2). Add the veil to `components/kit/markdown.coax`. |
| Code blocks | Radius 12, primary at 0.04, subtle edge, concentric Copy | 32pt header strip with language and Copy, radius 10 | Radius 12, the zgx fill, and a concentric Copy without a header strip. |
| Rich content | Tables, Mermaid, LaTeX, pictures (at most 660×420, radius 12) | Tables. No Mermaid, LaTeX or pictures | Pictures first, since agents produce screenshots. |
| Activity | "Ran 3 commands · read 2 files", settled as "Worked for {secs}s · {summary}". 16pt loader, hairline trunk with 6pt elbows, glyphs at x=24, gutter 48, rows 28, header 26. Rows enter over 360 ms `outExpo`; fold 200 ms | The same measures and behaviour (#13731) | Use "Worked for {secs}s" once settled. Add the row entrance and fold animation. |
| Tool detail | Diff view. Mono output on the code wash, first 10 lines then "Show all N lines". A failure shows its exit code with a danger edge. Output trimmed to the last 32 KB / 400 lines | Danger glyph on a failed row; no detail view | Add the detail view with diffs and exit codes. |
| Working row | Loader plus a rotating word (every 7 s) and an elapsed clock. "Sending…" before the turn starts | "Pondering… 9s" | Add the "Sending…" state. |
| Scroll | Pinned to the end while resting there. A "Jump to latest" or "New reply" pill appears away from the end. Sending glides over 380 ms. Rows measured above the view don't shift it | Sticks to the bottom | Add the pill, the glide and anchoring. |
| Selection | Across messages, with Mention and Attach | Copy buttons only | Add cross-message selection. |

### Right panel

Zeron's panel (`screens/panel/init.luau`) is a tabbed workspace: at least 360
wide, 520 by default. Tab chips are 28 tall with labels at most 160 and a
fade. Tabs can be dragged, and closed with Close Others, Left and Right. The
empty panel offers Browser (B), Files (F), Terminal (T), Side chat (S) and
Fork (K), plus run actions and lanes (side chats and subagents). Git changes
and diffs live there too.

Coder's right pane shows one delegated subagent's transcript. Its rows still
use the older `app_tool` style ("Ran a command", "Failed") instead of the
activity group. Its header shows the level name, the time taken and a Close
button.

Steps:

1. Draw the subagent pane with the same activity group as the main
   transcript (`docs/coder/activity-groups` lists this as open).
2. Give the pane a tab strip, so several subagents can stay open.
3. Add a Changes tab for the working folder's git diff.
4. Files and Terminal come later. A terminal is native in Zeron too (Ghostty),
   so it's the one panel tab that may stay a native view.

### Other surfaces

| Surface | Zeron | Coder now | Gap |
| --- | --- | --- | --- |
| Command palette | ⌘K. Width 640, 88 from the top, opens over 180 ms. Scopes All, Sessions, Commands, Files, Settings. Pinned and Recent; drill-in pages with ⌫ to go back | None | Build it. It also serves sidebar Search. Selection goes through the shared semantic selector. |
| Menus | Rows 28, headings 24, width 232. Virtualized dropdown rows | Native NSMenu for context menus; Coax model popover with 42pt rows | Draw context menus in Coax at Zeron's measures. |
| Dialogs | sm 380, md 460, lg 560, sheet 400, radius 16. In 200 ms, out 130 ms | NSAlert sheets for budget, goal, keys and updates | Draw dialogs in Coax with GPU text fields. |
| Toasts | 340 wide, at most 4, bottom corner, menu glass, pause on hover | None | Add them for update, Foreman and error notices that don't belong in the transcript. |
| Settings | Column 640, rows 52, control width 240, grouped cards. Many pages | Zeron's measures, five pages (`settings.coax`); search is a native editor overlay | Swap the search field to the GPU editor. Add Appearance (conversation width, text size) and Shortcuts. |
| Empty state | Composer in expanded mode as the page's centre | "What do you want to do?" | Use Zeron's expanded composer for new chats. |
| Shortcuts | ⌘N, ⌘K, ⌘/, ⌘B, ⌘D, ⌘F, ⌘W, ⌘[ / ⌘], ⌃Tab, ⇧⌘A, ⇧⌘P and more (`screens/shell/keys.luau`) | ⌘N, ⌘1-9, ⌘+ / ⌘- / ⌘0, ⌘F in settings | Add ⌘K, ⌘/, ⌘B, ⌃Tab, ⇧⌘A and ⇧⌘P. |

## 7. Verification

Zeron verifies its UI with about 80 headless fixtures, about 200 Luau tests
for geometry, motion and parity, a design lint that fails on hard-coded
values, and offscreen captures from the GPU renderer at 2x
(`src/audit_shots.rs`).

Coder Desktop has 60 `#test` blocks in `page/checks/` that check token
geometry and transcript semantics. It has no pixel goldens and no capture
command; comparisons in `docs/coder/` were made by hand.

To make "pixel-perfect" checkable:

1. Add a headless capture for Coder Desktop's page at 2x from fixed fixtures:
   empty chat, a running turn with activity, a settled turn, the composer with
   a queue, the model menu, settings and the subagent pane.
2. Capture the same scenes from Zeron with `AUDIT_SHOTS`, using its fixtures
   (`chat_fixture`, `composer_fixture`, `picker_fixture`, `panel_fixture`).
3. Keep both sets under `docs/coder/zeron-parity/` with a per-scene table of
   measured differences, and pin Coder's frames as goldens.
4. Add the design lint from section 3.

## 8. Order of work

| Step | Work | Done when |
| --- | --- | --- |
| 1 | Layout scale 1.0, all values in points (section 2) | Every font slot and layout constant matches Zeron at zoom 1.0 in `layout-checks.coax`. |
| 2 | One Zeron Dark source from `zeron-dark.json` (section 3), with goldens re-recorded across consumers | No colour literals in `apps/coder-desktop/page/`. The lint passes. |
| 3 | Shadow and blur in the shared shader; real composer, menu and dialog elevation | Composer and model menu match Zeron captures. |
| 4 | Capture pipeline and Zeron reference captures (section 7) | Captures and diff table committed. |
| 5 | Transparent title bar and Zeron traffic-light positions | No native bar above the canvas. |
| 6 | GPU composer editor replacing the NSTextView overlay | Composer text renders through the atlas with IME working. |
| 7 | Composer restructure: compact and expanded modes, footer chips, context ring | Matches Zeron's composer fixture. |
| 8 | Composer deck with Queue (#13717), then Running | Queued follow-ups visible and editable. |
| 9 | Display-link frames, scroll momentum, overscroll, overlay scrollbars | 120 Hz on ProMotion. Scroll matches `coast` and `elastic`. |
| 10 | Sidebar: Zeron row metrics, marks and status, project groups, virtualization | 1,000 chats scroll smoothly; no 50-chat cap. |
| 11 | Subagent pane on activity groups, then panel tabs and Changes | No `app_tool` rows remain. |
| 12 | Command palette, Coax menus and dialogs, toasts | No NSAlert or NSMenu in the chat surface. |
| 13 | Transcript polish: veil, bubble fold, Jump to latest, tool detail, pictures | Matches Zeron's chat fixtures. |

Steps 1 and 2 touch shared skins used by `/chat` and `apps/openagents-desktop`.
Do them as one change per step, and re-record those apps' goldens in the same
change.

## Differences to keep

- **Foreman.** The Foreman row, HUD and tiers have no Zeron equivalent.
- **Cost and goal.** Zeron shows no cost in the transcript. Keep Coder's
  budget and goal, styled as footer chips.
- **Models.** Coder's levels (Neon, Carbon, Hydrogen, Argon) replace Zeron's
  harness list. Keep Zeron's picker layout.
- **Coax.** Match Zeron's pixels in shared Coax components
  (`components/zeron/`, `components/kit/`, `components/skin/`). Don't port
  zgx or Luau.

## Source map

Zeron (`~/work/projects/repos/zeron/apps/zgx/`):

- Theme and tokens: `ui/ds/themes/zeron-dark.json`, `ui/ds/tokens.luau`, `ui/ds/motion.luau`
- Shell: `ui/ds/title_bar.luau`, `ui/ds/sidebar.luau`, `ui/screens/shell/`
- Composer: `ui/ds/composer*.luau`, `docs/composer.md`
- Transcript: `ui/ds/transcript.luau`, `ui/ds/user_message.luau`, `ui/ds/activity.luau`, `ui/ds/tool_detail.luau`, `ui/ds/working.luau`
- Panel: `ui/screens/panel/init.luau`, `ui/ds/git.luau`, `ui/ds/git_changes.luau`
- Palette and menus: `ui/ds/command_palette.luau`, `ui/ds/menu.luau`, `ui/ds/dropdown.luau`, `ui/ds/dialog.luau`
- Renderer: zgx at `b8f298b6` (`crates/zgx-gpu/src/lib.rs`, `zgx-desktop/src/frame_source.rs`, `coast.rs`, `zgx/src/elastic.rs`)
- Verification: `ui/*_fixture.luau`, `ui/tests/`, `tests/design_lint.rs`, `src/audit_shots.rs`

Coder Desktop (`apps/coder-desktop/`):

- Skin and scale: `page/skin.coax`, `page/state.coax`, `components/skin/zeron.coax`, `components/skin/arcade.coax`
- Layout: `page/layout/layout.coax`, `page/layout/sidebar.coax`, `page/layout/conversation.coax`, `page/layout/pane.coax`
- Host: `Window.swift`, `Window+Dialogs.swift`, `Window+Menus.swift`, `apps/shared/host/apple/ui-host.m`, `apps/shared/native/KitEditorMac.swift`
- Follow-ups: `page/followups.coax`, `Window+FollowUps.swift`
- Checks: `page/checks/`
