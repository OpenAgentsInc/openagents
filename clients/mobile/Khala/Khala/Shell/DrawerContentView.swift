import SwiftUI

/// Contents of the left slide-over drawer — the ChatGPT-style history panel.
///
/// FOUNDATION SEAM (issue #6344 fills this out): "Khala" title, a search field
/// that filters Recents, the Recents conversation list (tap to open, swipe to
/// delete, rename), and a bottom row with New Chat + a settings gear.
///
/// This foundation version is intentionally minimal but FUNCTIONAL: it lists
/// Recents from the store, opens a conversation on tap, supports New Chat,
/// swipe-to-delete, and opens Settings — so the shell is usable today. The
/// drawer/history lane can refine the search, rename UX, sectioning, and visuals.
struct DrawerContentView: View {
    @ObservedObject var store: ConversationStore
    @Binding var selection: Conversation?
    let onNewChat: () -> Void
    let onOpenSettings: () -> Void
    let onSelect: (Conversation) -> Void

    @State private var search = ""

    private var filtered: [Conversation] {
        let q = search.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !q.isEmpty else { return store.conversations }
        return store.conversations.filter { $0.title.lowercased().contains(q) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Khala")
                .font(.title2.weight(.semibold))
                .padding(.horizontal, 16)
                .padding(.top, 14)
                .padding(.bottom, 10)

            // Search (filters Recents). #6344 owns richer search behavior.
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)
                TextField("Search", text: $search)
                    .textFieldStyle(.plain)
                    .autocorrectionDisabled()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 9)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 10))
            .padding(.horizontal, 12)
            .padding(.bottom, 6)

            // Recents.
            List {
                Section("Recents") {
                    if filtered.isEmpty {
                        Text(store.conversations.isEmpty ? "No chats yet." : "No matches.")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(filtered) { convo in
                            Button {
                                onSelect(convo)
                            } label: {
                                HStack {
                                    Text(convo.title)
                                        .lineLimit(1)
                                        .foregroundStyle(.primary)
                                    Spacer()
                                }
                                .contentShape(Rectangle())
                            }
                            .listRowBackground(
                                Group {
                                    if convo.id == selection?.id {
                                        Color.primary.opacity(0.08)
                                    } else {
                                        Color.clear
                                    }
                                }
                            )
                            .swipeActions(edge: .trailing) {
                                Button(role: .destructive) {
                                    delete(convo)
                                } label: {
                                    Label("Delete", systemImage: "trash")
                                }
                            }
                        }
                    }
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)

            Divider()

            // Bottom row: New Chat + settings gear.
            HStack(spacing: 12) {
                Button(action: onNewChat) {
                    Label("New Chat", systemImage: "square.and.pencil")
                        .font(.callout.weight(.semibold))
                }
                Spacer()
                Button(action: onOpenSettings) {
                    Image(systemName: "gearshape")
                        .font(.title3)
                        .foregroundStyle(.secondary)
                }
                .accessibilityLabel("Settings")
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 14)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func delete(_ convo: Conversation) {
        let wasSelected = convo.id == selection?.id
        store.delete(convo)
        if wasSelected {
            selection = store.mostRecent
        }
    }
}
