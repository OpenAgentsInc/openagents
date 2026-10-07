import SwiftUI

// CARD-07 Credit card: what the player's work has earned (CHAT-14,
// FLOW-10), from the XP ledger. XP when another trainer checks a result or
// test set, and when Coder adopts a tool. No money.

enum CARD07State: String, Hashable, CaseIterable {
    case rows, empty
}

struct CARD07Credit: View {
    @Environment(MockApp.self) private var app
    var state: CARD07State = .rows
    var onAsk: (String) -> Void = { _ in }

    var body: some View {
        if state == .empty {
            VStack(alignment: .leading, spacing: Theme.Space.s) {
                ChatCardFrame {
                    Text("Your credit").condensedTitle(Theme.Fonts.cardTitle)
                    Text(MockData.creditEmpty).font(Theme.Fonts.body)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Chip(icon: "dumbbell", text: "Test a tool") { onAsk("testATool") }
            }
        } else {
            ChatCardFrame(highlighted: true) {
                // E01
                Text("Your credit").condensedTitle(Theme.Fonts.cardTitle)
                // E02
                ForEach(MockData.credit) { group in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(group.title).font(Theme.Fonts.bodyBold)
                        ForEach(group.rows) { row in
                            HStack(spacing: 8) {
                                Image(systemName: row.done ? "checkmark" : "ellipsis")
                                    .font(.paper(13, weight: .heavy))
                                    .foregroundStyle(row.done ? Theme.Colors.markPass : Theme.Colors.markFail)
                                    .frame(width: 18)
                                Text(row.text).font(Theme.Fonts.body)
                                    .foregroundStyle(row.done ? Theme.Colors.textPrimary : Theme.Colors.textSecondary)
                                Spacer()
                                if let xp = row.xp { Text("+\(xp) XP").font(Theme.Fonts.bodyBold) }
                            }
                        }
                    }
                }
                // E03
                Text("Level \(app.level) · \(app.xpForNextLevel - app.xp) XP to level \(app.level + 1)")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                // E04
                PrimaryShareLink(title: "Share what you made", item: MockData.creditShareText)
                // E05
                CardNote(text: MockData.creditHonesty, tertiary: true)
            }
        }
    }
}

#Preview("CARD-07 Credit") {
    ScrollView { CARD07Credit().padding() }.background(Theme.Colors.background).environment(MockApp())
}

#Preview("CARD-07 Nothing yet") {
    ScrollView { CARD07Credit(state: .empty).padding() }.background(Theme.Colors.background).environment(MockApp())
}
