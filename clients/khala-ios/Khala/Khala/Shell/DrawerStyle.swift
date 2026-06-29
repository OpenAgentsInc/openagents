import SwiftUI

/// Shared visual tokens for the left slide-over drawer (#6344).
///
/// Centralizing spacing, row metrics, and tints keeps the drawer's title,
/// search field, menu rows, Recents list, and bottom bar on one rhythm so the
/// panel reads as a single ChatGPT-style surface rather than stacked controls.
enum DrawerStyle {
    /// Horizontal inset used by the title, search field, menu rows, and bottom
    /// bar so every element shares one left edge.
    static let edge: CGFloat = 16

    /// Standard height for a tappable menu / Recents row. Comfortable touch
    /// target without feeling like a chunky settings cell.
    static let rowHeight: CGFloat = 44

    /// Corner radius for the search field and row selection highlight.
    static let fieldRadius: CGFloat = 11

    /// Width of the leading icon column so menu-row and Recents-row labels
    /// align on one grid.
    static let iconColumn: CGFloat = 26

    /// Selection / press tint for a row. Subtle: it marks the active chat
    /// without competing with the title text.
    static func rowFill(selected: Bool) -> Color {
        selected ? Color.primary.opacity(0.09) : .clear
    }
}
