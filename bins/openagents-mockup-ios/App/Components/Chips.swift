import SwiftUI

/// A rounded chip: suggestion chips, follow-ups, screen chips, offers.
struct Chip: View {
    var icon: String? = nil
    let text: String
    var filled = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                if let icon { Image(systemName: icon).font(.system(size: 14, weight: .semibold)) }
                Text(text).font(.system(size: 15, weight: .medium)).lineLimit(1)
            }
            .foregroundStyle(filled ? Theme.Colors.primaryLabel : Theme.Colors.textPrimary)
            .padding(.horizontal, 14)
            .frame(minHeight: Theme.Size.chipHeight)
            .background(Capsule().fill(filled ? Theme.Colors.primaryFill : Theme.Colors.surfaceRaised))
            .overlay(Capsule().stroke(filled ? .clear : Theme.Colors.stroke, lineWidth: 1))
        }
        .buttonStyle(PressStyle())
    }
}

/// Wraps chips onto as many lines as they need.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let maxWidth = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, rowHeight: CGFloat = 0, widest: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > 0 && x + size.width > maxWidth {
                y += rowHeight + spacing
                x = 0
                rowHeight = 0
            }
            x += size.width + spacing
            widest = max(widest, x - spacing)
            rowHeight = max(rowHeight, size.height)
        }
        return CGSize(width: min(widest, maxWidth), height: y + rowHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, rowHeight: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > bounds.minX && x + size.width > bounds.maxX {
                y += rowHeight + spacing
                x = bounds.minX
                rowHeight = 0
            }
            view.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
    }
}

#Preview("Chips") {
    FlowLayout {
        Chip(icon: "questionmark.circle", text: "Who are you?") {}
        Chip(icon: "questionmark.circle", text: "What can you do?") {}
        Chip(icon: "clock", text: "Fix login…") {}
        Chip(icon: "plus", text: "Connect a computer") {}
        Chip(icon: "desktopcomputer", text: "Run Coder on Studio Mac", filled: true) {}
    }
    .padding()
    .frame(maxHeight: .infinity)
    .background(Theme.Colors.background)
}
