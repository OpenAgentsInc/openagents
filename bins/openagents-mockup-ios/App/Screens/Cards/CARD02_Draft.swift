import SwiftUI

// CARD-02 Test set draft card: the draft the interview builds with the
// player (CHAT-10, FLOW-07). It changes as they approve each step; nothing
// runs until they tap TRY IT ONCE.

enum CARD02Step: String, Hashable, CaseIterable {
    /// Step 1: the tests, by name.
    case tests
    /// Step 2: how each test is checked.
    case checks
    /// Step 3: ready to try once.
    case ready
}

struct CARD02Draft: View {
    var step: CARD02Step = .tests
    var onLooksGood: () -> Void = {}
    var onTryOnce: () -> Void = {}
    var onChangeIt: () -> Void = {}
    var onSeeEvery: () -> Void = {}

    private var set: MockData.TestSet { MockData.testSet("changelog") }

    private var stepLine: String {
        switch step {
        case .tests: "First, the tests. Nothing runs yet."
        case .checks: "Now, how each test is checked."
        case .ready: "Ready. Try it once to see how it does."
        }
    }

    var body: some View {
        ChatCardFrame(highlighted: true) {
            // E01, E02
            HStack {
                Text("Your test set").condensedTitle(Theme.Fonts.cardTitle)
                Text("Draft").condensedTitle(Theme.Fonts.sectionLabel, tracking: Theme.Tracking.sectionLabel)
                    .padding(.horizontal, 8).padding(.vertical, 3)
                    .overlay(Capsule().stroke(Theme.Colors.strokeStrong, lineWidth: 1))
                Spacer()
            }
            Text("Tool: \(MockData.madeToolName) (yours)").font(Theme.Fonts.bodyBold)
            CardNote(text: stepLine, tertiary: true)

            // E03 (and each test's check from step 2)
            VStack(alignment: .leading, spacing: 8) {
                ForEach(Array(set.tests.enumerated()), id: \.offset) { i, test in
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Text("\(i + 1)").font(Theme.Fonts.bodyBold).frame(width: 16, alignment: .trailing)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(test.name).font(Theme.Fonts.body)
                                .fixedSize(horizontal: false, vertical: true)
                            if test.stayOut {
                                CardNote(text: "(\(MockData.stayOutNote.lowercased().dropLast()))", tertiary: true)
                            }
                            if step != .tests {
                                CardNote(text: test.checked, tertiary: true)
                            }
                        }
                    }
                }
            }

            // E06
            if step == .tests { CardNote(text: MockData.draftCheckLine) }

            // E05
            if step == .ready {
                PrimaryButton(title: "Try it once", detail: "One run · about 2 minutes · free") { onTryOnce() }
            } else {
                PrimaryButton(title: "Looks good") { onLooksGood() }
            }
            // E07, E04
            HStack(spacing: 8) {
                Chip(icon: "pencil", text: "Change it") { onChangeIt() }
                Chip(icon: "list.bullet", text: "See every test") { onSeeEvery() }
            }
        }
    }
}

#Preview("CARD-02 Draft: tests") {
    ScrollView { CARD02Draft(step: .tests).padding() }.background(Theme.Colors.background)
}

#Preview("CARD-02 Draft: checks") {
    ScrollView { CARD02Draft(step: .checks).padding() }.background(Theme.Colors.background)
}

#Preview("CARD-02 Draft: ready") {
    ScrollView { CARD02Draft(step: .ready).padding() }.background(Theme.Colors.background)
}
