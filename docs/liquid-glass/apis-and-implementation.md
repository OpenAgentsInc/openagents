# Liquid Glass APIs and Implementation

> Apple ships Liquid Glass as a first‑party UI material — not a 3rd‑party lib. Available starting iOS 26 / iPadOS 26 / macOS 15 (Sequoia).

This page captures how to use Liquid Glass in SwiftUI and UIKit, with patterns, fallbacks, and performance tips. See also: visual and layout guidance in sibling docs.

## Availability

- Platforms: iOS 26, iPadOS 26, macOS 15 (Sequoia) and later.
- Accessibility: There’s a system toggle (Clear vs Tinted) in iOS 26.1 that increases opacity for legibility. Design for both appearances.

## SwiftUI — essentials

- Apply glass to any shape with `glassEffect(_:in:)`.
- Group multiple glassy elements in a `GlassEffectContainer` (morphs/merges nearby shapes, improves performance).
- For smooth transitions between peers, tag children with `.glassEffectID(_:)`.

```swift
import SwiftUI

struct Card: View {
  var body: some View {
    VStack(spacing: 12) {
      Image(systemName: "bolt.fill").imageScale(.large)
      Text("Liquid Glass").font(.headline)
    }
    .padding(16)
    .background {
      // Regular glass, clipped to a rounded rect shape
      RoundedRectangle(cornerRadius: 20, style: .continuous)
        .fill(.clear)
        .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 20))
    }
  }
}

struct ContentView: View {
  var body: some View {
    GlassEffectContainer {            // groups & morphs glass shapes
      HStack {
        Card().glassEffectID("left")  // ids help with transitions
        Card().glassEffectID("right")
      }
      .padding()
    }
  }
}
```

### Variants

- Material variants include `regular` and `clear` (choose based on contrast and context).

### Fallbacks

```swift
if #available(iOS 26, macOS 15, *) {
  AnyView(
    RoundedRectangle(cornerRadius: 20)
      .fill(.clear)
      .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 20))
  )
} else {
  AnyView(
    RoundedRectangle(cornerRadius: 20)
      .fill(.ultraThinMaterial) // pre‑26 approximation
  )
}
```

## UIKit — essentials

Use `UIGlassEffect` with `UIVisualEffectView` on iOS 26+.

```swift
let effect = UIGlassEffect()              // new in iOS 26
// Optional tint adjustment for emphasis/legibility
effect.tintColor = .systemBackground.withAlphaComponent(0.2)

let glassView = UIVisualEffectView(effect: effect)
glassView.layer.cornerRadius = 20
glassView.layer.masksToBounds = true
```

- Place the `UIVisualEffectView` behind controls or within bars/sheets/popovers.
- Pre‑26 fallback: swap to `UIBlurEffect(style: .systemMaterial)`.

## Desktop and mobile

- SwiftUI API is available on macOS as well; share most view code across platforms.
- Wrap multi‑glass layouts in a single `GlassEffectContainer` on Mac for performance and shape morphing.

## Patterns and tips

- Hit contrast targets
  - Treat glass like background; place text on solid fills or add a subtle overlay if needed to pass WCAG.
- Mask the effect
  - Use custom shapes to limit glass to edges/corners for a “pooling” treatment.
- Prefer one container
  - Use a single `GlassEffectContainer` per scene/screen; let it manage morphing of multiple shapes.
- Don’t stack edge effects
  - Use at most one scroll edge effect per view; pick soft (default, iOS/iPadOS) or hard (often macOS) based on context.
- Mix SwiftUI + UIKit
  - Embed `UIVisualEffectView(UIGlassEffect)` via `UIViewRepresentable` when needed.
- Respect user toggle (iOS 26.1)
  - When users select “Tinted,” opacity increases. Ensure foreground elements remain legible in both modes.

## Cross‑references

- Layout and structure: `structure-and-navigation.md`, `layout.md`
- Visual language: `visual-design.md`
- Adoption steps: `adoption-checklist.md`

## References

- Applying Liquid Glass to custom views (SwiftUI)
- GlassEffectContainer
- Adopting Liquid Glass (technology overview)
- UIKit updates: `UIGlassEffect` in `UIVisualEffectView`
- Example repo: SwiftUI Liquid Glass patterns

```text
[1] https://developer.apple.com/documentation/swiftui/applying-liquid-glass-to-custom-views
[2] https://developer.apple.com/documentation/swiftui/glasseffectcontainer
[3] https://developer.apple.com/documentation/technologyoverviews/adopting-liquid-glass
[4] https://sebvidal.com/blog/whats-new-in-uikit-26/
[5] https://exploreswiftui.com/library/glasseffectcontainer/glass-effect-container
[6] https://github.com/mertozseven/LiquidGlassSwiftUI
```

