import SwiftUI

struct DesktopRootView: View {
    @ObservedObject var store: ConversationStore

    @State private var selection: UUID?
    @State private var showSettings = false
    @State private var hasKey = KeychainStore.hasAPIKey
    @State private var columnVisibility = NavigationSplitViewVisibility.all

    var body: some View {
        NavigationSplitView(columnVisibility: $columnVisibility) {
            sidebar
        } content: {
            ChatView(
                store: store,
                conversationID: activeConversation?.id,
                hasKey: $hasKey,
                onOpenSettings: { showSettings = true }
            )
        } detail: {
            NodeInspectorView(hasKey: hasKey, isUsingEphemeralFallback: store.isUsingEphemeralFallback)
        }
        .navigationTitle("Khala")
        .toolbar {
            ToolbarItemGroup {
                Button(action: newChat) {
                    Label("New Chat", systemImage: "square.and.pencil")
                }
                Button {
                    showSettings = true
                } label: {
                    Label("Settings", systemImage: "gearshape")
                }
            }
        }
        .sheet(isPresented: $showSettings, onDismiss: { hasKey = KeychainStore.hasAPIKey }) {
            SettingsView()
                .frame(width: 460)
        }
        .task {
            if store.conversations.isEmpty {
                selection = store.createConversation().id
            } else if selection == nil {
                selection = store.mostRecent?.id
            }
            if !KeychainStore.hasAPIKey,
               let token = try? await KhalaClient.mintFreeKey() {
                KeychainStore.saveAPIKey(token)
                hasKey = true
            }
        }
        .onChange(of: store.conversations) { _, conversations in
            guard selection == nil || !conversations.contains(where: { $0.id == selection }) else { return }
            selection = conversations.first?.id
        }
    }

    private var sidebar: some View {
        List(selection: $selection) {
            Section("Conversations") {
                ForEach(store.conversations) { conversation in
                    ConversationRow(conversation: conversation)
                        .tag(conversation.id)
                        .contextMenu {
                            Button("Rename") {
                                store.rename(conversation, to: Conversation.derivedTitle(from: conversation.title))
                            }
                            Button("Delete", role: .destructive) {
                                store.delete(conversation)
                            }
                        }
                }
            }

            Section("Local Node") {
                Label("Pylon not connected", systemImage: "bolt.horizontal.circle")
                Label("Apple FM unavailable", systemImage: "cpu")
                Label("Provider mode offline", systemImage: "power.circle")
            }
            .foregroundStyle(.secondary)
        }
        .safeAreaInset(edge: .bottom) {
            Button(action: newChat) {
                Label("New Chat", systemImage: "plus")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .padding()
        }
        .navigationSplitViewColumnWidth(min: 240, ideal: 280, max: 340)
    }

    private var activeConversation: Conversation? {
        if let selection, let conversation = store.conversation(id: selection) {
            return conversation
        }
        return store.mostRecent
    }

    private func newChat() {
        selection = store.createConversation().id
    }
}

private struct ConversationRow: View {
    let conversation: Conversation

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(conversation.title)
                .font(.callout.weight(.medium))
                .lineLimit(1)
            Text(conversation.updatedAt, style: .relative)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 4)
    }
}

struct SettingsView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var apiKey = KeychainStore.loadAPIKey() ?? ""
    @State private var status = ""
    @State private var isMinting = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Khala Settings")
                .font(.title2.weight(.semibold))

            Text("API key")
                .font(.headline)
            SecureField("oa_agent_...", text: $apiKey)
                .textFieldStyle(.roundedBorder)

            Text("Free keys use the public Khala API and may be used for service-quality and safety review according to the OpenAgents free-tier disclosure.")
                .font(.footnote)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            if !status.isEmpty {
                Text(status)
                    .font(.footnote.weight(.medium))
                    .foregroundStyle(.secondary)
            }

            HStack {
                Button("Mint Free Key") {
                    Task { await mint() }
                }
                .disabled(isMinting)

                Spacer()

                Button("Delete Key", role: .destructive) {
                    KeychainStore.deleteAPIKey()
                    apiKey = ""
                    status = "Key deleted."
                }
                Button("Save") {
                    let trimmed = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
                    if trimmed.isEmpty {
                        KeychainStore.deleteAPIKey()
                    } else {
                        KeychainStore.saveAPIKey(trimmed)
                    }
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(24)
    }

    private func mint() async {
        isMinting = true
        defer { isMinting = false }
        do {
            let token = try await KhalaClient.mintFreeKey()
            apiKey = token
            KeychainStore.saveAPIKey(token)
            status = "Free key saved."
        } catch {
            status = "Could not mint a free key: \(error.localizedDescription)"
        }
    }
}
