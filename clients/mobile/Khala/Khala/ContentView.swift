import SwiftUI

/// The single Khala screen: animated background, push-to-talk button, and the
/// transcript/response text. A gear opens Settings (key + voice options).
struct ContentView: View {
    @StateObject private var voice = VoiceController()
    @State private var showSettings = false
    @State private var hasKey = KeychainStore.hasAPIKey
    @State private var permissionsRequested = false

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

                if !hasKey {
                    Button("Add a Khala key to get started") { showSettings = true }
                        .font(.footnote)
                        .foregroundStyle(.orange)
                }
            }
            .padding()
        }
        .sheet(isPresented: $showSettings) {
            SettingsView(hasKey: $hasKey, speakResponses: $voice.speakResponses)
        }
        .task {
            guard !permissionsRequested else { return }
            permissionsRequested = true
            _ = await voice.requestPermissions()
        }
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
