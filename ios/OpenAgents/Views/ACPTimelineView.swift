import SwiftUI
import OpenAgentsCore

struct ACPTimelineView: View {
    let items: [ACPTimelineViewModel.Item]

    @State private var selectedReasoning: ReasoningSummary?
    @State private var selectedMessage: MessageDetail?

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 10) {
                    ForEach(items) { item in
                        switch item {
                        case .message(let role, let text, let ts):
                            MessageRow(
                                role: role,
                                text: text,
                                onTap: {
                                    selectedMessage = MessageDetail(
                                        id: "msg_\(ts)",
                                        text: text,
                                        rawJSON: nil
                                    )
                                }
                            )
                            .id(item.id)

                        case .toolCall(let call):
                            ToolCallView(call: call, result: findResult(for: call))
                                .id(item.id)

                        case .toolResult:
                            // Don't render tool results as separate items
                            // Results are shown via status indicator on the tool call itself
                            EmptyView()

                        case .reasoning(let summary):
                            HStack {
                                Spacer()
                                ReasoningSummaryView(summary: summary) {
                                    selectedReasoning = summary
                                }
                                Spacer()
                            }
                            .id(item.id)

                        case .plan(let plan, _):
                            PlanView(plan: plan)
                                .id(item.id)
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
        .sheet(item: $selectedReasoning) { summary in
            ReasoningDetailSheet(summary: summary, isPresented: .init(
                get: { selectedReasoning != nil },
                set: { if !$0 { selectedReasoning = nil } }
            ))
        }
        .sheet(item: $selectedMessage) { message in
            MessageDetailSheet(message: message, isPresented: .init(
                get: { selectedMessage != nil },
                set: { if !$0 { selectedMessage = nil } }
            ))
        }
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
        let onTap: () -> Void

        var body: some View {
            HStack {
                if role == .assistant { Spacer(minLength: 40) }
                Button(action: onTap) {
                    Text(text)
                        .font(OAFonts.ui(.body, 15))
                        .foregroundStyle(role == .user ? Color.white : OATheme.Colors.textPrimary)
                        .multilineTextAlignment(.leading)
                        .frame(maxWidth: .infinity, alignment: .leading)
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
                }
                .buttonStyle(.plain)
                if role == .user { Spacer(minLength: 40) }
            }
        }
    }
}

