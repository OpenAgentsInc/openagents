# Issue #14: Apply Consistent Theming (Liquid Glass, Berkeley Mono, OATheme)

## Phase
Phase 4: Integration & Features

## Priority
Medium - Visual polish and consistency

## Description
Apply consistent theming across all macOS views using Liquid Glass materials, Berkeley Mono font, and OATheme colors for a cohesive, polished interface.

## Current State
- Theme system exists (`OATheme`, `OAFonts`)
- Liquid Glass APIs available (macOS 15+)
- Inconsistent application across components
- Some views use default SwiftUI materials/fonts

## Target State
- All views use `OATheme.Colors` for consistent colors
- Berkeley Mono font used throughout (`OAFonts.mono()`)
- Liquid Glass materials for sidebars, sheets, backgrounds (with fallback)
- Consistent spacing, padding, corner radii
- Dark mode optimized (primary theme)
- High contrast for accessibility
- Smooth animations and transitions

## Acceptance Criteria
- [ ] All text uses Berkeley Mono via `OAFonts.mono()`
- [ ] All colors use `OATheme.Colors` semantic colors
- [ ] Sidebars, chat area, and inspector use Liquid Glass `.regular` material
- [ ] Use single `GlassEffectContainer` wrapping NavigationSplitView
- [ ] All glass shapes tagged with `.glassEffectID(_:)` for transitions
- [ ] Scroll edge effects applied (hard style for macOS)
- [ ] Sheets/modals use Liquid Glass backgrounds
- [ ] Concentric shapes used for nested components (fixed/capsule/concentric)
- [ ] Consistent spacing (8pt grid: 4, 8, 12, 16, 20, 24px)
- [ ] Capsule buttons for primary actions, rounded rects for dense controls
- [ ] Floating buttons use Liquid Glass capsule pattern
- [ ] Hover states implemented with smooth animations
- [ ] Animations smooth (0.2-0.3s ease, 0.3s spring for interactive)
- [ ] Dark mode polished, Light mode supported
- [ ] Accessibility: Clear vs Tinted glass modes respected

## Technical Details

### Theme Application Guidelines

#### Colors
```swift
// Use semantic colors from OATheme
.foregroundColor(OATheme.Colors.textPrimary)    // Main text
.foregroundColor(OATheme.Colors.textSecondary)  // Secondary text
.background(OATheme.Colors.background)          // Backgrounds
.accentColor(OATheme.Colors.accent)             // Accents, CTAs
.foregroundColor(OATheme.Colors.success)        // Success states
.foregroundColor(OATheme.Colors.danger)         // Errors, delete actions

// Never use:
.foregroundColor(.white) // ❌
.background(.black)      // ❌
.foregroundColor(.blue)  // ❌
```

#### Typography
```swift
// Use Berkeley Mono everywhere
.font(OAFonts.mono(size: 14))             // Body text
.font(OAFonts.mono(size: 18, weight: .semibold)) // Headers
.font(OAFonts.mono(size: 12))             // Small text
.font(OAFonts.mono(size: 11))             // Captions

// UI text
.font(OAFonts.ui(size: 14))               // When monospace isn't appropriate

// Never use:
.font(.system(size: 14))  // ❌
.font(.body)              // ❌
```

#### Liquid Glass Materials (macOS 15+)

**Core Principles**
- Liquid Glass creates a "functional layer" that floats above content
- Controls sit on glass material, not directly on content
- Glass provides legibility over varied backgrounds while maintaining immersion

**GlassEffectContainer**
- Wrap the entire `NavigationSplitView` in ONE container per scene
- Container automatically morphs and merges nearby glass shapes
- Improves performance and creates seamless transitions

```swift
// Top-level layout
GlassEffectContainer {
    NavigationSplitView {
        SidebarView().glassEffectID("sidebar")
    } content: {
        ChatAreaView().glassEffectID("chat")
    } detail: {
        InspectorView().glassEffectID("inspector")
    }
}
```

**Applying Glass to Shapes**
```swift
// For backgrounds
.background {
    if #available(macOS 15.0, *) {
        RoundedRectangle(cornerRadius: 12, style: .continuous)
            .fill(.clear)
            .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 12))
    } else {
        RoundedRectangle(cornerRadius: 12)
            .fill(.ultraThinMaterial)
    }
}
```

**Glass Variants**
- `.regular` - Most surfaces (sidebars, chat, inspector, cards)
- `.clear` - Areas needing higher contrast

**Fallback Strategy**
- macOS 13-14: Use `.ultraThinMaterial` or `.ultraThin`
- Always wrap in `#available(macOS 15.0, *)` check
- Don't nest `GlassEffectContainer` (one per scene)

**Scroll Edge Effects**
- Use `.scrollEdgeEffect(.hard)` for macOS (stronger boundary)
- Apply ONE edge effect per scrollable view
- Don't stack or mix soft/hard styles
- Edge effects replace hard dividers - they clarify where UI meets content

```swift
ScrollView {
    // Content
}
.scrollEdgeEffect(.hard)  // macOS style
```

#### Shapes and Concentricity (Liquid Glass Design Language)

**Three Shape Types**
1. **Fixed shapes** - Constant corner radius (e.g., 12pt card)
2. **Capsules** - Radius equals half the container height
3. **Concentric shapes** - Radius derived by subtracting padding from parent

**When to Use Each**
- **Capsules**: Buttons, switches, sliders, grouped table corners - emphasize action
- **Fixed shapes**: Cards, modals, sheets when consistent radius matters
- **Concentric shapes**: Nested components that need to align with parent curvature

**Concentric Layout Pattern**
```swift
// Parent card
RoundedRectangle(cornerRadius: 20, style: .continuous)
    .fill(.clear)
    .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 20))
    .padding(16)

// Child inside (concentric)
RoundedRectangle(cornerRadius: 12, style: .continuous)  // 20 - 8 padding = 12
    .fill(OATheme.Colors.background.opacity(0.5))
```

**Capsule Buttons (Primary Actions)**
```swift
Button("New Chat") { }
    .padding(.horizontal, 16)
    .padding(.vertical, 8)
    .background {
        Capsule()
            .fill(.clear)
            .glassEffect(.regular, in: Capsule())
    }
    .foregroundStyle(.white)
```

**Optical Balance**
- Center mathematically when shapes fit naturally
- Subtly offset when needed for visual balance
- Avoid pinched or flared corners (indicates wrong shape choice)

#### Spacing & Layout (8pt Grid)
```swift
// Use 8pt grid
.padding(4)   // Tiny (icon padding)
.padding(8)   // Small (compact spacing)
.padding(12)  // Medium (standard spacing)
.padding(16)  // Large (card padding, section spacing)
.padding(20)  // XLarge (generous spacing)
.padding(24)  // XXLarge (major sections)

.spacing(8)   // Stack spacing
.spacing(12)
.spacing(16)

// Corner radii (align with shapes)
.cornerRadius(6)   // Small fixed (badges, small buttons)
.cornerRadius(8)   // Medium fixed (inputs, secondary buttons)
.cornerRadius(12)  // Large fixed (cards, modals)
// Capsules: no corner radius needed (use Capsule() shape)
```

#### Buttons (Liquid Glass Guidance)

**Primary Action (Capsule Shape)**
```swift
Button("New Chat") {
    // Action
}
.padding(.horizontal, 16)
.padding(.vertical, 8)
.background {
    if #available(macOS 15.0, *) {
        Capsule()
            .fill(.clear)
            .glassEffect(.regular, in: Capsule())
    } else {
        Capsule()
            .fill(.ultraThinMaterial)
    }
}
.foregroundStyle(OATheme.Colors.accent)  // Tinted
```

**Floating Icon Button (See `docs/liquid-glass/floating-buttons.md`)**
```swift
Button(action: { }) {
    Image(systemName: "arrow.up")
        .renderingMode(.template)
        .symbolRenderingMode(.monochrome)
        .foregroundStyle(.white)  // Explicit, not environment tint
        .font(.system(size: 16, weight: .semibold))
        .frame(width: 36, height: 36)
}
.buttonStyle(.plain)
.background {
    // Glass layer
    if #available(macOS 15.0, *) {
        GlassEffectContainer {
            Capsule().fill(.clear).glassEffect(.regular, in: Capsule())
        }
    } else {
        Capsule().fill(.ultraThinMaterial)
    }
}
.background {
    // Gentle gradient overlay
    Capsule()
        .fill(
            LinearGradient(
                colors: [Color.black.opacity(0.16), Color.black.opacity(0.06)],
                startPoint: .top,
                endPoint: .bottom
            )
        )
}
.overlay {
    Capsule().strokeBorder(OATheme.Colors.textSecondary.opacity(0.3), lineWidth: 0.5)
}
.clipShape(Capsule())
.shadow(color: Color.black.opacity(0.25), radius: 8, x: 0, y: 4)
```

**Secondary/Dense UI (Rounded Rectangle)**
```swift
// For inspectors, toolbars with many controls
Button("Cancel") { }
    .buttonStyle(.bordered)  // System style, rounded rect
```

**Tertiary/Ghost**
```swift
Button("Details") { }
    .buttonStyle(.plain)
    .foregroundColor(OATheme.Colors.accent)
```

#### Hover States
```swift
@State private var isHovered = false

SomeView()
    .background(isHovered ? OATheme.Colors.accent.opacity(0.1) : Color.clear)
    .onHover { hovering in
        withAnimation(.easeInOut(duration: 0.2)) {
            isHovered = hovering
        }
    }
```

### Theme Audit Checklist

Go through each view and ensure:

#### ✅ SessionSidebarView
- [ ] Background uses `.sidebarMaterial()`
- [ ] Text uses `OAFonts.mono()`
- [ ] Colors use `OATheme.Colors`
- [ ] Session rows have hover states
- [ ] Spacing follows 8pt grid

#### ✅ ChatAreaView
- [ ] Background uses `OATheme.Colors.background`
- [ ] Messages use Berkeley Mono
- [ ] Tool calls use theme colors for status
- [ ] Consistent padding (16px)

#### ✅ InspectorPaneView
- [ ] Background uses `.backgroundMaterial()`
- [ ] Section headers use theme colors
- [ ] JSON viewer has proper contrast
- [ ] Collapsible sections animate smoothly

#### ✅ ComposerMac
- [ ] Input uses Berkeley Mono
- [ ] Background uses translucent material
- [ ] Send button uses accent color
- [ ] Placeholder uses `textSecondary`

#### ✅ SettingsView
- [ ] All form controls use theme
- [ ] Consistent section headers
- [ ] Proper spacing between sections
- [ ] Buttons use standard styles

#### ✅ DeveloperView
- [ ] Code blocks use Berkeley Mono
- [ ] Logs use appropriate colors
- [ ] Database query editor themed
- [ ] Export buttons consistent

### Animation Standards (Liquid Glass Guidelines)

**Timing and Curves**
```swift
// Quick transitions (color, opacity changes)
.animation(.easeInOut(duration: 0.2), value: someState)

// Standard transitions (layout, size changes)
.animation(.easeInOut(duration: 0.25), value: someState)

// Spring animations for interactive elements (hover, selection)
.animation(.spring(response: 0.3, dampingFraction: 0.7), value: someState)

// Explicit withAnimation for state changes
withAnimation(.easeInOut(duration: 0.2)) {
    isHovered = true
}
```

**Glass Shape Transitions**
- Glass shapes morph smoothly when using `.glassEffectID(_:)`
- Let `GlassEffectContainer` handle morphing automatically
- Don't manually animate glass shapes (container does this)

**Hover States**
```swift
@State private var isHovered = false

SomeView()
    .scaleEffect(isHovered ? 1.02 : 1.0)
    .background(isHovered ? OATheme.Colors.accent.opacity(0.1) : Color.clear)
    .animation(.spring(response: 0.3, dampingFraction: 0.7), value: isHovered)
    .onHover { hovering in
        isHovered = hovering
    }
```

**Focus Changes**
- Liquid Glass subtly adjusts opacity/size to signal depth during focus changes
- Let system handle automatic adjustments
- Don't override with custom animations unless needed

### Dark Mode Optimization
```swift
// Ensure colors work in dark mode
OATheme.Colors.background        // Should be dark in dark mode
OATheme.Colors.textPrimary       // Should be light in dark mode

// Test both modes:
// System Settings > Appearance > Dark/Light
```

### Component Templates

#### Card Template
```swift
struct ThemedCard<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        content
            .padding(16)
            .background(OATheme.Colors.background.opacity(0.8))
            .cornerRadius(12)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(OATheme.Colors.textSecondary.opacity(0.1), lineWidth: 1)
            )
    }
}
```

#### Section Header Template
```swift
struct ThemedSectionHeader: View {
    let title: String

    var body: some View {
        Text(title)
            .font(OAFonts.mono(size: 11, weight: .semibold))
            .foregroundColor(OATheme.Colors.textSecondary)
            .textCase(.uppercase)
            .padding(.vertical, 8)
    }
}
```

### Testing Theme Consistency
1. Run app in Dark Mode
2. Run app in Light Mode (if supported)
3. Check all views use theme colors
4. Verify no hardcoded colors (#FFFFFF, .white, etc.)
5. Test hover states
6. Test focus indicators
7. Screenshot comparison with iOS app for consistency

## Dependencies
- All UI components (Issues #1-#12)

## Blocked By
None - Can be applied incrementally

## Blocks
None - Polish/refinement

## Estimated Complexity
Medium (4-6 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] All views use consistent theming
- [ ] No hardcoded colors found (code audit)
- [ ] Berkeley Mono renders correctly everywhere
- [ ] Liquid Glass materials applied (macOS 15+)
- [ ] Fallback materials work (macOS 13-14)
- [ ] Dark mode looks polished
- [ ] Animations smooth (no jank)
- [ ] Hover states work consistently
- [ ] Visual comparison with iOS app shows consistency
- [ ] Accessibility contrast ratios pass (WCAG AA)

### Liquid Glass Adoption Checklist

Based on `docs/liquid-glass/adoption-checklist.md`:

**Visual Language**
- [ ] Use fixed, capsule, and concentric shapes appropriately
- [ ] No pinched or flared corners (indicates wrong shape)
- [ ] Capsule buttons for primary actions on macOS
- [ ] Rounded rectangles for dense UI (inspectors, small controls)

**Materials and Layering**
- [ ] Single `GlassEffectContainer` wrapping main layout
- [ ] All glass shapes tagged with `.glassEffectID(_:)`
- [ ] Controls on glass material, not directly on content
- [ ] Scroll edge effects (.hard for macOS) applied correctly

**Colors and Contrast**
- [ ] Test over varied backgrounds (light, dark, images)
- [ ] Verify WCAG contrast ratios for text on glass
- [ ] Respect user toggle (Clear vs Tinted) in iOS 26.1+

**Hierarchy and Structure**
- [ ] Glass treated as functional layer above content
- [ ] Remove decorative borders/backgrounds (rely on grouping/spacing)
- [ ] Primary actions stand apart and are tinted

**Cross-Device Consistency**
- [ ] Shared component anatomy across platforms
- [ ] Consistent behaviors (selection, navigation, state)
- [ ] Use same symbols across iOS/macOS where possible

## References
- OATheme: `ios/OpenAgents/Theme/Theme.swift`
- OAFonts: `ios/OpenAgents/Theme/Fonts.swift`
- **Liquid Glass Documentation:**
  - `docs/liquid-glass/README.md` - Overview
  - `docs/liquid-glass/apis-and-implementation.md` - SwiftUI APIs
  - `docs/liquid-glass/visual-design.md` - Colors, shapes, concentricity
  - `docs/liquid-glass/structure-and-navigation.md` - Functional layer, bars, edge effects
  - `docs/liquid-glass/layout.md` - Layout guidelines
  - `docs/liquid-glass/continuity-and-components.md` - Cross-device patterns
  - `docs/liquid-glass/floating-buttons.md` - Floating button pattern
  - `docs/liquid-glass/adoption-checklist.md` - Implementation checklist
- ADR-0005: Liquid Glass adoption
- Berkeley Mono: https://berkeleygraphics.com/typefaces/berkeley-mono/
- Apple HIG - Liquid Glass: https://developer.apple.com/documentation/swiftui/glasseffectcontainer
