import SwiftUI

// SCR-05 Result (detail). Opened from CARD-04's See details: the headline,
// tests passed without and with the tool, and each test with a mark per
// side. The with-the-tool count counts up as a fake reveal.

enum SCR05State: String, Hashable, CaseIterable {
    case better, noChange, worse, firstTry, added, addingFailed, confirmed, didntHold

    init(outcomeKey: String, added: Bool) {
        if added { self = .added; return }
        self = SCR05State(rawValue: outcomeKey == "madeBetter" ? "better" : outcomeKey) ?? .better
    }

    var outcomeKey: String {
        switch self {
        case .added, .addingFailed: "better"
        default: rawValue
        }
    }
}

struct SCR05Result: View {
    @Environment(MockApp.self) private var app
    let state: SCR05State
    /// The chat message whose CARD-04 opened this, so adding here updates it.
    var messageID: UUID? = nil
    @State private var shownWith = 0
    @State private var revealed = false

    private var o: MockData.Outcome { MockData.outcome(state.outcomeKey) }
    private var set: MockData.TestSet { MockData.testSet(o.testSet) }
    private var isAdded: Bool { state == .added }

    private var headlineColor: Color {
        switch state {
        case .noChange, .didntHold: Theme.Colors.verdictNoChange
        case .worse: Theme.Colors.verdictWorse
        default: Theme.Colors.verdictBetter
        }
    }

    var body: some View {
        if state == .addingFailed {
            PAT01States(state: .addingFailed)
        } else {
            ScreenScaffold {
                // E01
                TopBar(back: BackControl(label: "Chat") { app.returnToChat() }, title: "Your result")
            } content: {
                // E02
                Text(o.headline)
                    .condensedTitle(Theme.Fonts.headline, tracking: 1)
                    .foregroundStyle(headlineColor)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)
                    .opacity(revealed ? 1 : 0)
                    .scaleEffect(revealed ? 1 : 0.9)
                    .padding(.top, Theme.Space.xs)

                // E03
                HStack(alignment: .top, spacing: Theme.Space.s) {
                    score(o.withoutCount, caption: "without the tool")
                    Image(systemName: "arrow.right").font(.paper(26, weight: .bold)).padding(.top, 10)
                    score(shownWith, caption: "with \(o.toolName)")
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, Theme.Space.m)
                .background(RoundedRectangle(cornerRadius: Theme.Radius.card).fill(Theme.Colors.surface))
                .overlay(RoundedRectangle(cornerRadius: Theme.Radius.card).stroke(Theme.Colors.stroke, lineWidth: 1))

                // E12
                testList
                // E13
                OutlinedButton(title: "See the whole test set", icon: "list.bullet") {
                    app.present(.testSet(o.testSet, draft: false))
                }

                // E04
                VStack(alignment: .leading, spacing: 8) {
                    Text(isAdded ? "Added. \(o.xpLine)." : o.xpLine)
                        .font(Theme.Fonts.bodyBold).fixedSize(horizontal: false, vertical: true)
                    XPBar(progress: Double(app.xp) / Double(app.xpForNextLevel))
                    Text("Level \(app.level) · \(app.xp)/\(app.xpForNextLevel) XP")
                        .font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                }
                // E05
                Text(o.why).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
            } bottom: {
                if isAdded {
                    Label("Added to the Gym", systemImage: "checkmark.circle.fill")
                        .font(Theme.Fonts.bodyBold)
                        .frame(maxWidth: .infinity, minHeight: 40)
                    NextLine(text: "check someone else's result for more XP.")
                    PrimaryButton(title: "Back to chat") { app.returnToChat() }
                } else if o.isFirstTry {
                    NextLine(text: "run the full test set to add it to the Gym.")
                    PrimaryButton(title: "Run the full test set") {
                        app.returnToChat(command: .runFullTestSet(messageID))
                    }
                } else {
                    // E06, E07 (opens SCR-20, which says what becomes public)
                    NextLine(text: "add your result to the Gym.")
                    PrimaryButton(title: "Add to the Gym") {
                        app.present(.addToGym(.normal, state.outcomeKey, messageID))
                    }
                }
                // E09, E10, E11
                ShareLink(item: o.shareText) {
                    Label("Share outside the app", systemImage: "square.and.arrow.up")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        .frame(maxWidth: .infinity, minHeight: 36)
                }
                HStack {
                    SecondaryLink(title: "Test another tool") { app.returnToChat(sending: "whichTool") }
                    SecondaryLink(title: "Ask about this result") { app.returnToChat(sending: "result") }
                }
            }
            .task { await reveal() }
        }
    }

    private var testList: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionLabel(text: "Tests").padding(.bottom, 6)
            HStack(spacing: 6) {
                Text("without").frame(width: 50, alignment: .center)
                Text("with").frame(width: 30, alignment: .center)
                Spacer()
            }
            .font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary)
            ForEach(Array(set.tests.enumerated()), id: \.offset) { i, test in
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    TestMark(passed: o.passedWithout(i)).frame(width: 50)
                    TestMark(passed: o.passedWith(i)).frame(width: 30)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(test.name).font(Theme.Fonts.body).fixedSize(horizontal: false, vertical: true)
                        if test.stayOut {
                            Text(MockData.stayOutNote).font(Theme.Fonts.caption)
                                .foregroundStyle(Theme.Colors.textSecondary)
                        }
                    }
                    Spacer(minLength: 0)
                }
                .padding(.vertical, 7)
                if i < set.tests.count - 1 { Divider().overlay(Theme.Colors.divider) }
            }
        }
    }

    private func score(_ n: Int, caption: String) -> some View {
        VStack(spacing: 2) {
            Text("\(n) of \(o.total)").font(Theme.Fonts.hugeNumber)
                .contentTransition(.numericText())
            Text(caption).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                .lineLimit(1).minimumScaleFactor(0.8)
        }
        .frame(minWidth: 120)
    }

    private func reveal() async {
        guard !revealed else { return }
        shownWith = o.withoutCount
        try? await Task.sleep(for: .seconds(0.4))
        while shownWith != o.withCount {
            try? await Task.sleep(for: .seconds(0.35))
            withAnimation(.snappy) { shownWith += o.withCount > shownWith ? 1 : -1 }
        }
        try? await Task.sleep(for: .seconds(0.2))
        withAnimation(Theme.Motion.reveal) { revealed = true }
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }
}

#Preview("SCR-05 Better") {
    NavigationStack { SCR05Result(state: .better) }.environment(MockApp())
}

#Preview("SCR-05 No clear change") {
    NavigationStack { SCR05Result(state: .noChange) }.environment(MockApp())
}

#Preview("SCR-05 Worse") {
    NavigationStack { SCR05Result(state: .worse) }.environment(MockApp())
}

#Preview("SCR-05 First try") {
    NavigationStack { SCR05Result(state: .firstTry) }.environment(MockApp())
}

#Preview("SCR-05 Added") {
    NavigationStack { SCR05Result(state: .added) }.environment(MockApp())
}

#Preview("SCR-05 You confirmed it") {
    NavigationStack { SCR05Result(state: .confirmed) }.environment(MockApp())
}
