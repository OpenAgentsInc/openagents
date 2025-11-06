import SwiftUI

#if os(iOS)
/// Fresh screen showcasing the new top toolbar header for iOS 26+.
/// This screen intentionally does not auto-load a conversation thread.
struct ChatHomeView: View {
    @State private var isMenuPresented = false

    var body: some View {
        NavigationStack {
            // Main content placeholder
            VStack(spacing: 16) {
                Image(systemName: "bubble.left.and.bubble.right")
                    .font(.system(size: 44, weight: .regular))
                    .foregroundStyle(.secondary)
                Text("Welcome to OpenAgents")
                    .font(.title2)
                Text("Use the toolbar to start a new chat or open the menu.")
                    .font(.body)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
            .navigationTitle("")
            .toolbarTitleDisplayMode(.inline)
            .toolbar {
                ChatHeaderToolbar(
                    title: "Conversations",
                    onToggleMenu: { isMenuPresented.toggle() },
                    onNewChat: { /* hook up compose/present flow here */ }
                )
            }
            // iOS 26+ only: let the system render the Liquid Glass toolbar background
            .sheet(isPresented: $isMenuPresented) {
                MenuSheet()
            }
        }
    }
}

private struct MenuSheet: View {
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section("Navigation") {
                    Label("Home", systemImage: "house")
                    Label("Recent", systemImage: "clock")
                    Label("Settings", systemImage: "gear")
                }
            }
            .navigationTitle("Menu")
            .toolbarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done", action: { dismiss() })
                        .buttonStyle(.glass)
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}

#Preview {
    ChatHomeView()
}

#endif
