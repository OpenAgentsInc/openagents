# Continuity and Components

## Continuity across devices

- People aren’t starting over when switching devices or resizing windows — they’re continuing the same task.
- One decision (layout, hierarchy, interaction) should carry across iPhone, iPad, and Mac.
- iPhone: zoomed‑in, vertical focus. iPad: scaling layer that bridges phone and desktop. Mac: wide, expansive canvas.
- Apple frameworks (SwiftUI, Mac Catalyst) and capabilities (e.g., iPad window resizing) support a shared foundation.

## Symbols and labels

- Reuse the same symbols across devices to preserve meaning and build familiarity.
- When no clear shorthand exists (e.g., Select, Edit), prefer a text label over a potentially ambiguous icon.
- Bars and menus lean more on symbols; populate menus with symbols where it aids recognition.
- For grouped actions (e.g., multiple copy actions), introduce the group with one symbol and rely on text for individual items — avoid repeating/tweaking icons.

## Shared anatomy, consistent behavior

- Define a shared component anatomy so pieces persist in familiar placements across platforms.
- Example: Popup (macOS) and context menu (iOS) differ in form but share selection indicator, icon, label, accessory.
- Behavior fills the visual gaps: components should support the same core interactions across devices.
- Tab bars, segmented controls, sidebars consistently signal selection, navigation, and state — same function and feedback, regardless of form.

## Bringing components closer

- Treat platform variations as expressions of a shared framework, not exceptions.
- When related components behave and feel connected, the system “clicks”: structure, language, and interaction reinforce each other.

