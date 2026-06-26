import SwiftUI

/// The single Khala screen: animated background, push-to-talk button, and the
/// transcript/response text. A gear opens Settings (key + voice options).
struct ContentView: View {
    @StateObject private var voice = VoiceController()
    @State private var showSettings = false
    @State private var hasKey = KeychainStore.hasAPIKey
    @State private var permissionsRequested = false
    @State private var typedMessage = ""
    @FocusState private var composerFocused: Bool

    var body: some View {
        ZStack {
            AnimatedBackground(level: voice.level, accent: voice.state.accentColor)

            VStack(spacing: 24) {
                header

                Spacer()

                if !voice.transcript.isEmpty {
                    Text("“\(voice.transcript)”")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal)
                }

                if !voice.response.isEmpty {
                    ScrollView {
                        Text(voice.response)
                            .font(.body)
                            .foregroundStyle(.primary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding()
                    }
                    .frame(maxHeight: 220)
                }

                Spacer()

                Text(voice.state.label)
                    .font(.subheadline)
                    .foregroundStyle(voice.state.accentColor)

                PushToTalkButton(
                    state: voice.state,
                    level: voice.level,
                    onPressDown: { voice.pressDown() },
                    onPressUp: { voice.pressUp() }
                )
                .padding(.bottom, 12)

                composer

                if !hasKey {
                    Button("Add a Khala key to get started") { showSettings = true }
                        .font(.footnote)
                        .foregroundStyle(.orange)
                }
            }
            .padding()
        }
        .sheet(isPresented: $showSettings) {
            SettingsView(hasKey: $hasKey)
        }
        .task {
            guard !permissionsRequested else { return }
            permissionsRequested = true
            _ = await voice.requestPermissions()
            // Demo/test hook (env-gated; no-op in normal use): auto-send a prompt
            // on launch so the end-to-end Khala API round-trip is verifiable on a
            // simulator without driving the UI. Pair with KHALA_API_KEY.
            if let demo = ProcessInfo.processInfo.environment["KHALA_DEMO_PROMPT"],
               !demo.isEmpty, hasKey {
                voice.sendText(demo)
            }
        }
    }

    /// Voice-free composer: type a message and send it through the same Khala
    /// round-trip the push-to-talk path uses. This is the minimal way to test
    /// the end-to-end handshake without microphone/speech permissions.
    private var composer: some View {
        HStack(spacing: 8) {
            TextField("Type a message…", text: $typedMessage, axis: .vertical)
                .lineLimit(1...4)
                .textFieldStyle(.plain)
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 12))
                .focused($composerFocused)
                .submitLabel(.send)
                .onSubmit(sendTyped)
                .disabled(voice.state.isBusy)

            Button(action: sendTyped) {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.title)
                    .foregroundStyle(canSend ? voice.state.accentColor : Color.secondary)
            }
            .disabled(!canSend)
            .accessibilityLabel("Send message")
        }
        .padding(.horizontal, 4)
    }

    private var canSend: Bool {
        hasKey
            && !voice.state.isBusy
            && !typedMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func sendTyped() {
        let text = typedMessage
        guard canSend else { return }
        composerFocused = false
        typedMessage = ""
        voice.sendText(text)
    }

    private var header: some View {
        HStack {
            Text("Khala")
                .font(.title2.weight(.semibold))
                .foregroundStyle(.primary)
            Spacer()
            Button { showSettings = true } label: {
                Image(systemName: "gearshape")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }
        }
    }
}

#Preview {
    ContentView().preferredColorScheme(.dark)
}
