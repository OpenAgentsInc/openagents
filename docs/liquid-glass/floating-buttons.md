# Floating Buttons on Liquid Glass (iOS)

This notes how we made small, bright buttons sit over Liquid Glass without getting dimmed or expanding unexpectedly.

Key points

- Layer separation: Render the glyph above the glass. Build the foreground (icon + padding) first, then attach the glass background via `.background(...)`. This ensures the background hugs the control’s intrinsic size and never fills the screen.
- Template symbol + explicit color: Use `Image(...).renderingMode(.template).symbolRenderingMode(.monochrome).foregroundStyle(.white)` to avoid environment tints that can darken the symbol.
- Avoid `.tint` on the container: It can re-tint symbols via the environment. Keep the symbol’s color explicit instead of relying on `.tint`.
- Gentle tint over glass: Use a light gradient overlay on top of the glass material to fit the dark theme without crushing contrast (e.g., black 0.16 → 0.06).
- Border and shadow: A subtle stroke (`.strokeBorder`) and shadow lift the control and improve separation over varied backdrops.

SwiftUI pattern (simplified)

```swift
let fg = HStack { icon }
  .padding(.horizontal, 8).padding(.vertical, 8)

fg.background(
    GlassEffectContainer { Capsule().fill(Color.clear).glassEffect(.regular, in: Capsule()) }
)
.background(
    Capsule().fill(LinearGradient(colors: [Color.black.opacity(0.16), Color.black.opacity(0.06)], startPoint: .top, endPoint: .bottom))
)
.overlay(Capsule().strokeBorder(OATheme.Colors.border.opacity(0.6), lineWidth: 1))
.clipShape(Capsule())
.shadow(color: Color.black.opacity(0.35), radius: 12, x: 0, y: 8)

Image(systemName: "pencil")
  .renderingMode(.template)
  .symbolRenderingMode(.monochrome)
  .foregroundStyle(.white)
```

Why this works

- The glass background is anchored to the foreground’s size. There’s no full‑screen ZStack background that could accidentally expand the material.
- The template symbol keeps a stable single tone, independent of glass tint. A tiny shadow (or hierarchical mode) adds legibility over variable backdrops.

See `ios/OpenAgents/FloatingToolbar.swift` and `FloatingMenuButton.swift` for full implementations.

Multi-action capsule

- You can place multiple icon buttons inside the same capsule by increasing the HStack spacing slightly (e.g., 6) and keeping each icon framed to a square hit area (e.g., 36×36). The foreground stays on top of the glass background.
- Example adds a second `mic` button next to the compose button while keeping the same glass background sizing.

Related: Tab bar bottom accessory

- For a demo `TabView` with a bottom accessory, see `ios/OpenAgents/Examples/ChatTabsDemo.swift`.
- It uses `tabViewBottomAccessory { ... }` that adapts to `@Environment(\.tabViewBottomAccessoryPlacement)` — rendering an expanded inline composer above the tab bar and a compact control set when collapsed into the bar.
