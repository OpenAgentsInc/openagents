# Coder styling design with Rust Native

Rust Native should make a component's appearance a typed, deterministic value.
Define styles near the component, compose them in an explicit order, resolve
semantic tokens for the current surface, and pass the result to a renderer.
Application state determines which styles apply. Styling does not determine
which actions the application permits.

This document proposes the styling contract and its implementation order. It
draws on a read of the local StyleX checkout at commit
`fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2`. It does not copy StyleX code or require
its JavaScript compiler. The initial API example uses the types in
[`src/style.rs`](../../../crates/rust-native/src/style.rs). Other API sketches are explicitly marked as
planned; they do not claim that those interfaces or renderers exist. The
crate's source and README define the delivered subset.

The initial core implements `Style`, `StylePatch`, `Patch<T>`, and a validated
`StyleSheet` name registry. It has no implicit text inheritance, dynamic theme
groups, predicate engine, geometry conversion, CSS extraction, or platform
renderer inside the generic crate. The separate [iOS reader adapter](../../../bins/coder-ios/host/App/NativeView.swift)
now maps RGBA colors, spacing, weight, and alignment to SwiftUI. Its
[verification record](../verification/2026-09-26-mobile-reader.md) covers that
application subset, not every platform or proposed theme feature.

## What to take from StyleX

StyleX separates authored declarations, composition, and the browser's final
rendering. That separation is useful beyond CSS:

| StyleX mechanism | Rust Native recommendation |
| --- | --- |
| Statically analyzable named style definitions | Ordinary immutable Rust values, with `const` constructors where practical. Keep styles near their component. |
| Last applied style wins for the same property | Compose typed property patches left to right, independently of module, definition, or map iteration order. |
| Compiler rules for shorthand conflicts | Normalize convenience constructors to leaf properties before composition. Use one conflict policy. |
| `null` removes an earlier local declaration | Represent an explicit reset separately from an absent patch. Document the resolution default. |
| `defineVars` and `createTheme` | Use typed semantic tokens and explicit theme scope. Keep token identity separate from resolved platform values. |
| Conditional styles selected by ordinary application code | Start with Rust `match` and `if`. Add declarative predicates only when their semantics are stable. |
| Media and support conditions | Resolve a host-supplied environment snapshot and a renderer capability profile. |
| `firstThatWorks` | Select a declared supported fallback, and expose the selection to diagnostics. |
| Build-time CSS extraction and atomic declaration reuse | Consider a web optimization after the shared semantics work. Native and terminal renderers do not need CSS classes. |

StyleX is not uniformly “last wins” across all CSS spellings. Its default
`property-specificity` mode lets a longhand beat a shorthand even when the
shorthand is applied later. Its `application-order` mode chooses a different
policy. Rust Native should avoid exposing this choice: a padding convenience
constructor expands to four leaf patches, and those patches follow the same
ordered merge rule as every other property. See StyleX's
[composition guide](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/docs/content/docs/learn/styling-ui/using-styles.mdx)
and [resolution configuration](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/docs/content/docs/api/configuration/babel-plugin.mdx).

The StyleX runtime also makes its compiler dependency explicit: authoring
functions such as `create` and `defineVars` throw if they reach runtime, while
`props` composes compiled values. Rust can start with checked data structures
without inventing a corresponding source transformation. See the
[runtime entry point](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/@stylexjs/stylex/src/stylex.js).

## One predictable composition rule

Use three states for a property patch:

```rust,ignore
// The three cases of the implemented Patch<T> type.
enum Patch<T> {
    Unset,   // This layer makes no change.
    Set(T),  // This layer supplies the value.
    Reset,   // This layer removes earlier local choices for this property.
}
```

For each leaf property, a later `Unset` preserves the earlier patch. A later
`Set` or `Reset` replaces it. Preserve `Reset` through composition; converting
it to `Unset` would accidentally resurrect an earlier value when another layer
is added. In particular:

| Earlier patch | Later patch | Composed patch |
| --- | --- | --- |
| `Set(a)` | `Unset` | `Set(a)` |
| `Set(a)` | `Set(b)` | `Set(b)` |
| `Set(a)` | `Reset` | `Reset` |
| `Reset` | `Unset` | `Reset` |
| `Reset` | `Set(b)` | `Set(b)` |

Composition is associative, with an all-`Unset` identity. It is not commutative.
The same ordered sequence must produce the same result whether its layers were
defined in one module or several. Never sort layers by name, address, hash, or
source location.

Resolve a reset against an explicit property default, not a renderer's
incidental previous state. `StylePatch::resolve(defaults)` takes a `Style`
explicitly: both remaining `Unset` and `Reset` slots use those defaults after
composition. A `None` in the resulting `Style` delegates to the adapter's
documented default. The adapter must supply a complete interpretation before
drawing; `Style::default()` alone does not promise a complete rendered style.
Callers must pass any inherited text context explicitly. Do not silently
import browser inheritance into terminal or native views. If a later
version adds `Inherit`, give it its own operation and specify which properties
support it. Padding and gap do not inherit from a parent.

The core has generic foreground and background RGBA colors, four padding leaf
values, gap, text weight, and text alignment. Coder resolves its application
intensities into those colors before constructing a view. Avoid a second generic map of string property
names that bypasses these types. Keep layout direction on the stack or layout
node until a shared layout contract needs it in styles.

```rust
use rust_native::style::{Color, Patch, Space, Style, StyleError, StylePatch, StyleSheet};

fn panel_style() -> Result<Style, StyleError> {
    let panel = StylePatch::padding(Space::Md).then(StylePatch {
        foreground: Patch::Set(Color::rgb(200, 200, 200)),
        ..StylePatch::default()
    });
    let emphasis = StylePatch {
        foreground: Patch::Set(Color::rgb(255, 255, 255)),
        ..StylePatch::default()
    };
    let caller = StylePatch {
        gap: Patch::Set(Space::Sm),
        ..StylePatch::default()
    };
    let sheet = StyleSheet::new([
        ("panel".into(), panel),
        ("caller".into(), caller),
        ("emphasis".into(), emphasis),
    ])?;
    Ok(sheet.compose(["panel", "caller", "emphasis"])?
        .resolve(Style::default()))
}
// Foreground is white; all padding is Md; gap is Sm.
// Other properties remain None for the adapter to resolve.
```

The component chooses the override contract by choosing this order. A caller
style applied after defaults may override defaults. A required focus indicator
can be applied after caller styling. Document component-specific restrictions;
do not invent hidden “important” layers.

`StyleSheet::new` rejects duplicate or invalid names; `compose` rejects an
unknown name instead of skipping it. Names identify reusable declarations,
not selectors. The registry's key order never determines the layer order.
Ordinary Rust modules and named values provide the initial static definition
mechanism. Runtime-created patches use the same typed contract.

Convenience methods must have ordinary sequential behavior. Setting all padding
after setting one side replaces that side; setting one side afterward overrides
only that side. The initial padding fields are `padding_top`, `padding_end`,
`padding_bottom`, and `padding_start`. `TextAlign` also uses `Start` and `End`.
This permits direction-aware mapping but does not itself implement a
bidirectional layout engine. Do not silently treat left as start or add
physical aliases without specifying their normalization order.

## Tokens before platform coordinates

Keep Coder's four-level amber ladder in `coder-ui`, the application layer.
`coder-terminal` re-exports that same `Intensity` and background constants.
Rust Native has no palette, intensity, selected-surface color, or product
reference; it receives `Color { red, green, blue, alpha }` values supplied by
its caller. The terminal's
[ladder adapter](../../../crates/coder-terminal/src/ladder.rs) distinguishes
exact RGB, indexed color, and colorless output.

The shared framework vocabulary is:

- Generic sRGB `Color` channels for foreground and background.
- `Space::{None, Xs, Sm, Md, Lg}` for semantic spacing roles.
- `TextWeight::{Normal, Bold}` and `TextAlign::{Start, Center, End}` for text.

Do not encode “small” as an untyped number that every renderer interprets
differently. A terminal cell, a native logical unit, and a CSS pixel are distinct
measurements. The profile maps a semantic token into each renderer's units.
Horizontal and vertical terminal spacing may need different mappings because
cells are not square.

Before adding arbitrary geometry, introduce domain-specific units with checked
constructors. This is a planned extension:

```rust,ignore
struct Cells(u16);
struct LogicalUnits(FiniteNonNegative);
struct CssPixels(FiniteNonNegative);
struct Fraction(UnitInterval);

// Adapter-owned mappings; these are not From conversions between units.
fn terminal_space(token: Space, axis: Axis) -> Cells;
fn native_space(token: Space, scale: TextScale) -> LogicalUnits;
fn web_space(token: Space, scale: TextScale) -> CssPixels;
```

Use private fields and checked deserialization where validity depends on numeric
bounds. A constructor does not protect an unchecked JSON decoder. Reject NaN,
infinity, negative padding, and unsupported values instead of truncating them.
If layout rounds a valid fractional size to cells, make the rounding rule part
of the terminal adapter contract and test clipping and narrow viewports.

### Theme scope

A style should retain token references until resolution. Resolving a token to a
literal at declaration time prevents a later theme or accessibility change
from taking effect.

The Coder palette is an application theme, not a framework default. An adapter supplies its complete
default interpretation. Later, separate complete theme replacement from
partial overrides:

```rust,ignore
// Planned extension, not the initial StyleSheet API.
ThemeScope::replace(complete_theme);
ThemeScope::overlay(typed_theme_patch);
```

Replacement supplies the complete group and does not merge omitted fields from
an unrelated theme. An overlay explicitly inherits the parent group and changes
specified members. Token names and types remain fixed; a color token cannot
receive spacing. Avoid arbitrary string token names in the public core.

This distinction is deliberate. StyleX themes for the same variable group are
mutually exclusive; unoverridden variables return to group defaults. It does
not combine every partial theme passed to `props`. See its
[theme composition recipe](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/docs/content/docs/learn/recipes/merge-themes.mdx).

If derived tokens are added, restrict them to a typed, acyclic expression graph.
Check missing references and cycles when constructing or loading the theme.
Constants can be evaluated at build time; environment-dependent values must
resolve when that environment changes. A theme does not execute arbitrary code,
read a file, or fetch a URL.

## State and capability adaptation

Start by selecting style layers from application state in Rust. For example,
the application can choose a selected style from the current selection and a
disabled style from whether an action is available. A disabled appearance alone
must never disable or authorize an action. Event dispatch checks the current
view and current application state independently.

An optional future rule layer should accept closed predicates, not CSS selector
strings or arbitrary callbacks inside serialized views:

```rust,ignore
// Planned rule API. Direct Rust conditionals are sufficient for the first core.
Rule::when(State::FocusVisible, focus_style);
Rule::when(State::Selected, selected_style);
Rule::when(Capability::Hover, hover_affordance);
Rule::when(Preference::ReducedMotion, static_style);
```

Resolve all predicates against one immutable environment snapshot for the view:
interaction state, viewport, text scale, writing direction, accessibility
preferences, and renderer capabilities. Apply matching rules in their declared
order using the same patch composition. Conjunction must be explicit; do not
recreate CSS specificity by counting conditions. Cache only with the theme,
style, relevant state, environment, and renderer profile in the cache key.

The renderer reports capabilities; a remote view document cannot claim that a
terminal supports hover or that a device permits animation. Unknown support
does not count as supported. An application-provided preference may request an
appearance, but the local renderer retains the accessibility and capability
constraints needed to display it safely.

Support fallbacks need three outcomes: exact mapping, declared approximation,
or unsupported. The shared theme can map intensity to monochrome attributes,
but the renderer must not claim that four levels remain visually distinct if
two levels collapse. Focus, selection, errors, and disabled states need textual
or structural cues as well as brightness. Reduced motion needs a static
alternative that preserves progress or status information.

For optional decoration, the renderer may use a declared fallback and retain a
diagnostic. For essential layout or interaction, refuse an unsupported profile
or select an explicitly designed alternate component. Silently dropping a
property is not portable behavior.

## Renderer responsibilities

The shared contract describes intent. Each adapter owns concrete measurement,
layout, drawing, and platform constraints. The iOS reader implements a bounded
SwiftUI mapping for its current controls; the table describes the broader
cross-platform target. It does not claim delivered terminal/web adapters,
complete accessibility support, or every native styling behavior.

| Concern | Terminal | Native | Web |
| --- | --- | --- | --- |
| Foreground and surface | Reuse `Ladder` for truecolor, indexed, and `NO_COLOR`; preserve glyph cues. | Resolve shared tokens into toolkit colors; respect the declared accessibility profile. | Resolve tokens into escaped CSS values or generated classes under a controlled root. |
| Spacing | Resolve by axis to whole cells; clip within the allocation. | Resolve to platform logical units, then let the backend map to physical pixels. | Resolve to CSS lengths; browser zoom remains browser behavior. |
| Text | Use terminal cell width and wrapping; do not assume bytes or scalar values equal columns. | Use the backend's font metrics and text scaling. | Preserve semantic text elements and browser text scaling. |
| Alignment and direction | Start with stated direction support; avoid fake bidirectional parity. | Resolve logical start/end with the layout direction. | Emit logical properties where supported by the declared profile. |
| State | Keyboard focus and explicit selection are primary; hover requires an actual input capability. | Map focus, pressed, selected, and disabled state from the native input layer. | Use semantic element state and focus-visible behavior; no unrelated selector overrides. |
| Effects | Do not imply opacity, blur, transform, or arbitrary animation support. | Admit effects only when the backend implements them. | Keep CSS-only extensions outside the portable property set. |

A future CSS emitter must preserve shared composition even when browser CSS
would cascade differently. It can emit fully resolved values, or use controlled
classes with deterministic conflict handling. It must not hand conflicting
atomic class names to the browser and assume HTML class order determines the
winner. External stylesheets, inheritance, and browser defaults also need an
explicit embedding contract before parity can be claimed.

Platform-specific escape hatches should be typed, namespaced extensions that
declare their required adapter. Do not add a universal `raw_style: String` that
creates an unvalidated parallel system. A native-only extension must report its
unsupported status when the same view reaches a terminal renderer.

## Validation belongs at several boundaries

| Boundary | Checks |
| --- | --- |
| Rust type checking | Property value domains, token families, closed variants, and explicit unit types. |
| Constant construction | Canonical leaf expansion and compile-time-valid constants where Rust permits them. No custom compiler is required for this stage. |
| Deserialization and admission | Schema version, recognized fields, numeric bounds, view depth, layer count, theme references, and supported extension identities. |
| Composition | Ordered patch semantics, reset preservation, and deterministic output. No I/O or application actions. |
| Theme resolution | Missing or cyclic token references, complete replacement groups, and known environment inputs. |
| Renderer admission | Supported properties and capabilities, documented fallbacks, and available size and text metrics. |
| Event dispatch | Current node identity, enabled state, and the typed intent carried by the current view. Style is never authorization. |

Static declarations reduce repeated work but do not make every value static.
Selection, viewport changes, user preferences, and dynamic content are runtime
inputs. A runtime-provided view must receive the same semantic validation as a
locally authored one. Do not treat a serialized style as trusted because the
Rust producer normally creates valid values.

For inspectability, a development resolver can report the winning layer for
each property, the token resolution, and any fallback. Keep that explanation
separate from rendering and application control. It need not ship full source
paths or other machine-local metadata to a remote client.

## Implementation order

1. **Establish the small pure core.** Keep the intensity palette in Coder, implement
   typed styles and ordered patches with explicit default resolution, and
   serialize the supported view values. This is the initial implementation
   scope. Keep predicates, theme scopes, custom units, macros, and CSS
   generation out of this first contract.
2. **Connect the existing terminal adapter.** Consume the shared values through
   the existing color ladder, introduce named spacing mappings, and preserve
   current keyboard and colorless behavior. Add focused fixtures before
   migrating consumers. Sharing a palette alone is not renderer migration.
3. **Implement native and web adapters.** Add the basic token mappings,
   capability diagnostics, and accessibility alternatives each adapter needs.
   Use the same view and style fixtures to compare intended properties and
   actions. Record unsupported cases. Code-level conformance does not establish
   visual or assistive-technology acceptance; those checks remain separate.
4. **Extend tokens and environment resolution when needed.** Theme scopes,
   checked geometry, and explicit state predicates are optional RN6 work in the
   [build order](build-order.md), not prerequisites for the first adapters or
   application migrations. Define serialized schema changes before exposing
   new values to remote views.
5. **Optimize measured bottlenecks.** Consider interned styles, cache keys, a
   `const` or macro authoring layer, and static web CSS extraction only after
   the semantics and real consumers justify them. Preserve the pure Rust data
   interface as the compatibility boundary.

Each stage needs a small semantic fixture set: left-to-right overrides,
`Unset` identity, associative composition, resets across nested composition,
padding expansion order, distinct token/unit domains, theme scope isolation,
and explicit unsupported mappings. Later adapters need matching fixtures for
colorless focus, large text, narrow layouts, Unicode text, direction, state
changes, and serialized version refusal. These are test requirements, not
reported platform results.

## Reviewed sources

The source links below pin the reviewed StyleX revision so later changes cannot
silently change the basis for this design:

- [StyleX design principles](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/docs/content/docs/learn/thinking-in-stylex.mdx)
  explain co-location, deterministic resolution, and compiler/runtime separation.
- [Style definitions](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/docs/content/docs/learn/styling-ui/defining-styles.mdx)
  describe static restrictions, conditional properties, and bounded authoring
  forms for dynamic styles.
- [Variable definitions](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/docs/content/docs/learn/theming/defining-variables.mdx)
  and [variable types](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/docs/content/docs/learn/theming/variable-types.mdx)
  separate token identity, theme values, and CSS type declarations.
- [Context-driven styles](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/docs/content/docs/learn/recipes/context-driven-styles.mdx)
  show explicit context as an alternative to remote descendant styling.
- [Ordered fallbacks](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/docs/content/docs/api/javascript/firstThatWorks.mdx)
  describe browser support selection; they do not establish portability to
  native or terminal backends.
- [Application-order preprocessing](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/@stylexjs/babel-plugin/src/shared/preprocess-rules/application-order.js)
  and [property-specificity preprocessing](https://github.com/facebook/stylex/blob/fe0be7f0e76ccc585385f0fa56d9b50053ff9ca2/packages/@stylexjs/babel-plugin/src/shared/preprocess-rules/property-specificity.js)
  demonstrate the compiler work required to reconcile CSS shorthand conflicts.

This work is a Rust design derived from those public concepts. It makes no
performance claim, native-rendering claim, or platform-acceptance claim from
reading the reference implementation.
