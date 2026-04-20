# Engineering Graphics UI Style Guide

Autopilot UI is engineering graphics for professionals.

Graphics here means the visual representation of information: operational
state, code, work, evidence, verification, payments, and execution flow. The
purpose is accurate and truthful interpretation. The interface should help an
operator see what is true, what changed, what is uncertain, and what can be
done next.

## Lineage

This guide restores and narrows older OpenAgents design direction found in repo
history and backroom.

OpenAgents commit history hits:

- `1f0bca7e5` introduced `Design philosophy in AGENTS.md`: dense, fast,
  present, file-over-app, moldable, and BYOAI.
- `c267d4316` moved that guidance from `AGENTS.md` into
  `docs/DEVELOPMENT.md`.
- `8606a8518` pruned the old development doc during the MVP cleanup.
- `53270c512` explicitly reduced WGPUI layout spacing for dense display.
- `53558866b` added `docs/inspiration/bt-ui.md`, including a
  USGC-compliant table color-system note.
- `7c0f294ec` added `docs/inspiration/bloomberg-terminal.md`, naming the
  preference set: dense, table-first, explicit labels, no icons, no animations,
  strict USGC, Berkeley Mono, dark `#08090a`, and performance targets.
- `fcb0efdf5` removed the old inspiration docs during the desktop-foundation
  cleanup, which is why backroom now carries the durable copies.
- `621837635` and `85b8a80f0` are older frontend-history hits for USGC/color
  assets.

Backroom durable sources:

- `/Users/christopherdavid/work/backroom/usgc/bloomberg-terminal.md`
- `/Users/christopherdavid/work/backroom/usgc/bt-ui.md`
- `/Users/christopherdavid/work/backroom/reference/openagents-docs/inspiration/bloomberg-terminal.md`
- `/Users/christopherdavid/work/backroom/reference/openagents-docs/inspiration/bt-ui.md`
- `/Users/christopherdavid/work/backroom/platform-basecoat-2025-12-19/ui/colors.rs`
- `/Users/christopherdavid/work/backroom/crates/server/tests/api/css_colors.rs`
- `/Users/christopherdavid/work/backroom/reference/openagents-docs/hud/fixes.md`

Generated JSON/JSONL transcript and feed dumps may contain incidental matches.
Do not treat those as style authority.

## Principles

- Emergent over prescribed aesthetics. Let the data, state, and task shape the
  surface.
- Expose state and inner workings. Prefer registers, event tapes, exact labels,
  IDs, timestamps, counters, and evidence over vague status prose.
- Dense, not sparse. Information density is a feature when the user is a
  professional operator.
- Explicit is better than implicit. Label controls, state, sources, authority,
  and failure modes.
- Engineer for human vision and perception. Use alignment, contrast, grouping,
  color semantics, and gamma-aware choices to make scanning fast and truthful.
- Regiment functionalism. Reuse disciplined grids, tables, panes, strips, and
  state rows.
- Performance is design. Latency, frame stability, load time, and low motion
  are part of the visual spec.
- Verbosity over opacity. Use extra words when they remove ambiguity.
- Ignore trends. The interface should be timeless, unfashionable, and stable.
- Flat, not hierarchical. Avoid big marketing hierarchy inside operator tools.
  Prefer equally inspectable panes.
- Complex as needed. Do not pursue minimalism when the work is inherently
  complex.
- Driven by objective reasoning and common sense. Every visual choice should
  defend what it helps the user see or do.
- Do not infantilize users. Expert users can handle complexity, terminology,
  dense state, and direct controls.
- No consignments. Do not accept decorative components, trend tokens, or visual
  cargo unless they carry state, clarify action, or improve perception.

## Application Rules

- Start UI work from the state model, not from a hero layout.
- Prefer tables, registers, split panes, command strips, status strips, and
  event tapes.
- Use Berkeley Mono as the primary UI font. Use Inter only for longer prose.
- Keep the type scale tight: 10px small, 11px base, 12px section, 14px maximum
  for product/tool titles unless a view has a real inspection need.
- Keep line-height tight: roughly 1.15 for tables and 1.25 for prose.
- Use dark foundations: `#08090a`, `#000000`, `#0a0a0a`, `#101010`.
- Use grid and pane boundaries: `#1a1a1a` and stronger borders when needed.
- Use bright colors only for semantic state:
  - `#ffb400` highlight/focus
  - `#00c853` positive/success
  - `#d32f2f` negative/failure
  - `#ff6f00` warning
  - `#2979ff` link/info
  - `#9e9e9e` muted text
- Avoid gradients, decorative shadows, bokeh, ornamental illustrations, and
  animation that does not communicate state.
- Avoid large empty cards. A card-like panel should carry operational state,
  evidence, or controls.
- Prefer stable dimensions and explicit overflow behavior over layout shifts.
- Show empty state honestly. Do not invent fake activity.
- When a surface is read-only, say so in the chrome.
- When a surface can mutate state, show the authority path and expected effect.
