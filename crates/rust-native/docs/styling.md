# Style semantics

The implemented style system is typed data and deterministic composition.
It does not implement CSS, a layout engine, a theme service, or a renderer.
Applications supply their palettes and defaults.

`Color` contains sRGB `red`, `green`, `blue`, and `alpha` channels as `u8` values.
`Color::rgb` sets alpha to 255. The framework supplies no product colors.
`Space::{None, Xs, Sm, Md, Lg}` identifies semantic spacing roles; an adapter
must document their mapping to cells, logical units, or CSS pixels. Text weight
and alignment use `TextWeight` and `TextAlign`. Logical `Start` and `End` do not
by themselves provide a bidirectional layout engine.

## Ordered composition

Each `StylePatch` property has three states:

| Later patch | Effect on the earlier property |
| --- | --- |
| `Unset` | Keep the earlier declaration. |
| `Set(value)` | Replace it with the supplied value. |
| `Reset` | Remove the earlier choice and use the explicit default when resolving. |

`StylePatch::then` composes canonical leaf properties in caller order.
Composition is associative, with an all-`Unset` identity, but is not commutative.
A padding convenience constructor expands into four leaf patches before
composition, so the same rule applies to shorthands and individual edges.
Do not sort layers by name, map order, or source location.

`StyleSheet::new` validates declaration names and refuses duplicates.
`compose` refuses unknown names. Names are identifiers, not selectors, and
there is no specificity or hidden priority mechanism. Callers own the registry
size; view bounds apply to the resolved values serialized into a view.

```rust
use rust_native::style::{Color, Patch, Space, Style, StylePatch};

let base = StylePatch::padding(Space::Md).then(StylePatch {
    foreground: Patch::Set(Color::rgb(240, 240, 240)),
    ..StylePatch::default()
});
let override_style = StylePatch {
    padding_start: Patch::Set(Space::Sm),
    foreground: Patch::Reset,
    ..StylePatch::default()
};
let resolved = base.then(override_style).resolve(Style {
    foreground: Some(Color::rgb(90, 90, 90)),
    ..Style::default()
});
assert_eq!(resolved.foreground, Some(Color::rgb(90, 90, 90)));
assert_eq!(resolved.padding_start, Some(Space::Sm));
assert_eq!(resolved.padding_end, Some(Space::Md));
```

## Explicit resolution and support

After composition, `resolve(defaults)` substitutes the supplied default for
remaining `Unset` and `Reset` values. A resulting `None` delegates to an
adapter's documented default, never incidental state left in a recycled
widget. The initial contract has no implicit inheritance from parent text or
layout nodes.

Each adapter must state how it maps spacing, color and alpha, weight,
alignment, and omitted values. It must expose an unsupported property or use a
documented faithful fallback rather than silently changing meaning. Color-only
information is insufficient for accessibility; labels and semantic state
remain necessary.

Dynamic theme groups, environment predicates, units with checked geometry,
macros, CSS extraction, and platform mapping are future extensions. Add them
only with explicit semantics and fixtures. Styling never grants authority or
executes an application's intent.
