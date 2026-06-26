import SwiftUI

/// App shell: a single-conversation chat `NavigationStack` inside the left
/// slide-over `DrawerContainer`. Deliberately minimal — the top bar is the
/// hamburger (sidebar), a small "Khala" menu (open-trace-in-web + settings), and
/// a new-chat button. No model picker, no voice.
struct RootView: View {
    @ObservedObject var store: ConversationStore
    @Environment(\.openURL) private var openURL

    @State private var drawerOpen = false
    @State private var showSettings = false
    @State private var hasKey = KeychainStore.hasAPIKey
    @State private var didLaunch = false
    @State private var selection: Conversation?
    /// Streaming view model for the active conversation, recreated when it changes.
    @State private var model: ChatViewModel?

    var body: some View {
        DrawerContainer(isOpen: $drawerOpen) {
            chatStack
        } drawer: {
            DrawerContentView(
                store: store,
                selection: $selection,
                onNewChat: newChat,
                onOpenSettings: { drawerOpen = false; showSettings = true },
                onSelect: open(_:)
            )
        }
        .sheet(isPresented: $showSettings) {
            SettingsView(hasKey: $hasKey)
        }
        .task { await onAppear() }
        .onChange(of: selection?.id) { _, _ in syncModel() }
    }

    private var chatStack: some View {
        NavigationStack {
            Group {
                if let conversation = activeConversation, let model = modelFor(conversation) {
                    ChatView(
                        store: store,
                        model: model,
                        conversation: conversation,
                        hasKey: $hasKey,
                        onOpenSettings: { showSettings = true }
                    )
                    .id(conversation.id)
                } else {
                    ContentUnavailableView {
                        Label("No conversation", systemImage: "bubble.left.and.bubble.right")
                    } actions: {
                        Button("New Chat", action: newChat)
                    }
                }
            }
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button {
                        withAnimation { drawerOpen.toggle() }
                    } label: {
                        Image(systemName: "line.3.horizontal")
                    }
                    .accessibilityLabel("Menu")
                }
                ToolbarItem(placement: .principal) {
                    Menu {
                        Button {
                            openTracesInWeb(conversation: activeConversation)
                        } label: {
                            Label("Open traces in web", systemImage: "safari")
                        }
                        Button {
                            showSettings = true
                        } label: {
                            Label("Settings", systemImage: "gearshape")
                        }
                    } label: {
                        HStack(spacing: 4) {
                            Text("Khala")
                                .font(.headline)
                                .foregroundStyle(.primary)
                            Image(systemName: "chevron.down")
                                .font(.caption2.weight(.bold))
                                .foregroundStyle(.secondary)
                        }
                    }
                    .accessibilityLabel("Khala menu")
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button(action: newChat) {
                        Image(systemName: "square.and.pencil")
                    }
                    .accessibilityLabel("New chat")
                }
            }
        }
    }

    // MARK: - Traces

    /// Open the owner's traces on the web, passing the API key as a token so the
    /// owner can view their own (unshared, owner-only) traces even when not
    /// logged into the web. The backend authorizes the token for owner-scoped
    /// trace viewing; if a per-conversation trace ref becomes available it can be
    /// appended here to deep-link the specific trace.
    private func openTracesInWeb(conversation: Conversation?) {
        var components = URLComponents(string: "https://openagents.com/traces")
        if let key = KeychainStore.loadAPIKey() {
            components?.queryItems = [URLQueryItem(name: "token", value: key)]
        }
        if let url = components?.url {
            openURL(url)
        }
    }

    // MARK: - State

    private var activeConversation: Conversation? {
        selection ?? store.mostRecent
    }

    private func modelFor(_ conversation: Conversation) -> ChatViewModel? {
        if let model, model.conversationID == conversation.id { return model }
        return nil
    }

    private func syncModel() {
        guard let conversation = activeConversation else { model = nil; return }
        if model?.conversationID == conversation.id { return }
        model?.stop()
        model = ChatViewModel(store: store, conversation: conversation)
    }

    private func open(_ conversation: Conversation) {
        selection = conversation
        withAnimation { drawerOpen = false }
    }

    private func newChat() {
        let convo = store.createConversation()
        selection = convo // triggers syncModel via onChange
        withAnimation { drawerOpen = false }
    }

    private func onAppear() async {
        // Always have a conversation to render (non-black launch gate).
        if store.conversations.isEmpty {
            selection = store.createConversation()
        } else if selection == nil {
            selection = store.mostRecent
        }
        syncModel()

        // Free dogfood app: guarantee an API key so the composer works out of the
        // box. Auto-mint a free key on first launch when none is stored.
        if KeychainStore.hasAPIKey {
            hasKey = true
        } else if let token = try? await KhalaClient.mintFreeKey() {
            KeychainStore.saveAPIKey(token)
            hasKey = true
        }

        guard !didLaunch else { return }
        didLaunch = true
        // Demo/test hook (env-gated; no-op in normal use): auto-send a prompt on
        // launch so the streaming round-trip is verifiable on a simulator.
        if let demo = ProcessInfo.processInfo.environment["KHALA_DEMO_PROMPT"],
           !demo.isEmpty, hasKey, let conversation = activeConversation,
           let model = modelFor(conversation) {
            model.send(demo)
        }
    }
}
