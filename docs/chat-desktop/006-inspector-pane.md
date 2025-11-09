# Issue #6: Create Collapsible Inspector Pane (Tool Details, JSON Viewer)

## Phase
Phase 2: Main UI Components

## Priority
Medium - Developer/power-user feature

## Description
Build the right inspector pane for displaying detailed information about tool calls, JSON output, and message metadata.

## Current State
- iOS has detail sheets (`MessageDetailSheet`, `ToolCallDetailSheet`, `JSONInspectorView`)
- Modal presentation doesn't work well for desktop persistent inspector
- No persistent inspector UI on macOS

## Target State
- Right pane shows detailed information for selected message/tool call
- Sections:
  - **Message Info**: Timestamp, role, session ID
  - **Tool Call Details**: Name, parameters, status, execution time
  - **Output**: JSON output with syntax highlighting
  - **Raw JSON**: Full message JSON for debugging
- Collapsible sections with disclosure triangles
- Syntax-highlighted JSON viewer
- Copy buttons for JSON/output
- Toggle inspector visibility with ⌘I
- Empty state when nothing selected

## Acceptance Criteria
- [ ] Create `InspectorPaneView.swift` for right pane
- [ ] Display selected message/tool call details
- [ ] JSON syntax highlighting using `JSONInspectorView` or similar
- [ ] Copy buttons for code/JSON blocks
- [ ] Collapsible sections (Message, Tool, Output, Raw JSON)
- [ ] Empty state when nothing selected
- [ ] Keyboard shortcut ⌘I to toggle inspector
- [ ] Persist inspector visibility in UserDefaults
- [ ] Smooth animations for show/hide

## Technical Details

### File Structure
```swift
// ios/OpenAgents/Views/macOS/InspectorPaneView.swift
struct InspectorPaneView: View {
    @EnvironmentObject var bridgeManager: BridgeManager
    @Binding var selectedUpdateIndex: Int?
    @State private var expandedSections: Set<InspectorSection> = [.message, .tool, .output]

    enum InspectorSection: String, CaseIterable {
        case message = "Message Info"
        case tool = "Tool Call"
        case output = "Output"
        case rawJSON = "Raw JSON"
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Inspector")
                    .font(OAFonts.mono(size: 14, weight: .semibold))
                    .foregroundColor(OATheme.Colors.textPrimary)

                Spacer()

                Button(action: { selectedUpdateIndex = nil }) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(OATheme.Colors.textSecondary)
                }
                .buttonStyle(.plain)
            }
            .padding()
            .background(OATheme.Colors.background.opacity(0.5))

            Divider()

            // Content
            if let index = selectedUpdateIndex,
               index < bridgeManager.updates.count {
                let update = bridgeManager.updates[index]

                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        // Message Info Section
                        InspectorSectionView(
                            title: InspectorSection.message.rawValue,
                            isExpanded: expandedSections.contains(.message),
                            onToggle: { toggleSection(.message) }
                        ) {
                            MessageInfoView(update: update)
                        }

                        Divider()

                        // Tool Call Section (if applicable)
                        if case .toolUse(let toolUse) = update.content {
                            InspectorSectionView(
                                title: InspectorSection.tool.rawValue,
                                isExpanded: expandedSections.contains(.tool),
                                onToggle: { toggleSection(.tool) }
                            ) {
                                ToolCallInfoView(toolUse: toolUse)
                            }

                            Divider()
                        }

                        // Output Section (if applicable)
                        if case .toolUse(let toolUse) = update.content,
                           let output = bridgeManager.outputJSONByCallId[toolUse.id] {
                            InspectorSectionView(
                                title: InspectorSection.output.rawValue,
                                isExpanded: expandedSections.contains(.output),
                                onToggle: { toggleSection(.output) }
                            ) {
                                JSONOutputView(json: output)
                            }

                            Divider()
                        }

                        // Raw JSON Section
                        InspectorSectionView(
                            title: InspectorSection.rawJSON.rawValue,
                            isExpanded: expandedSections.contains(.rawJSON),
                            onToggle: { toggleSection(.rawJSON) }
                        ) {
                            RawJSONView(update: update)
                        }
                    }
                }
            } else {
                // Empty state
                InspectorEmptyStateView()
            }

            Spacer()
        }
        .frame(minWidth: 280, idealWidth: 300, maxWidth: 350)
        .background {
            // Liquid Glass inspector material
            if #available(macOS 15.0, *) {
                Rectangle()
                    .fill(.clear)
                    .glassEffect(.regular, in: Rectangle())
            } else {
                Material.ultraThin
            }
        }
        .scrollEdgeEffect(.hard)  // macOS edge effect for text/headers
    }

    private func toggleSection(_ section: InspectorSection) {
        if expandedSections.contains(section) {
            expandedSections.remove(section)
        } else {
            expandedSections.insert(section)
        }
    }
}

struct InspectorSectionView<Content: View>: View {
    let title: String
    let isExpanded: Bool
    let onToggle: () -> Void
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 0) {
            // Section header
            Button(action: onToggle) {
                HStack {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(.system(size: 12))
                        .foregroundColor(OATheme.Colors.textSecondary)

                    Text(title)
                        .font(OAFonts.mono(size: 12, weight: .semibold))
                        .foregroundColor(OATheme.Colors.textPrimary)

                    Spacer()
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            // Section content
            if isExpanded {
                content
                    .padding(.horizontal, 12)
                    .padding(.bottom, 12)
            }
        }
    }
}

struct MessageInfoView: View {
    let update: ACP.Client.SessionNotificationWire

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            InfoRowView(label: "Role", value: update.role.rawValue.capitalized)
            InfoRowView(label: "Type", value: update.content.type)
            if let sessionId = update.sessionId {
                InfoRowView(label: "Session", value: sessionId)
            }
            // Add timestamp, index, etc.
        }
    }
}

struct ToolCallInfoView: View {
    let toolUse: ACP.Client.ToolUse

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            InfoRowView(label: "Tool", value: toolUse.name)
            InfoRowView(label: "Call ID", value: toolUse.id)

            Text("Parameters")
                .font(OAFonts.mono(size: 11, weight: .semibold))
                .foregroundColor(OATheme.Colors.textSecondary)
                .padding(.top, 4)

            // Show parameters (formatted JSON or key-value pairs)
            JSONOutputView(json: toolUse.input.description)
        }
    }
}

struct JSONOutputView: View {
    let json: String
    @State private var copied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Spacer()
                Button(action: copyJSON) {
                    Label(copied ? "Copied!" : "Copy", systemImage: "doc.on.doc")
                        .font(OAFonts.mono(size: 11))
                }
                .buttonStyle(.bordered)
            }

            ScrollView(.horizontal) {
                Text(json)
                    .font(OAFonts.mono(size: 11))
                    .foregroundColor(OATheme.Colors.textPrimary)
                    .textSelection(.enabled)
                    .padding(8)
                    .background(OATheme.Colors.background.opacity(0.8))
                    .cornerRadius(6)
            }
        }
    }

    private func copyJSON() {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(json, forType: .string)
        copied = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            copied = false
        }
    }
}

struct RawJSONView: View {
    let update: ACP.Client.SessionNotificationWire

    var body: some View {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try? encoder.encode(update)
        let json = data.flatMap { String(data: $0, encoding: .utf8) } ?? "{}"

        JSONOutputView(json: json)
    }
}

struct InfoRowView: View {
    let label: String
    let value: String

    var body: some View {
        HStack(alignment: .top) {
            Text(label)
                .font(OAFonts.mono(size: 11))
                .foregroundColor(OATheme.Colors.textSecondary)
                .frame(width: 80, alignment: .leading)

            Text(value)
                .font(OAFonts.mono(size: 11))
                .foregroundColor(OATheme.Colors.textPrimary)
                .textSelection(.enabled)

            Spacer()
        }
    }
}

struct InspectorEmptyStateView: View {
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "info.circle")
                .font(.system(size: 48))
                .foregroundColor(OATheme.Colors.textSecondary)

            Text("No selection")
                .font(OAFonts.mono(size: 14))
                .foregroundColor(OATheme.Colors.textPrimary)

            Text("Click a message or tool call to view details")
                .font(OAFonts.mono(size: 11))
                .foregroundColor(OATheme.Colors.textSecondary)
                .multilineTextAlignment(.center)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
```

### Integration with ChatAreaView
```swift
// In ChatMacOSView or ChatAreaView
@State private var selectedUpdateIndex: Int?

// Pass to inspector
InspectorPaneView(selectedUpdateIndex: $selectedUpdateIndex)

// In MessageBubbleView, add tap gesture to select
.onTapGesture {
    selectedUpdateIndex = index
}
```

### Keyboard Shortcut
```swift
// In ChatMacOSView
.keyboardShortcut("i", modifiers: .command)
```

## Dependencies
- Issue #1 (Three-pane layout - provides inspector container)
- Issue #2 (BridgeManager chat state - provides `outputJSONByCallId`)

## Blocked By
- Issue #1
- Issue #2

## Blocks
None - This is an enhancement, not blocking

## Estimated Complexity
Medium (4-5 hours)

## Testing Requirements
- [ ] Build succeeds on macOS target
- [ ] Inspector shows when message selected
- [ ] All sections expand/collapse correctly
- [ ] JSON syntax highlighting works
- [ ] Copy buttons work
- [ ] ⌘I toggles inspector visibility
- [ ] Empty state shows when nothing selected
- [ ] Layout doesn't break with long JSON

### Liquid Glass Inspector Features

**Material**
- Inspector pane uses Liquid Glass `.regular` variant
- Consistent with sidebar and chat area materials
- Collapsible sections can have subtle glass backgrounds

**Scroll Edge Effect**
- Use `.scrollEdgeEffect(.hard)` - appropriate for text and code viewing
- Single edge effect for the scrollable inspector content

**Section Expansion**
- Smooth animations when expanding/collapsing sections
- Use spring animations: `.animation(.spring(response: 0.3, dampingFraction: 0.7), value: expandedSections)`

## References
- iOS detail sheets: `ios/OpenAgents/ACP/MessageDetailSheet.swift`, `ToolCallDetailSheet.swift`
- JSON viewer: `ios/OpenAgents/Views/JSONInspectorView.swift`
- Liquid Glass structure: `docs/liquid-glass/structure-and-navigation.md`
- Existing inspector patterns in Xcode, VS Code for reference
