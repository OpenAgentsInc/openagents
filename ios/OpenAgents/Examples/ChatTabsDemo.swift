import SwiftUI
#if os(iOS)

@available(iOS 26, *)
struct ChatTabsDemo: View {
    @State private var selection: Int = 0

    var body: some View {
        TabView(selection: $selection) {
            ChatTab()
                .tabItem { Label("Chat", systemImage: "bubble.left.and.bubble.right") }
                .tag(0)

            HistoryTab()
                .tabItem { Label("History", systemImage: "clock") }
                .tag(1)

            SettingsTab()
                .tabItem { Label("Settings", systemImage: "gear") }
                .tag(2)
        }
        .tabViewBottomAccessory { ChatBottomAccessory() }
    }
}

@available(iOS 26, *)
private struct ChatBottomAccessory: View {
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement

    var body: some View {
        Group {
            if placement == .inline {
                // Expanded accessory above the tab bar
                HStack(spacing: 12) {
                    Image(systemName: "mic")
                    Text("Listening for a quick reply…")
                        .lineLimit(1)
                    Spacer(minLength: 0)
                    Button(action: {}) { Image(systemName: "paperplane.fill") }
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .fill(.ultraThinMaterial)
                )
            } else {
                // Collapsed into the tab bar area – keep it compact
                HStack(spacing: 16) {
                    Image(systemName: "mic")
                    Image(systemName: "paperplane.fill")
                }
                .padding(.vertical, 2)
            }
        }
        .foregroundStyle(.primary)
    }
}

@available(iOS 26, *)
private struct ChatTab: View {
    var body: some View {
        List {
            ForEach(0..<20, id: \.self) { i in
                Text(i % 2 == 0 ? "User: How do I fix the crash in FileCache?" : "Assistant: Try clearing the stale index and re-run tests.")
            }
        }
        .listStyle(.plain)
    }
}

@available(iOS 26, *)
private struct HistoryTab: View {
    var body: some View {
        List {
            ForEach(0..<10, id: \.self) { i in
                Label("Thread #\(i + 1)", systemImage: "text.bubble")
            }
        }
        .listStyle(.plain)
    }
}

@available(iOS 26, *)
private struct SettingsTab: View {
    var body: some View {
        Form {
            Toggle(isOn: .constant(true)) { Text("Notifications") }
            Toggle(isOn: .constant(true)) { Text("Voice input") }
        }
    }
}

#if DEBUG
@available(iOS 26, *)
#Preview {
    ChatTabsDemo()
}
#endif
 #endif // os(iOS)
