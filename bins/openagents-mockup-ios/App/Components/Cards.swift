import SwiftUI

/// A plain surface card.
struct Card<Content: View>: View {
    var highlighted = false
    @ViewBuilder var content: Content

    var body: some View {
        content
            .padding(Theme.Space.m)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: Theme.Radius.card).fill(Theme.Colors.surface))
            .overlay(
                RoundedRectangle(cornerRadius: Theme.Radius.card)
                    .stroke(highlighted ? Theme.Colors.strokeStrong : Theme.Colors.stroke,
                            lineWidth: highlighted ? Theme.Stroke.selected : Theme.Stroke.hairline)
            )
    }
}

/// SCR-01.E09: the announcement (season) card.
struct AnnouncementCard: View {
    let title: String
    let line: String
    var action: (() -> Void)? = nil

    var body: some View {
        Button { action?() } label: {
            HStack(spacing: Theme.Space.s) {
                Image(systemName: "flag.checkered")
                    .font(.paper(22, weight: .bold))
                    .frame(width: 40)
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).condensedTitle(Theme.Fonts.sectionLabel, tracking: Theme.Tracking.sectionLabel)
                    Text(line).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        .multilineTextAlignment(.leading)
                }
                Spacer(minLength: 0)
            }
            .foregroundStyle(Theme.Colors.textPrimary)
            .padding(Theme.Space.m)
            .background(
                RoundedRectangle(cornerRadius: Theme.Radius.card)
                    .fill(LinearGradient(colors: [Color(white: 0.12), Theme.Colors.surface],
                                         startPoint: .topLeading, endPoint: .bottomTrailing))
            )
            .overlay(RoundedRectangle(cornerRadius: Theme.Radius.card).stroke(Theme.Colors.stroke, lineWidth: 1))
        }
        .buttonStyle(PressStyle())
    }
}

/// A read-only command card in chat (SCR-17.E07).
struct CommandCardView: View {
    let card: MockData.CommandCard
    @State private var ran = false

    var body: some View {
        Card {
            VStack(alignment: .leading, spacing: 8) {
                Text(card.command).font(Theme.Fonts.mono).foregroundStyle(Theme.Colors.textPrimary)
                Text(card.note).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                if ran {
                    Text(card.output)
                        .font(Theme.Fonts.captionMono)
                        .foregroundStyle(Theme.Colors.textSecondary)
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 8).fill(Color.black))
                }
                Chip(icon: "terminal", text: ran ? "Run again" : "Run") {
                    withAnimation { ran = true }
                }
            }
        }
    }
}

#Preview("Cards") {
    ScrollView {
        VStack(spacing: 12) {
            AnnouncementCard(title: MockData.season, line: MockData.seasonLine)
            CommandCardView(card: MockData.answer("can").command!)
        }
        .padding()
    }
    .background(Theme.Colors.background)
}
