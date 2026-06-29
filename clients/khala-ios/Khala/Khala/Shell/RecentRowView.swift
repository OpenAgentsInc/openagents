import SwiftUI

/// One conversation row in the drawer's Recents list (#6344).
///
/// ChatGPT-style: a single-line title that fills the row, a subtle highlight
/// for the active chat, a long-press context menu (Rename / Delete) and a
/// trailing swipe-to-delete. Tapping the row opens the conversation.
struct RecentRowView: View {
    let conversation: Conversation
    let isSelected: Bool
    let onSelect: () -> Void
    let onRename: () -> Void
    let onDelete: () -> Void

    var body: some View {
        Button(action: onSelect) {
            Text(conversation.title)
                .font(.body)
                .lineLimit(1)
                .truncationMode(.tail)
                .foregroundStyle(.primary)
                .frame(maxWidth: .infinity, minHeight: DrawerStyle.rowHeight, alignment: .leading)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .listRowInsets(EdgeInsets(top: 0, leading: DrawerStyle.edge, bottom: 0, trailing: DrawerStyle.edge))
        .listRowBackground(
            DrawerStyle.rowFill(selected: isSelected)
                .clipShape(RoundedRectangle(cornerRadius: DrawerStyle.fieldRadius))
                .padding(.horizontal, 8)
                .padding(.vertical, 2)
        )
        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
            Button(role: .destructive, action: onDelete) {
                Label("Delete", systemImage: "trash")
            }
        }
        .contextMenu {
            Button(action: onRename) {
                Label("Rename", systemImage: "pencil")
            }
            Button(role: .destructive, action: onDelete) {
                Label("Delete", systemImage: "trash")
            }
        }
        .accessibilityLabel(conversation.title)
        .accessibilityHint(isSelected ? "Current chat" : "Opens this chat")
    }
}
