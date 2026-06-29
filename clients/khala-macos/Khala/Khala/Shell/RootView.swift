import SwiftUI

struct RootView: View {
    @ObservedObject var store: ConversationStore
    @ObservedObject var pylonSupervisor: PylonSupervisor
    @State private var draft = ""
    @State private var apiKeyDraft = ""
    @State private var isShowingSettings = false
    @State private var isSending = false
    @State private var banner: String?

    var body: some View {
        NavigationSplitView { sidebar } content: { chatPane } detail: { inspector }
            .navigationTitle("Khala")
            .toolbar {
                ToolbarItemGroup {
                    Text("Khala").font(.caption.weight(.semibold)).padding(.horizontal, 10).padding(.vertical, 5).background(.quaternary, in: Capsule())
                    Button { store.createConversation() } label: { Label("New Chat", systemImage: "square.and.pencil") }
                    Button { isShowingSettings.toggle() } label: { Label("Settings", systemImage: "gearshape") }
                }
            }
            .sheet(isPresented: $isShowingSettings) { settings }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Conversations").font(.headline)
                Spacer()
                Button { store.createConversation() } label: { Image(systemName: "plus") }.buttonStyle(.borderless).help("New Chat")
            }
            List(selection: Binding(get: { store.selectedConversationID }, set: { id in
                guard let id, let conversation = store.conversations.first(where: { $0.id == id }) else { return }
                store.select(conversation)
            })) {
                ForEach(store.conversations) { conversation in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(conversation.title).lineLimit(1)
                        Text(conversation.updatedAt, style: .relative).font(.caption).foregroundStyle(.secondary)
                    }
                    .tag(conversation.id)
                    .contextMenu { Button("Delete") { store.delete(conversation) } }
                }
            }
            Divider()
            StatusLine(title: "Local Pylon", value: pylonSupervisor.snapshot.pylonStatusText, systemImage: "bolt.horizontal")
            StatusLine(title: "Apple FM", value: pylonSupervisor.snapshot.capacitySummary.contains("ready=1") ? "Ready" : "Not attached", systemImage: "cpu")
            StatusLine(title: "Provider Mode", value: pylonSupervisor.snapshot.providerStatusText, systemImage: "antenna.radiowaves.left.and.right")
        }
        .padding()
        .frame(minWidth: 250)
    }

    private var chatPane: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 14) {
                        if let text = store.persistenceError { BannerView(text: text, systemImage: "exclamationmark.triangle") }
                        if let banner { BannerView(text: banner, systemImage: "info.circle") }
                        ForEach(store.selectedConversation?.messages ?? []) { message in MessageRow(message: message).id(message.id) }
                        if store.selectedConversation?.messages.isEmpty ?? true { emptyState }
                    }.padding(24)
                }
                .onChange(of: store.selectedConversation?.messages.last?.id) { _, id in
                    guard let id else { return }
                    withAnimation { proxy.scrollTo(id, anchor: .bottom) }
                }
            }
            Divider()
            composer
        }
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Khala").font(.largeTitle.bold())
            Text("Ask a question or send a bounded public coding task prompt.").foregroundStyle(.secondary)
            Text("Model: \(KhalaClient.model)").font(.caption.monospaced()).foregroundStyle(.secondary)
        }.frame(maxWidth: .infinity, alignment: .leading).padding(.top, 120)
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 12) {
            TextField("Message Khala", text: $draft, axis: .vertical)
                .textFieldStyle(.plain).lineLimit(1...6).padding(12).background(.quinary, in: RoundedRectangle(cornerRadius: 8)).onSubmit(send)
            Button { send() } label: { Image(systemName: isSending ? "hourglass" : "paperplane.fill") }
                .keyboardShortcut(.return, modifiers: [.command]).disabled(isSending || draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty).buttonStyle(.borderedProminent).help("Send")
        }.padding()
    }

    private var inspector: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text("Node").font(.title2.bold())
                InspectorCard(title: "Pylon Supervisor", status: pylonSupervisor.snapshot.pylonStatusText) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(pylonSupervisor.snapshot.controlURL.absoluteString).font(.caption.monospaced())
                        Text(pylonSupervisor.snapshot.pylonHome.path).font(.caption.monospaced())
                        if let error = pylonSupervisor.snapshot.lastError { Text(error) }
                    }
                }
                InspectorCard(title: "Apple FM Bridge", status: pylonSupervisor.snapshot.capacitySummary.contains("ready=1") ? "Ready" : "Unavailable") { Text(pylonSupervisor.snapshot.capacitySummary).lineLimit(6) }
                InspectorCard(title: "Fleet", status: pylonSupervisor.snapshot.accountsSummary == "Not loaded" ? "Loading" : "Visible") {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(pylonSupervisor.snapshot.identitySummary).lineLimit(3)
                        Text(pylonSupervisor.snapshot.accountsSummary).lineLimit(6)
                        Text(pylonSupervisor.snapshot.assignmentsSummary).lineLimit(6)
                    }
                }
                Button { Task { await pylonSupervisor.refreshReadiness() } } label: { Label("Refresh Node", systemImage: "arrow.clockwise") }
                Text("Privacy").font(.headline)
                Text("The API key is stored in Keychain. Local chat history and the bundled Pylon home stay in Application Support on this Mac. The supervisor never writes to the default Codex home.").foregroundStyle(.secondary)
            }.padding(24)
        }.frame(minWidth: 300)
    }

    private var settings: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Settings").font(.title2.bold())
            Text("Paste an `oa_agent_...` key for Khala chat. The value is stored in Keychain and is not displayed after saving.").foregroundStyle(.secondary)
            SecureField("Khala API key", text: $apiKeyDraft).textFieldStyle(.roundedBorder)
            HStack {
                Button("Save Key") {
                    let key = apiKeyDraft.trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !key.isEmpty else { return }
                    KeychainStore.saveAPIKey(key); apiKeyDraft = ""; isShowingSettings = false
                }.buttonStyle(.borderedProminent)
                Button("Remove Key") { KeychainStore.deleteAPIKey(); apiKeyDraft = "" }
                Spacer()
                Button("Done") { isShowingSettings = false }
            }
        }.padding(24).frame(width: 460)
    }

    private func send() {
        let prompt = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty, !isSending else { return }
        guard let apiKey = KeychainStore.loadAPIKey() else { banner = "Add a Khala API key in Settings before sending."; isShowingSettings = true; return }
        let conversationID = store.selectedConversation?.id ?? store.createConversation().id
        draft = ""; banner = nil
        store.appendMessage(.user, content: prompt, to: conversationID)
        guard let assistant = store.appendMessage(.assistant, content: "Thinking...", to: conversationID) else { return }
        isSending = true
        Task {
            do {
                let response = try await KhalaClient.complete(prompt: prompt, apiKey: apiKey)
                await MainActor.run { store.updateMessage(assistant.id, in: conversationID, content: response); isSending = false }
            } catch {
                await MainActor.run { store.updateMessage(assistant.id, in: conversationID, content: (error as? LocalizedError)?.errorDescription ?? error.localizedDescription); isSending = false }
            }
        }
    }
}

private struct MessageRow: View {
    let message: ChatMessage
    var body: some View { HStack(alignment: .top) { if message.role == .assistant { content; Spacer(minLength: 80) } else { Spacer(minLength: 80); content } } }
    private var content: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(message.role == .assistant ? "Khala" : "You").font(.caption.weight(.semibold)).foregroundStyle(.secondary)
            Text(message.content).textSelection(.enabled).padding(12).background(message.role == .assistant ? Color(nsColor: .controlBackgroundColor) : Color.accentColor.opacity(0.18)).clipShape(RoundedRectangle(cornerRadius: 8))
        }.frame(maxWidth: 680, alignment: .leading)
    }
}

private struct StatusLine: View {
    let title: String
    let value: String
    let systemImage: String
    var body: some View { HStack(spacing: 8) { Image(systemName: systemImage).frame(width: 18); VStack(alignment: .leading, spacing: 2) { Text(title).font(.caption).foregroundStyle(.secondary); Text(value).font(.callout.weight(.medium)) }; Spacer() } }
}

private struct InspectorCard<Content: View>: View {
    let title: String
    let status: String
    @ViewBuilder let content: Content
    var body: some View { VStack(alignment: .leading, spacing: 8) { HStack { Text(title).font(.headline); Spacer(); Text(status).font(.caption.weight(.semibold)).padding(.horizontal, 8).padding(.vertical, 4).background(.quaternary, in: Capsule()) }; content.font(.callout).foregroundStyle(.secondary) }.padding(14).background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8)) }
}

private struct BannerView: View {
    let text: String
    let systemImage: String
    var body: some View { HStack(spacing: 8) { Image(systemName: systemImage); Text(text); Spacer() }.font(.callout).padding(10).background(Color.yellow.opacity(0.16), in: RoundedRectangle(cornerRadius: 8)) }
}
