# Issue #5: Build Main Chat Area (Adapt iOS UpdatesListView/ACPTimelineView)

## Phase
Phase 2: Main UI Components

## Priority
Critical - Core chat functionality

## Description
Create the main chat area in the center pane by adapting iOS's `UpdatesListView` and `ACPTimelineView` components for macOS.

## Current State
- macOS chat area implemented on main as `ChatAreaView`.
- iOS has fully functional chat components:
  - `UpdatesListView.swift` - Renders ACP updates as message stream
  - `ACPTimelineView.swift` - Alternative timeline-based renderer
  - `UpdateRow.swift` - Individual message/tool call rendering
- All ACP renderers exist and are platform-agnostic (`ToolCallView`, `PlanView`, etc.)
- macOS has no chat UI

## Target State
- Center pane displays scrollable message list
- Messages rendered using existing ACP components
- Auto-scroll to bottom on new messages
- User messages right-aligned, agent messages left-aligned (or ChatGPT-style full-width)
- Tool calls, plans, reasoning blocks rendered inline
- Composer at bottom (from Issue #3)
- Loading states during agent processing
- Smooth animations for new messages

## Acceptance Criteria
- [x] Create `ChatAreaView.swift` for center pane (macOS)
- [x] Integrate `ComposerMac` at bottom
- [x] Render messages from `bridgeManager.updates`
- [x] Reuse existing ACP renderers where feasible (ToolCallView)
- [x] Auto-scroll to bottom on new messages
- [x] Empty state when no messages (prompt and icon)
- [x] Proper spacing and padding for readability
- [x] Handle long messages with wrapping, text selection enabled
- [ ] Typing indicator (deferred)
- [ ] Plan rendering via PlanStateView (deferred; currently hidden inline)
- [ ] Detail sheets for full inspection (deferred)
- [ ] Scroll edge effect and glass background (deferred; using OATheme dark surfaces per current directive)

## Technical Details

### File Structure
```swift
// ios/OpenAgents/Views/macOS/ChatAreaView.swift
struct ChatAreaView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @State private var messageText = ""
    @State private var isSending = false
    @State private var scrollProxy: ScrollViewProxy?

    var body: some View {
        VStack(spacing: 0) {
            // Messages area
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 16) {
                        if bridgeManager.updates.isEmpty {
                            EmptyStateView()
                        } else {
                            ForEach(bridgeManager.updates.indices, id: \.self) { index in
                                MessageBubbleView(
                                    update: bridgeManager.updates[index],
                                    bridgeManager: bridgeManager
                                )
                                .id(index)
                            }
                        }

                        // Auto-scroll anchor
                        Color.clear
                            .frame(height: 1)
                            .id("bottom")
                    }
                    .padding()
                }
                .scrollEdgeEffect(.hard)  // macOS hard edge effect
                .background {
                    // Liquid Glass background for chat area
                    if #available(macOS 15.0, *) {
                        Rectangle()
                            .fill(.clear)
                            .glassEffect(.regular, in: Rectangle())
                    } else {
                        OATheme.Colors.background
                    }
                }
                .onAppear {
                    scrollProxy = proxy
                    scrollToBottom()
                }
                .onChange(of: bridgeManager.updates.count) { _ in
                    scrollToBottom()
                }
            }

            Divider()

            // Composer (has its own glass background)
            ComposerMac(
                text: $messageText,
                placeholder: "Ask an agent...",
                isSending: isSending,
                onSend: sendMessage
            )
            .padding()
        }
    }

    private func sendMessage() {
        guard !messageText.isEmpty else { return }
        bridgeManager.sendPrompt(text: messageText)
        messageText = ""
        scrollToBottom()
    }

    private func scrollToBottom() {
        withAnimation {
            scrollProxy?.scrollTo("bottom", anchor: .bottom)
        }
    }
}

struct MessageBubbleView: View {
    let update: ACP.Client.SessionNotificationWire
    @ObservedObject var bridgeManager: BridgeManager
    @State private var showDetail = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Message header (role, timestamp)
            HStack {
                Text(roleLabel)
                    .font(OAFonts.mono(size: 11))
                    .foregroundColor(OATheme.Colors.textSecondary)

                Spacer()

                Text(timestamp)
                    .font(OAFonts.mono(size: 11))
                    .foregroundColor(OATheme.Colors.textSecondary)
            }

            // Content
            switch update.content {
            case .text(let text):
                Text(text.text)
                    .font(OAFonts.mono(size: 14))
                    .foregroundColor(OATheme.Colors.textPrimary)
                    .textSelection(.enabled)

            case .toolUse(let toolUse):
                ToolCallView(
                    callId: toolUse.id,
                    toolName: toolUse.name,
                    status: .running, // Derive from state
                    percentage: nil,
                    bridgeManager: bridgeManager
                )
                .onTapGesture {
                    showDetail = true
                }

            case .toolResult(let result):
                // Show result preview or success/error state
                ToolResultView(result: result)

            case .thinking(let thinking):
                ReasoningSummaryView(content: thinking.content)
                    .onTapGesture {
                        showDetail = true
                    }

            default:
                // Handle other content types
                Text("[\(update.content.type)]")
                    .font(OAFonts.mono(size: 12))
                    .foregroundColor(OATheme.Colors.textSecondary)
            }
        }
        .padding()
        .background(bubbleBackground)
        .cornerRadius(12)
        .sheet(isPresented: $showDetail) {
            MessageDetailSheet(update: update, bridgeManager: bridgeManager)
        }
    }

    private var bubbleBackground: Color {
        switch update.role {
        case .user:
            return OATheme.Colors.accent.opacity(0.1)
        case .assistant:
            return OATheme.Colors.background.opacity(0.8)
        default:
            return Color.clear
        }
    }

    private var roleLabel: String {
        switch update.role {
        case .user: return "You"
        case .assistant: return "Agent"
        default: return "System"
        }
    }

    private var timestamp: String {
        // Format timestamp from update
        let formatter = DateFormatter()
        formatter.timeStyle = .short
        return formatter.string(from: Date()) // Use update.timestamp if available
    }
}

struct EmptyStateView: View {
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "bubble.left.and.bubble.right")
                .font(.system(size: 64))
                .foregroundColor(OATheme.Colors.textSecondary)

            Text("Start a conversation")
                .font(OAFonts.mono(size: 18))
                .foregroundColor(OATheme.Colors.textPrimary)

            Text("Ask an agent to help with coding, debugging, or research")
                .font(OAFonts.mono(size: 13))
                .foregroundColor(OATheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
        }
        .padding(64)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct ToolResultView: View {
    let result: ACP.Client.ToolResult

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: result.isError ? "xmark.circle.fill" : "checkmark.circle.fill")
                .foregroundColor(result.isError ? OATheme.Colors.danger : OATheme.Colors.success)

            Text(result.isError ? "Tool execution failed" : "Tool execution complete")
                .font(OAFonts.mono(size: 13))
                .foregroundColor(OATheme.Colors.textSecondary)
        }
        .padding(8)
        .background(OATheme.Colors.background.opacity(0.5))
        .cornerRadius(8)
    }
}
```

### Reuse Existing Components
- **ToolCallView** (`ios/OpenAgents/ACP/ToolCallView.swift`) - Already platform-agnostic
- **PlanView/PlanStateView** - For plan rendering
- **ReasoningSummaryView** - For thinking blocks
- **MessageDetailSheet** - For full message inspection

### Auto-Scroll Logic
- Scroll to bottom on new message
- Don't auto-scroll if user has scrolled up manually
- Smooth animation for scroll

### Loading States
Show typing indicator when agent is processing:
```swift
if bridgeManager.isAgentProcessing {
    HStack {
        ProgressView()
            .scaleEffect(0.6)
        Text("Agent is thinking...")
            .font(OAFonts.mono(size: 13))
            .foregroundColor(OATheme.Colors.textSecondary)
    }
    .padding()
}
```

## Dependencies
- Issue #1 (Three-pane layout - provides center container)
- Issue #2 (BridgeManager chat state - provides `updates` array)
- Issue #3 (Composer - for message input)

## Blocked By
- Issue #1
- Issue #2
- Issue #3

## Blocks
- Issue #11 (Chat integration - needs rendering)

## Estimated Complexity
Medium-High (5-7 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] Messages render correctly from `updates` array
- [ ] Auto-scroll works on new messages
- [ ] User can manually scroll up without auto-scroll interference
- [ ] Empty state shows when no messages
- [ ] Tool calls render correctly using `ToolCallView`
- [ ] Plans render correctly using `PlanView`
- [ ] Composer integration works (send message appears in chat)
- [ ] Detail sheets open on click
- [ ] Text selection works for messages

### Surface and Scroll Effects

For now, use OATheme dark surfaces (no glass) to match current app direction. We can revisit hard scroll edge effects and glass once the theme settles.

**Message Bubbles**
- User messages: subtle accent color background with concentric rounded rect
- Agent messages: transparent or subtle background on glass
- Use concentric shapes when bubbles nest within cards

## References
- iOS UpdatesListView: `ios/OpenAgents/Views/UpdatesListView.swift`
- iOS ACPTimelineView: `ios/OpenAgents/Views/ACPTimelineView.swift`
- ACP renderers: `ios/OpenAgents/ACP/` (all files)
- MessageDetailSheet: `ios/OpenAgents/ACP/MessageDetailSheet.swift`
- Liquid Glass structure: `docs/liquid-glass/structure-and-navigation.md`
- Scroll edge effects: `docs/liquid-glass/apis-and-implementation.md`
