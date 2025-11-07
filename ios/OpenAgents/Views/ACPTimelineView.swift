import SwiftUI
import OpenAgentsCore

#if os(iOS)

struct ACPTimelineView: View {
    let items: [ACPTimelineViewModel.Item]

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 10) {
                    ForEach(items) { item in
                        switch item {
                        case .message(let role, let text, _):
                            MessageRow(role: role, text: text)
                                .id(item.id)
                        case .toolCall(let call):
                            ToolCallView(call: call, result: findResult(for: call))
                                .id(item.id)
                        case .toolResult:
                            // Don't render tool results as separate items
                            // Results are shown via status indicator on the tool call itself
                            EmptyView()
                        }
                    }
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                // Leave bottom room for the composer overlay
                .padding(.bottom, 72)
            }
        }
        .background(OATheme.Colors.background)
    }

    /// Find the matching result for a tool call
    private func findResult(for call: ACPToolCall) -> ACPToolResult? {
        for item in items {
            if case .toolResult(let result) = item, result.call_id == call.id {
                return result
            }
        }
        return nil
    }

    private struct MessageRow: View {
        let role: ACPTimelineViewModel.Item.Role
        let text: String

        var body: some View {
            HStack {
                if role == .assistant { Spacer(minLength: 40) }
                Text(text)
                    .font(OAFonts.ui(.body, 15))
                    .foregroundStyle(role == .user ? Color.white : OATheme.Colors.textPrimary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 10)
                    .background(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(role == .user ? OATheme.Colors.accent.opacity(0.85) : Color.black.opacity(0.25))
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .stroke(OATheme.Colors.border.opacity(0.25), lineWidth: 1)
                    )
                if role == .user { Spacer(minLength: 40) }
            }
        }
    }
}

#endif

