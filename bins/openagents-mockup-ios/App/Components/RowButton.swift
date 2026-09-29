import SwiftUI

/// The big stacked menu row: icon, condensed uppercase title, one-line
/// subtitle, chevron. `.primary` is white-filled (one per screen);
/// `.outlined` is everything else.
struct RowButton: View {
    enum Style { case primary, outlined }

    let icon: String
    let title: String
    var subtitle: String? = nil
    var style: Style = .outlined
    let action: () -> Void

    private var fg: Color { style == .primary ? Theme.Colors.primaryLabel : Theme.Colors.textPrimary }
    private var sub: Color { style == .primary ? Theme.Colors.primaryLabel.opacity(0.62) : Theme.Colors.textSecondary }

    var body: some View {
        Button(action: action) {
            HStack(spacing: Theme.Space.s) {
                Image(systemName: icon)
                    .font(.system(size: Theme.Size.rowIcon * 0.8, weight: .bold))
                    .frame(width: Theme.Size.rowIconBox, height: Theme.Size.rowIconBox)
                    .foregroundStyle(fg)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).condensedTitle()
                        .foregroundStyle(fg)
                        .lineLimit(1)
                        .minimumScaleFactor(0.8)
                    if let subtitle {
                        Text(subtitle)
                            .font(Theme.Fonts.rowSubtitle)
                            .foregroundStyle(sub)
                            .lineLimit(2)
                            .multilineTextAlignment(.leading)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Image(systemName: "chevron.right")
                    .font(.system(size: 17, weight: .bold))
                    .foregroundStyle(sub)
            }
            .padding(.horizontal, Theme.Space.s)
            .padding(.vertical, Theme.Space.s)
            .frame(maxWidth: .infinity, minHeight: Theme.Size.rowMinHeight, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: Theme.Radius.row)
                    .fill(style == .primary ? Theme.Colors.primaryFill : Theme.Colors.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: Theme.Radius.row)
                    .stroke(style == .primary ? .clear : Theme.Colors.stroke, lineWidth: Theme.Stroke.hairline)
            )
            .themeShadow(style == .primary ? Theme.Shadows.primary : Theme.Shadow(color: .clear, radius: 0, y: 0))
            .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
    }
}

/// A plain list row (SCR-08 tools, SCR-16 chats): icon, title, trailing text, chevron.
struct ListRow: View {
    var icon: String? = nil
    let title: String
    var subtitle: String? = nil
    var trailing: String? = nil
    var chevron = true
    var action: (() -> Void)? = nil

    var body: some View {
        Button { action?() } label: {
            HStack(spacing: Theme.Space.s) {
                if let icon {
                    Image(systemName: icon)
                        .font(.system(size: 18, weight: .semibold))
                        .frame(width: 28)
                        .foregroundStyle(Theme.Colors.textPrimary)
                }
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(Theme.Fonts.bodyBold).foregroundStyle(Theme.Colors.textPrimary)
                        .lineLimit(1)
                    if let subtitle {
                        Text(subtitle).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                    }
                }
                Spacer(minLength: 8)
                if let trailing {
                    Text(trailing).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                }
                if chevron && action != nil {
                    Image(systemName: "chevron.right").font(.system(size: 14, weight: .bold))
                        .foregroundStyle(Theme.Colors.textTertiary)
                }
            }
            .padding(.vertical, 14)
            .frame(minHeight: 56)
            .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
        .disabled(action == nil)
    }
}

#Preview("Rows") {
    VStack(spacing: 10) {
        RowButton(icon: "dumbbell.fill", title: "Enter the gym", subtitle: "Make Coder better · 3 runs left today", style: .primary) {}
        RowButton(icon: "message.fill", title: "Chat with OpenAgents", subtitle: "Ask us anything. No setup needed.") {}
        RowButton(icon: "person.fill", title: "Profile", subtitle: "Level, XP, help") {}
        ListRow(icon: "map", title: "PROJECT MAP", trailing: "Helps ✓") {}
    }
    .padding()
    .frame(maxHeight: .infinity)
    .background(Theme.Colors.background)
}
