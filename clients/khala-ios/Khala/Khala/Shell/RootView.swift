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
    @State private var showDiagnostics = false
    @State private var hasKey = Self.hasUsableAPIKey()
    @State private var didLaunch = false
    @State private var selection: Conversation?
    /// Active conversation channel. `.khala` is the public collective model;
    /// `.artanis` is the owner-only operator channel (#6363). Toggled from the
    /// title menu; switching starts a fresh conversation so the two personas
    /// never share transcript context.
    @State private var channel: ChatViewModel.Channel = .khala
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
        .sheet(isPresented: $showDiagnostics) {
            DiagnosticsView(snapshot: diagnosticsSnapshot)
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
                        if channel == .khala {
                            Button {
                                switchChannel(to: .artanis)
                            } label: {
                                Label("Talk to Artanis", systemImage: "person.crop.circle.badge.checkmark")
                            }
                        } else {
                            Button {
                                switchChannel(to: .khala)
                            } label: {
                                Label("Back to Khala", systemImage: "bubble.left.and.bubble.right")
                            }
                        }
                        Button {
                            switchChannel(to: .appleFM)
                        } label: {
                            Label("Use Apple FM", systemImage: "desktopcomputer")
                        }
                        Button {
                            openTracesInWeb(conversation: activeConversation)
                        } label: {
                            Label("Open traces in web", systemImage: "safari")
                        }
                        Button {
                            showDiagnostics = true
                        } label: {
                            Label("Diagnostics", systemImage: "lock.shield")
                        }
                        Button {
                            showSettings = true
                        } label: {
                            Label("Settings", systemImage: "gearshape")
                        }
                    } label: {
                        HStack(spacing: 4) {
                            Text(channel.speaker)
                                .font(.headline)
                                .foregroundStyle(.primary)
                            Image(systemName: "chevron.down")
                                .font(.caption2.weight(.bold))
                                .foregroundStyle(.secondary)
                        }
                    }
                    .accessibilityLabel("\(channel.speaker) menu")
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

    /// Open the owner's traces on the web without putting the stored key into a
    /// URL. Captured API keys must never be re-displayed in plaintext after
    /// Settings stores them.
    private func openTracesInWeb(conversation _: Conversation?) {
        if let url = URL(string: "https://openagents.com/traces") {
            openURL(url)
        }
    }

    // MARK: - State

    private var activeConversation: Conversation? {
        selection ?? store.mostRecent
    }

    private var diagnosticsSnapshot: LocalDiagnosticsSnapshot {
        LocalDiagnosticsSnapshot.make(
            hasAPIKey: hasKey,
            channelName: channel.speaker,
            isStreaming: model?.isStreaming ?? false,
            activeConversation: activeConversation,
            conversationCount: store.conversations.count,
            isUsingEphemeralFallback: store.isUsingEphemeralFallback
        )
    }

    private func modelFor(_ conversation: Conversation) -> ChatViewModel? {
        if let model, model.conversationID == conversation.id, model.channel == channel { return model }
        return nil
    }

    private func syncModel() {
        guard let conversation = activeConversation else { model = nil; return }
        if model?.conversationID == conversation.id, model?.channel == channel { return }
        model?.stop()
        model = ChatViewModel(store: store, conversation: conversation, channel: channel)
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

    /// Switch the active channel. Starts a fresh conversation so the owner-only
    /// Artanis operator persona and the public Khala model never share
    /// transcript context, then rebuilds the streaming model for the new channel.
    private func switchChannel(to next: ChatViewModel.Channel) {
        guard next != channel else { return }
        model?.stop()
        channel = next
        let convo = store.createConversation()
        selection = convo
        syncModel()
        withAnimation { drawerOpen = false }
    }

    private func onAppear() async {
        // Demo/test hook (env-gated; a no-op in normal use): open straight into a
        // given channel so the Artanis operator surface is screenshot-verifiable
        // on a simulator without driving the title menu.
        if ProcessInfo.processInfo.environment["KHALA_DEMO_CHANNEL"] == "artanis" {
            channel = .artanis
        }

        // Always have a conversation to render (non-black launch gate).
        if store.conversations.isEmpty {
            selection = store.createConversation()
        } else if selection == nil {
            selection = store.mostRecent
        }
        syncModel()

        hasKey = Self.hasUsableAPIKey()

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

    private static func hasUsableAPIKey() -> Bool {
        guard let key = KeychainStore.loadAPIKey() else { return false }
        return FreeTierDisclosureStore.canUse(apiKey: key)
    }
}
