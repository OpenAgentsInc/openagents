import SwiftUI

/// Contents of the left slide-over drawer — the ChatGPT-style history panel
/// (issue #6344).
///
/// Top to bottom: the "Khala" title, a search field that filters Recents, a
/// short menu (New chat), the **Recents** conversation list grouped by recency
/// (tap to open, long-press to Rename / Delete, swipe to delete), and a bottom
/// bar with **New Chat** + a settings gear.
///
/// History is read live from `ConversationStore` (sorted `updatedAt` desc), so
/// new/rename/delete and new turns reorder Recents immediately. The shell
/// (`DrawerContainer`) owns slide/scrim/gesture mechanics; this view owns only
/// the panel contents.
struct DrawerContentView: View {
    @ObservedObject var store: ConversationStore
    @Binding var selection: Conversation?
    let onNewChat: () -> Void
    let onOpenSettings: () -> Void
    let onSelect: (Conversation) -> Void

    @State private var search = ""

    /// Rename flow state: the conversation being renamed + the draft title.
    @State private var renaming: Conversation?
    @State private var renameText = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            searchField
            menu
            Divider()
                .padding(.horizontal, DrawerStyle.edge)
                .padding(.top, 4)
            recents
            Divider()
            bottomBar
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .alert("Rename chat", isPresented: renameBinding) {
            TextField("Title", text: $renameText)
            Button("Cancel", role: .cancel) { renaming = nil }
            Button("Save") {
                if let convo = renaming {
                    store.rename(convo, to: renameText)
                }
                renaming = nil
            }
        }
    }

    // MARK: - Sections

    private var header: some View {
        Text("Khala")
            .font(.title2.weight(.semibold))
            .padding(.horizontal, DrawerStyle.edge)
            .padding(.top, 14)
            .padding(.bottom, 12)
    }

    private var searchField: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.callout)
                .foregroundStyle(.secondary)
            TextField("Search", text: $search)
                .textFieldStyle(.plain)
                .submitLabel(.search)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
            if !search.isEmpty {
                Button {
                    search = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.tertiary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Clear search")
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: DrawerStyle.fieldRadius))
        .padding(.horizontal, DrawerStyle.edge)
        .padding(.bottom, 4)
    }

    /// Short menu above Recents. Single primary action for now (New chat);
    /// shaped as a seam so future entries drop in on the same grid. Hidden
    /// while a search is active so it doesn't sit above filtered results.
    @ViewBuilder private var menu: some View {
        if trimmedQuery.isEmpty {
            Button(action: onNewChat) {
                MenuRow(icon: "square.and.pencil", title: "New chat")
            }
            .buttonStyle(.plain)
            .padding(.horizontal, DrawerStyle.edge)
            .padding(.top, 4)
        }
    }

    private var recents: some View {
        List {
            if groups.isEmpty {
                Section {
                    Text(store.conversations.isEmpty
                         ? "No chats yet. Start one with New Chat."
                         : "No chats match \u{201C}\(trimmedQuery)\u{201D}.")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets(top: 8, leading: DrawerStyle.edge, bottom: 8, trailing: DrawerStyle.edge))
                } header: {
                    sectionHeader("Recents")
                }
            } else {
                ForEach(groups, id: \.title) { group in
                    Section {
                        ForEach(group.conversations) { convo in
                            RecentRowView(
                                conversation: convo,
                                isSelected: convo.id == selection?.id,
                                onSelect: { onSelect(convo) },
                                onRename: { beginRename(convo) },
                                onDelete: { delete(convo) }
                            )
                        }
                    } header: {
                        sectionHeader(group.title)
                    }
                }
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .scrollDismissesKeyboard(.interactively)
    }

    private var bottomBar: some View {
        HStack(spacing: 12) {
            Button(action: onNewChat) {
                Label("New Chat", systemImage: "square.and.pencil")
                    .font(.callout.weight(.semibold))
            }
            .buttonStyle(.plain)
            Spacer()
            Button(action: onOpenSettings) {
                Image(systemName: "gearshape")
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Settings")
        }
        .padding(.horizontal, DrawerStyle.edge)
        .padding(.vertical, 14)
    }

    private func sectionHeader(_ title: String) -> some View {
        Text(title)
            .font(.footnote.weight(.semibold))
            .foregroundStyle(.secondary)
            .textCase(nil)
            .listRowInsets(EdgeInsets(top: 12, leading: DrawerStyle.edge, bottom: 4, trailing: DrawerStyle.edge))
    }

    // MARK: - Derived data

    private var trimmedQuery: String {
        search.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var filtered: [Conversation] {
        let q = trimmedQuery.lowercased()
        guard !q.isEmpty else { return store.conversations }
        return store.conversations.filter { $0.title.lowercased().contains(q) }
    }

    /// Recents grouped into ChatGPT-style recency buckets, preserving the
    /// store's `updatedAt`-desc order within each bucket. Empty buckets drop out.
    private var groups: [RecentGroup] {
        let calendar = Calendar.current
        let now = Date()
        var today: [Conversation] = []
        var week: [Conversation] = []
        var older: [Conversation] = []

        for convo in filtered {
            if calendar.isDateInToday(convo.updatedAt) {
                today.append(convo)
            } else if let days = calendar.dateComponents([.day], from: convo.updatedAt, to: now).day, days < 7 {
                week.append(convo)
            } else {
                older.append(convo)
            }
        }

        var result: [RecentGroup] = []
        if !today.isEmpty { result.append(.init(title: "Today", conversations: today)) }
        if !week.isEmpty { result.append(.init(title: "Previous 7 Days", conversations: week)) }
        if !older.isEmpty { result.append(.init(title: "Older", conversations: older)) }
        return result
    }

    private var renameBinding: Binding<Bool> {
        Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })
    }

    // MARK: - Actions

    private func beginRename(_ convo: Conversation) {
        renameText = convo.title
        renaming = convo
    }

    private func delete(_ convo: Conversation) {
        let wasSelected = convo.id == selection?.id
        store.delete(convo)
        if wasSelected {
            selection = store.mostRecent
        }
    }
}

/// A recency bucket of conversations shown as one Recents section.
private struct RecentGroup {
    let title: String
    let conversations: [Conversation]
}

/// A drawer menu row (icon column + label) above Recents, aligned to the same
/// grid as the conversation rows.
private struct MenuRow: View {
    let icon: String
    let title: String

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .font(.body)
                .frame(width: DrawerStyle.iconColumn, alignment: .center)
                .foregroundStyle(.primary)
            Text(title)
                .font(.body)
                .foregroundStyle(.primary)
            Spacer(minLength: 0)
        }
        .frame(minHeight: DrawerStyle.rowHeight)
        .contentShape(Rectangle())
    }
}
