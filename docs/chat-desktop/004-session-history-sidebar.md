# Issue #4: Implement Session History Sidebar with Search/Filtering

## Phase
Phase 2: Main UI Components

## Priority
High - Core ChatGPT-style feature

## Description
Build the left sidebar that displays session history with search functionality, mimicking ChatGPT's session list UI.

## Current State
- No session history UI on macOS
- iOS has `HistoryListView` but designed for mobile sheet presentation
- `TinyvexManager` can store/retrieve sessions but no desktop UI

## Target State
- Left sidebar showing all chat sessions
- Sessions grouped by date (Today, Yesterday, Last 7 Days, Last 30 Days, Older)
- Search/filter functionality at the top
- Each session shows:
  - AI-generated title (via Foundation Models) or first message snippet
  - Timestamp
  - Active/selected state
- "New Chat" button at the top
- Delete session on hover/right-click
- Smooth animations for selection changes

## Acceptance Criteria
- [ ] Create `SessionSidebarView.swift` for the left pane
- [ ] Display sessions grouped by date
- [ ] Implement search bar with live filtering
- [ ] Show "New Chat" button at top (use capsule shape per Liquid Glass)
- [ ] Highlight currently active session
- [ ] Click session to load it in chat area
- [ ] Hover shows delete button (or context menu)
- [ ] Use Foundation Models for auto-generated titles (fallback to snippet)
- [ ] Apply Liquid Glass material to sidebar background
- [ ] Use hard scroll edge effect for macOS (pinned section headers)
- [ ] Empty state when no sessions exist
- [ ] Keyboard navigation (↑/↓ arrows, Enter to select)

## Technical Details

### File Structure
```swift
// ios/OpenAgents/Views/macOS/SessionSidebarView.swift
struct SessionSidebarView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @State private var searchText = ""
    @State private var sessions: [ThreadSummary] = []
    @State private var hoveredSessionId: String?

    var body: some View {
        VStack(spacing: 0) {
            // New Chat button
            Button(action: createNewSession) {
                Label("New Chat", systemImage: "plus.message")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .padding()

            // Search bar
            HStack {
                Image(systemName: "magnifyingglass")
                    .foregroundColor(OATheme.Colors.textSecondary)
                TextField("Search sessions...", text: $searchText)
                    .textFieldStyle(.plain)
                    .font(OAFonts.mono(size: 13))
                if !searchText.isEmpty {
                    Button(action: { searchText = "" }) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundColor(OATheme.Colors.textSecondary)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(8)
            .background(OATheme.Colors.background.opacity(0.5))
            .cornerRadius(8)
            .padding(.horizontal)

            Divider()

            // Session list
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 4, pinnedViews: [.sectionHeaders]) {
                    ForEach(groupedSessions.keys.sorted(), id: \.self) { group in
                        Section(header: SectionHeaderView(title: group)) {
                            ForEach(groupedSessions[group] ?? [], id: \.id) { session in
                                SessionRowView(
                                    session: session,
                                    isActive: session.id == bridgeManager.currentSessionId,
                                    isHovered: session.id == hoveredSessionId,
                                    onSelect: { loadSession(session) },
                                    onDelete: { deleteSession(session) }
                                )
                                .onHover { isHovered in
                                    hoveredSessionId = isHovered ? session.id : nil
                                }
                            }
                        }
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
            }

            Spacer()
        }
        .frame(minWidth: 220, idealWidth: 250, maxWidth: 280)
        .background {
            // Liquid Glass sidebar material
            if #available(macOS 15.0, *) {
                Rectangle()
                    .fill(.clear)
                    .glassEffect(.regular, in: Rectangle())
            } else {
                Color.clear
                    .background(.ultraThinMaterial)
            }
        }
        .scrollEdgeEffect(.hard)  // macOS hard edge effect for text/pinned headers
        .onAppear(perform: loadSessions)
    }

    private var filteredSessions: [ThreadSummary] {
        guard !searchText.isEmpty else { return sessions }
        return sessions.filter { session in
            session.title.localizedCaseInsensitiveContains(searchText) ||
            (session.firstMessage?.localizedCaseInsensitiveContains(searchText) ?? false)
        }
    }

    private var groupedSessions: [String: [ThreadSummary]] {
        // Group by "Today", "Yesterday", "Last 7 Days", etc.
        Dictionary(grouping: filteredSessions) { session in
            groupLabel(for: session.timestamp)
        }
    }

    private func groupLabel(for date: Date) -> String {
        let calendar = Calendar.current
        if calendar.isDateInToday(date) { return "Today" }
        if calendar.isDateInYesterday(date) { return "Yesterday" }
        if calendar.dateComponents([.day], from: date, to: Date()).day ?? 0 < 7 {
            return "Last 7 Days"
        }
        if calendar.dateComponents([.day], from: date, to: Date()).day ?? 0 < 30 {
            return "Last 30 Days"
        }
        return "Older"
    }

    private func loadSessions() {
        // Load via Tinyvex history RPCs through BridgeManager dispatcher
        bridgeManager.fetchRecentSessions()
        // Map `RecentSession` to `ThreadSummary` if a richer model is needed for grouping
        // Alternatively, add a small helper to fetch full timeline titles via ConversationSummarizer
    }

    private func createNewSession() {
        Task {
            await bridgeManager.startNewSession()
        }
    }

    private func loadSession(_ session: ThreadSummary) {
        bridgeManager.loadSession(session.id)
    }

    private func deleteSession(_ session: ThreadSummary) {
        // Implement server delete (JSON-RPC) if exposed; otherwise, hide until available
    }
}

struct SessionRowView: View {
    let session: ThreadSummary
    let isActive: Bool
    let isHovered: Bool
    let onSelect: () -> Void
    let onDelete: () -> Void

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text(session.title)
                    .font(OAFonts.mono(size: 13))
                    .foregroundColor(OATheme.Colors.textPrimary)
                    .lineLimit(1)

                Text(relativeTimestamp(session.timestamp))
                    .font(OAFonts.mono(size: 11))
                    .foregroundColor(OATheme.Colors.textSecondary)
            }

            Spacer()

            if isHovered {
                Button(action: onDelete) {
                    Image(systemName: "trash")
                        .font(.system(size: 12))
                        .foregroundColor(OATheme.Colors.danger)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 12)
        .background(isActive ? OATheme.Colors.accent.opacity(0.2) : Color.clear)
        .cornerRadius(8)
        .contentShape(Rectangle())
        .onTapGesture(perform: onSelect)
    }

    private func relativeTimestamp(_ date: Date) -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}

struct SectionHeaderView: View {
    let title: String

    var body: some View {
        Text(title)
            .font(OAFonts.mono(size: 11))
            .foregroundColor(OATheme.Colors.textSecondary)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background {
                // Glass effect for pinned headers
                if #available(macOS 15.0, *) {
                    Rectangle()
                        .fill(.clear)
                        .glassEffect(.regular, in: Rectangle())
                } else {
                    Material.ultraThin
                }
            }
    }
}
```

### Data Model
Use `OpenAgentsCore/ThreadSummary.swift` returned by Tinyvex history APIs; do not redefine in app layer.

### Foundation Models Integration
Use `ConversationSummarizer.summarizeTitle(messages:)` from `OpenAgentsCore/Summarization/` to generate titles from the first few messages; falls back to a deterministic snippet when FM are unavailable.

## Dependencies
- Issue #1 (Three-pane layout - provides sidebar container)
- Issue #2 (BridgeManager chat state - provides `currentSessionId`, `threads`)

## Blocked By
- Issue #1
- Issue #2

## Blocks
- Issue #12 (Session management - needs sidebar for navigation)

## Estimated Complexity
Medium-High (4-6 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] Sessions load and display grouped by date
- [ ] Search filters sessions correctly
- [ ] Clicking session loads it
- [ ] Delete button removes session
- [ ] New Chat button creates new session
- [ ] Keyboard navigation works
- [ ] Foundation Models title generation works (with fallback)
- [ ] Empty state renders when no sessions

### Liquid Glass Sidebar Features

**Material and Structure**
- Sidebar uses Liquid Glass `.regular` variant (macOS 15+) with fallback to `.ultraThinMaterial`
- Content flows naturally behind the glass material
- Section headers also use glass for consistency when pinned

**Scroll Edge Effect**
- Use `.scrollEdgeEffect(.hard)` for macOS - appropriate for text-heavy content and pinned headers
- Hard edge provides stronger visual boundary between UI and scrolling content
- Don't stack multiple edge effects - one per scrollable view

**New Chat Button**
- Use capsule shape (per Liquid Glass design guidelines)
- Could optionally use floating button pattern like composer send button
- Primary action tinted with accent color

**Session Row Hover States**
```swift
.background(isActive ? OATheme.Colors.accent.opacity(0.2) : Color.clear)
.background(isHovered ? OATheme.Colors.textSecondary.opacity(0.05) : Color.clear)
.animation(.easeInOut(duration: 0.2), value: isHovered)
```

## References
- iOS History: `ios/OpenAgents/Views/Components/HistoryListView.swift`
- Tinyvex History RPCs: `DesktopWebSocketServer+Threads.swift`, `DesktopWebSocketServer+Session.swift`
- Foundation Models: ADR-0006, `ios/OpenAgentsCore/Sources/OpenAgentsCore/Summarization/`
- Liquid Glass structure: `docs/liquid-glass/structure-and-navigation.md`
- Scroll edge effects: `docs/liquid-glass/apis-and-implementation.md`
- ThreadSummary model: `ios/OpenAgentsCore/Sources/OpenAgentsCore/ThreadSummary.swift`
