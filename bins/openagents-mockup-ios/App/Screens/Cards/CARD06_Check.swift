import SwiftUI

// CARD-06 Check card: another trainer's result, waiting for someone to run
// the same tests (CHAT-13, FLOW-08). Never the player's own result, and a
// check never uses one of today's runs.

enum CARD06State: String, Hashable, CaseIterable {
    case ready, noneWaiting
}

struct CARD06Check: View {
    var state: CARD06State = .ready
    var onRun: () -> Void = {}
    var onSeeTests: () -> Void = {}
    var onAsk: (String) -> Void = { _ in }

    var body: some View {
        if state == .noneWaiting {
            VStack(alignment: .leading, spacing: Theme.Space.s) {
                ChatCardFrame {
                    Text("No results need a check right now.").font(Theme.Fonts.body)
                    CardNote(text: "We'll say so on the menu when one does.")
                }
                Chip(icon: "dumbbell", text: "Test a tool") { onAsk("testATool") }
            }
        } else {
            ChatCardFrame(highlighted: true) {
                // E01
                HStack {
                    Text("Check a result").condensedTitle(Theme.Fonts.cardTitle)
                    Spacer()
                    Text("+\(MockData.xpForACheck) XP").font(Theme.Fonts.bodyBold)
                }
                // E02
                Text("\(MockData.checkTrainer) says \(MockData.checkTool) made Coder pass \(MockData.checkClaimAfter) of 8 tests instead of \(MockData.checkClaimBefore).")
                    .font(Theme.Fonts.body)
                    .fixedSize(horizontal: false, vertical: true)
                CardNote(text: "Run the same tests to check it.")
                // E03
                CardNote(text: "About \(MockData.runMinutes) minutes. Doesn't use a run.")
                // E04
                PrimaryButton(title: "Run the check") { onRun() }
                Chip(icon: "list.bullet", text: "See the tests") { onSeeTests() }
            }
        }
    }
}

#Preview("CARD-06 Check") {
    ScrollView { CARD06Check().padding() }.background(Theme.Colors.background)
}

#Preview("CARD-06 None waiting") {
    ScrollView { CARD06Check(state: .noneWaiting).padding() }.background(Theme.Colors.background)
}
