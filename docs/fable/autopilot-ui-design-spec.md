# Autopilot UI design specification

Status: active design authority

Updated: 2026-07-16

## Scope

This document records the visual-system choices shared by OpenAgents product
surfaces descended from the Autopilot UI. It does not create a separate
Autopilot application, renderer, or token authority. Web, docs, splash, and
Desktop should consume the same owned UI primitives and typography tokens.

## Relationship to Khala

Autopilot is a design donor inside Khala, not a competing product theme.
`khalaTheme` remains the only mounted palette authority across Desktop, web,
docs, splash, boot frames, and preference-scaled rendering. When the two
languages conflict, Khala wins.

The parts to fold in are Autopilot's tactical density, condensed information
hierarchy, restrained mono instrumentation, and compatible color
relationships. They are adapted through Khala semantic roles rather than
copied as raw literals or mounted as `autopilotTheme`:

- Disket Mono carries machine state, commands, paths, shortcuts, and compact
  metadata. Inter carries all product UI and sustained reading. Zalando Sans
  is reserved for explicit, occasional marketing accents and may not enter
  product controls, navigation, conversation content, or ordinary prose.
- Autopilot indigo, cool-neutral, muted-danger, and rare-success ideas may
  inform the corresponding Khala ramps, but the result must stay visibly in
  the Khala blue family and resolve through `--en-color-*` / `--khala-*`
  semantic tokens.
- Main workroom backgrounds and raised surfaces remain the Khala hierarchy:
  `#05070d`, `#0b1220`, and `#141f36`, with Protoss blue `#3b82f6` as the
  primary accent. The gray Autopilot page/panel values must not replace those
  product-surface roles.
- No host may mix both theme objects, add a route-local palette, or use raw
  Autopilot hex values to bypass the shared token authority.

This is a fold-in rule: preserve the useful Autopilot grammar while keeping
Khala's identity and blue depth unmistakable.

### Command execution cards

Command cards use Autopilot's dense scan-readout grammar only through Khala
roles. The header keeps the command primary, with exit code and duration in a
right-aligned mono column. The detail well reports `CWD` and `SOURCE`, streams
a bounded mono output tail, and states when earlier output was omitted. Running,
completed, and failed states use Khala semantic accent/success/danger roles.
they never introduce a gray page surface or raw donor-palette literal.

Live `item/commandExecution/outputDelta` updates and persisted rollout history
must render the same shared component. Provider `itemId` is the reconciliation
key so concurrent commands with the same label cannot cross streams.

### File-change and turn-diff cards

File-change cards borrow Autopilot's compact tally grammar, not its palette.
Each file is a square `[ADD]`, `[DEL]`, or `[MOD]` row with the path left and
`+adds`/`−removes` right aligned in mono type. Add, remove, and diff-metadata
lines resolve through Khala success, danger, and accent roles on Khala's blue
surface hierarchy. No raw Autopilot literal or parallel theme may be mounted.

Each row expands independently to a bounded unified diff and must state when
the bound omitted content. The patch lifecycle remains visible in the card
header. Live file patch updates reconcile by provider item identity. The
latest aggregate turn diff has one stable turn-scoped identity. Persisted
`apply_patch` records use the identical typed component rather than a generic
tool or metadata row.

## Typography

### Primary sans: Inter

Inter is the primary family for interface copy, prose, controls, headings,
navigation, conversation content, docs, and marketing body copy:

```css
font-family: var(--oa-font-sans);
font-optical-sizing: auto;
```

The canonical fallback order is Inter Variable, Inter, the platform UI stack,
and generic `sans-serif`.

### Opt-in sans accent: Zalando Sans

Zalando Sans is not a product-UI family. It may appear occasionally in
deliberately selected marketing copy through the explicit accent token or
utility class:

```css
font-family: var(--oa-font-sans-accent);
```

No global element, application shell, docs surface, control, navigation item,
or conversation component may inherit that accent token. The `.zalando-sans`
and `.oa-font-accent` utilities are opt-in signals, never defaults.

### Primary mono: Disket Mono

Disket Mono is the primary family for code, commands, paths, shortcuts,
machine-state labels, compact metadata, and agent/runtime event detail:

```css
font-family: var(--oa-font-mono);
```

The canonical fallback order is Disket Mono followed by the approved platform
monospace stack. Disket Mono should not replace readable prose merely to make a
surface feel technical.

### Ownership and delivery

- `@openagentsinc/ui/typography.css` is the single family and font-face
  authority.
- Font files are self-hosted. Product rendering must not depend on Google Fonts
  or another third-party font CDN.
- The normal and italic Inter variable faces cover weights 100–900 and optical
  sizing for readable product and marketing rendering.
- The normal and italic Zalando Sans variable faces remain available only for
  explicit marketing accents and cover weights 200–900 and widths 75–125.
- Disket Mono is shipped in regular and bold web-font faces. Do not synthesize
  a replacement family for missing weights.
- `font-display: swap` preserves first paint while the branded faces load.
- Web, docs, splash, and Desktop must use `--oa-font-sans` and
  `--oa-font-mono`. A surface may not redefine competing primary stacks.

### Licensing

Inter and Zalando Sans are included under the SIL Open Font License 1.1.
Disket Mono is included under the Rostype end-user license, which permits
commercial use and generation of web-font formats for websites. License
notices ship with `@openagentsinc/ui` and its font dependencies.

### Verification

The Desktop approved-font oracle scans Desktop and shared UI sources for
unapproved families and CSS font-shorthand escapes. The web typography
contract verifies that the website and integrated docs consume the shared
tokens and do not request Google Fonts. Production acceptance must also verify
that the compiled CSS and each emitted WOFF2 asset return successfully from
`openagents.com`.

## Visual relationship

Typography should support the existing Khala visual system: precise,
high-contrast, information-dense, and calm. Inter carries human-facing
hierarchy and sustained reading. Disket Mono marks machine-facing detail.
Zalando Sans contributes only rare marketing emphasis. The contrast between
those roles is structural. It should not be recreated with arbitrary display
fonts, excessive letter spacing, or all-monospace layouts.
