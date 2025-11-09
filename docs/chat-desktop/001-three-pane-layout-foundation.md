# Issue #1: Create Three-Pane NavigationSplitView Layout Foundation

## Phase
Phase 1: Core Infrastructure

## Priority
Critical - Foundation for entire transformation

## Description
Replace the current `SimplifiedMacOSView` dashboard with a three-pane `NavigationSplitView` layout that serves as the foundation for the ChatGPT-style interface.

## Current State
- `SimplifiedMacOSView.swift` displays a two-column masonry-style card dashboard
- Uses `NavigationStack` with card-based navigation
- No chat interface or session management

## Target State
- Three-pane layout using `NavigationSplitView`:
  - **Left sidebar** (220-280px): Session history and navigation
  - **Center pane** (flex): Main chat interface area
  - **Right pane** (280-320px, collapsible): Inspector/details pane
- Responsive column widths with user-adjustable dividers
- Proper state management for pane visibility
- macOS-native appearance with Liquid Glass materials (when available)

## Acceptance Criteria
- [ ] Create new `ChatMacOSView.swift` file with `NavigationSplitView` structure
- [ ] Implement three columns with appropriate default widths
- [ ] Add visibility toggles for left and right panes
- [ ] Support keyboard shortcuts for toggling panes (⌘B for sidebar, ⌘I for inspector)
- [ ] Apply Liquid Glass materials with `GlassEffectContainer` for the overall layout
- [ ] Apply proper styling with `OATheme` colors
- [ ] Use scroll edge effects (hard style for macOS) where appropriate
- [ ] Replace `SimplifiedMacOSView` as the main macOS entry point
- [ ] Ensure proper SwiftUI preview for development

## Technical Details

### New File Structure
```swift
// ios/OpenAgents/Views/macOS/ChatMacOSView.swift
struct ChatMacOSView: View {
    @State private var columnVisibility = NavigationSplitViewVisibility.all
    @State private var showInspector = true

    var body: some View {
        // Wrap the entire layout in GlassEffectContainer for performance and morphing
        GlassEffectContainer {
            NavigationSplitView(columnVisibility: $columnVisibility) {
                // Sidebar: Session history (placeholder for Issue #4)
                SidebarPlaceholderView()
                    .navigationSplitViewColumnWidth(min: 220, ideal: 250, max: 280)
                    .glassEffectID("sidebar")  // For smooth transitions
            } content: {
                // Center: Chat area (placeholder for Issue #5)
                ChatAreaPlaceholderView()
                    .navigationSplitViewColumnWidth(min: 400, ideal: 600)
                    .glassEffectID("chat-area")
            } detail: {
                if showInspector {
                    // Inspector: Details pane (placeholder for Issue #6)
                    InspectorPlaceholderView()
                        .navigationSplitViewColumnWidth(min: 280, ideal: 300, max: 350)
                        .glassEffectID("inspector")
                }
            }
            .navigationSplitViewStyle(.balanced)
        }
    }
}
```

### Files to Create
- `ios/OpenAgents/Views/macOS/ChatMacOSView.swift` - Main three-pane layout
- `ios/OpenAgents/Views/macOS/SidebarPlaceholderView.swift` - Temporary sidebar content
- `ios/OpenAgents/Views/macOS/ChatAreaPlaceholderView.swift` - Temporary chat content
- `ios/OpenAgents/Views/macOS/InspectorPlaceholderView.swift` - Temporary inspector content

### Files to Modify
- `ios/OpenAgents/OpenAgentsApp.swift` - Switch macOS view from `SimplifiedMacOSView` to `ChatMacOSView`

### Keyboard Shortcuts
- ⌘B: Toggle left sidebar
- ⌘I: Toggle right inspector pane
- ⌘N: New chat (will be implemented in Issue #12)

### Liquid Glass Integration

**GlassEffectContainer**
- Wrap the entire `NavigationSplitView` in a `GlassEffectContainer` for optimal performance
- The container automatically morphs and merges nearby glass shapes
- Single container per scene is recommended (don't nest multiple containers)

**Glass Effect IDs**
- Tag each pane with `.glassEffectID(_:)` for smooth transitions when toggling visibility
- IDs: "sidebar", "chat-area", "inspector"

**Scroll Edge Effects**
- Use **hard** scroll edge effect style for macOS (stronger boundary for text and pinned headers)
- Apply one scroll edge effect per scrollable pane (don't stack or mix styles)
- Edge effects clarify where UI meets content without hard dividers

**Example placeholder with glass background:**
```swift
struct SidebarPlaceholderView: View {
    var body: some View {
        ScrollView {
            Text("Sidebar Placeholder")
                .padding()
        }
        .background {
            if #available(macOS 15.0, *) {
                Rectangle()
                    .fill(.clear)
                    .glassEffect(.regular, in: Rectangle())
            } else {
                Color.clear
                    .background(.ultraThinMaterial)
            }
        }
        .scrollEdgeEffect(.hard)  // macOS style
    }
}
```

**Material Variants**
- Use `.regular` for most surfaces
- Use `.clear` for areas needing higher contrast
- Always provide fallback to `.ultraThinMaterial` for macOS 13-14

### Styling Considerations
- Use `OATheme.Colors` for consistent theming
- Apply Liquid Glass via `GlassEffectContainer` (macOS 15+) with fallback to `.ultraThin` material
- Berkeley Mono font via `OAFonts` for consistent typography
- Concentric shapes for cards and containers (align radii with parent padding)

## Dependencies
None - This is the foundation issue

## Blocked By
None

## Blocks
- Issue #4 (Session history sidebar)
- Issue #5 (Main chat area)
- Issue #6 (Inspector pane)

## Estimated Complexity
Medium (2-3 hours)

## Testing Requirements
- Build succeeds on macOS target
- All three panes render correctly
- Dividers are draggable
- Keyboard shortcuts work
- Preview renders in Xcode Canvas

## References
- Apple NavigationSplitView docs: https://developer.apple.com/documentation/swiftui/navigationsplitview
- Liquid Glass APIs: `docs/liquid-glass/apis-and-implementation.md`
- Liquid Glass structure: `docs/liquid-glass/structure-and-navigation.md`
- GlassEffectContainer: https://developer.apple.com/documentation/swiftui/glasseffectcontainer
- Existing iOS navigation: `ios/OpenAgents/Views/NewChatView.swift`
- Current macOS view: `ios/OpenAgents/SimplifiedMacOSView.swift`
