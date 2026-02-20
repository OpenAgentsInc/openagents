import SwiftUI

struct InboxSectionView: View {
    @EnvironmentObject private var model: AppModel
    @State private var composeText = ""
    let onOpenApprovals: () -> Void
    let onOpenAudit: () -> Void

    var body: some View {
        HStack(spacing: 0) {
            VStack(spacing: 12) {
                HStack {
                    TextField("Search subject, sender, snippet", text: $model.searchText)
                        .textFieldStyle(.roundedBorder)
                        .onSubmit {
                            Task { await model.refreshThreads() }
                        }
                    Button("Search") {
                        Task { await model.refreshThreads() }
                    }
                }

                HStack(spacing: 8) {
                    Button("Backfill 90d") {
                        Task { await model.runBackfill() }
                    }
                    .disabled(!model.gmailConnected)

                    Button("Sync now") {
                        Task { await model.syncNow() }
                    }
                    .disabled(!model.gmailConnected)

                    Button("Approvals") {
                        onOpenApprovals()
                    }

                    Button("Audit") {
                        onOpenAudit()
                    }

                    Spacer()
                }

                List(model.threads, selection: Binding(get: {
                    model.selectedThreadID
                }, set: { selected in
                    Task { await model.openThread(id: selected) }
                })) { thread in
                    ThreadRowView(thread: thread)
                        .tag(thread.id)
                }
                .listStyle(.inset)
            }
            .padding(12)
            .frame(width: 360)

            Divider()

            ThreadDetailPane(composeText: $composeText)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}

private struct ThreadRowView: View {
    let thread: ThreadSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(thread.subject)
                .font(.headline)
                .lineLimit(1)
            Text(thread.fromAddress)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
            Text(thread.snippet)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(2)

            HStack(spacing: 6) {
                if let category = thread.category {
                    chip(category.title, color: .blue)
                }
                if let risk = thread.risk {
                    chip(risk.title, color: risk == .high ? .red : (risk == .medium ? .orange : .green))
                }
                if thread.hasPendingDraft {
                    chip("Draft", color: .mint)
                }
            }
        }
        .padding(.vertical, 4)
    }

    private func chip(_ text: String, color: Color) -> some View {
        Text(text)
            .font(.caption2)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.2), in: Capsule())
    }
}

private struct ThreadDetailPane: View {
    @EnvironmentObject private var model: AppModel
    @Binding var composeText: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let detail = model.threadDetail {
                header(detail.thread)

                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 10) {
                        ForEach(detail.messages) { message in
                            MessageBubble(message: message)
                        }
                    }
                }

                Divider()

                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text("Suggested Draft")
                            .font(.headline)
                        Spacer()
                        if let modelUsed = detail.draft?.modelUsed {
                            Text(modelUsed)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }

                    TextEditor(text: Binding(get: {
                        if !composeText.isEmpty {
                            return composeText
                        }
                        return detail.draft?.body ?? ""
                    }, set: { newValue in
                        composeText = newValue
                    }))
                    .font(.body)
                    .frame(minHeight: 140)
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(Color.gray.opacity(0.25), lineWidth: 1)
                    )

                    HStack(spacing: 8) {
                        Button("Generate Draft") {
                            Task { await model.generateDraftForSelectedThread() }
                        }
                        .disabled(model.selectedThreadID == nil)

                        Button("Insert Draft") {
                            composeText = detail.draft?.body ?? ""
                        }
                        .disabled(detail.draft == nil)

                        Button("Approve & Send") {
                            Task { await model.approveAndSendSelectedThread() }
                        }
                        .disabled(detail.draft == nil || detail.thread.policy == .blocked)

                        Button("Mark Needs Human") {
                            if let draftID = detail.draft?.id {
                                Task { await model.markNeedsHuman(draftID: draftID) }
                            }
                        }
                        .disabled(detail.draft == nil)

                        Spacer()

                        if detail.thread.policy == .blocked {
                            Text("Blocked by policy")
                                .foregroundStyle(.red)
                                .font(.caption)
                        }
                    }
                }
            } else {
                Spacer()
                Text("Select a thread to view the conversation and draft.")
                    .foregroundStyle(.secondary)
                Spacer()
            }
        }
        .padding(14)
    }

    private func header(_ thread: ThreadSummary) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(thread.subject)
                .font(.title3)
                .fontWeight(.semibold)
            HStack(spacing: 8) {
                Text(thread.fromAddress)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                if let category = thread.category {
                    chip(category.title, color: .blue)
                }
                if let risk = thread.risk {
                    chip(risk.title, color: risk == .high ? .red : (risk == .medium ? .orange : .green))
                }
                if let policy = thread.policy {
                    chip(policy.title, color: .purple)
                }
            }
        }
    }

    private func chip(_ text: String, color: Color) -> some View {
        Text(text)
            .font(.caption2)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.2), in: Capsule())
    }
}

private struct MessageBubble: View {
    let message: MessageRecord

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(message.inbound ? "Inbound" : "Outbound")
                    .font(.caption)
                    .fontWeight(.semibold)
                Spacer()
                Text(message.sentAt.formatted(date: .abbreviated, time: .shortened))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            Text(message.body)
                .font(.body)
                .textSelection(.enabled)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background((message.inbound ? Color.blue : Color.green).opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
    }
}
