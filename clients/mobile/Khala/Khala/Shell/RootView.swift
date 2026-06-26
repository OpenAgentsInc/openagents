import SwiftUI

/// App shell: the main chat `NavigationStack` hosted inside the left slide-over
/// `DrawerContainer`. The top bar carries the hamburger (toggles the drawer),
/// the "Khala" model pill (single model — no fake variants), and a new-chat
/// icon. Settings is reachable from the drawer and the pill sheet.
///
/// This is the foundation shell: it compiles and shows a usable chat surface
/// today. The drawer CONTENTS (#6344) and chat-view internals (#6345) are clear
/// seams the feature lanes fill in.
struct RootView: View {
    @ObservedObject var store: ConversationStore
    @StateObject private var voice = VoiceController()

    @State private var drawerOpen = false
    @State private var showSettings = false
    @State private var showAbout = false
    @State private var hasKey = KeychainStore.hasAPIKey
    @State private var permissionsRequested = false
    @State private var selection: Conversation?

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
        .sheet(isPresented: $showAbout) {
            aboutSheet
        }
        .task { await onAppear() }
    }

    // MARK: - Chat stack + top bar

    private var chatStack: some View {
        NavigationStack {
            Group {
                if let conversation = activeConversation {
                    ChatView(
                        store: store,
                        voice: voice,
                        conversation: conversation,
                        hasKey: $hasKey,
                        onOpenSettings: { showSettings = true }
                    )
                    .id(conversation.id)
                    // Empty-state greeting + coding suggestions for a fresh
                    // chat. Overlaid above the chat surface so the composer
                    // stays usable; tapping a suggestion sends the first user
                    // turn and the live transcript replaces this overlay.
                    .overlay {
                        if isEmpty(conversation) {
                            EmptyStateView(
                                canSend: hasKey && !voice.state.isBusy,
                                onSelect: { sendSuggestion($0) }
                            )
                            .transition(.opacity)
                        }
                    }
                    .animation(.easeOut(duration: 0.2), value: isEmpty(conversation))
                } else {
                    // Should not normally happen (we ensure one exists), but never
                    // black-screen: offer a way forward.
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
                    Button { showAbout = true } label: {
                        HStack(spacing: 5) {
                            Text("Khala")
                                .font(.subheadline.weight(.semibold))
                                .foregroundStyle(.primary)
                            Image(systemName: "chevron.down")
                                .font(.caption2.weight(.bold))
                                .foregroundStyle(.secondary)
                        }
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(.ultraThinMaterial, in: Capsule())
                        .overlay(Capsule().stroke(.white.opacity(0.08), lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Khala model. About")
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

    private var aboutSheet: some View {
        NavigationStack {
            List {
                Section {
                    LabeledContent("Model", value: "openagents/khala")
                    LabeledContent("Backend", value: "openagents.com/api/v1")
                } footer: {
                    Text("Khala is a single model. There are no mini/pro/code variants.")
                }
            }
            .navigationTitle("About Khala")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { showAbout = false }
                }
            }
        }
        .presentationDetents([.medium])
    }

    // MARK: - State

    private var activeConversation: Conversation? {
        selection ?? store.mostRecent
    }

    /// A conversation is "empty" (shows the empty state) when it has no
    /// user/assistant turns yet and no in-flight request is being rendered.
    private func isEmpty(_ conversation: Conversation) -> Bool {
        let hasTurns = conversation.messages.contains { $0.role != .system }
        let inFlight = voice.state.isBusy || !voice.response.isEmpty
            || !voice.transcript.isEmpty || voice.requestError != nil
        return !hasTurns && !inFlight
    }

    /// Start a conversation from a tapped empty-state suggestion. Sends the
    /// canned prompt as the first user turn through the same Khala round-trip
    /// the composer uses; the empty state then yields to the live transcript.
    private func sendSuggestion(_ prompt: String) {
        guard hasKey, !voice.state.isBusy, let conversation = activeConversation else { return }
        let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        store.appendMessage(.user, content: trimmed, to: conversation)
        voice.sendText(trimmed)
    }

    private func open(_ conversation: Conversation) {
        selection = conversation
        withAnimation { drawerOpen = false }
    }

    private func newChat() {
        let convo = store.createConversation()
        selection = convo
        withAnimation { drawerOpen = false }
    }

    private func onAppear() async {
        // Ensure there is always at least one conversation to render (non-black
        // launch gate): create one if the store is empty, otherwise select the
        // most recent.
        if store.conversations.isEmpty {
            selection = store.createConversation()
        } else if selection == nil {
            selection = store.mostRecent
        }

        guard !permissionsRequested else { return }
        permissionsRequested = true
        // Demo/test hook (env-gated; no-op in normal use): skip the mic/speech
        // permission prompt so launch-render screenshots and CI smoke runs show
        // the chat surface without the system dialog. Real users still get the
        // prompt on first push-to-talk use.
        if ProcessInfo.processInfo.environment["KHALA_SKIP_PERMISSIONS"] == nil {
            _ = await voice.requestPermissions()
        }

        // Demo/test hook (env-gated; no-op in normal use): auto-send a prompt on
        // launch so the end-to-end Khala API round-trip is verifiable on a
        // simulator without driving the UI. Pair with KHALA_API_KEY.
        if let demo = ProcessInfo.processInfo.environment["KHALA_DEMO_PROMPT"],
           !demo.isEmpty, hasKey, let conversation = activeConversation {
            store.appendMessage(.user, content: demo, to: conversation)
            voice.sendText(demo)
        }
    }
}
