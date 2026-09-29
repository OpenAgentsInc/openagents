import SwiftUI

// SCR-05 Result. The payoff: before and after, XP, and one action that
// adds the result to the Gym. The after score counts up as a fake reveal.

enum SCR05State: String, Hashable, CaseIterable {
    case better, noChange, worse, added, addingFailed, checkConfirmed, checkFailed
}

struct SCR05Result: View {
    @Environment(MockApp.self) private var app
    let state: SCR05State
    @State private var shownAfter: Int = MockData.coderScoreBefore
    @State private var revealed = false
    @State private var adding = false
    @State private var added = false

    private var isAdded: Bool { added || state == .added }
    private var before: Int { MockData.coderScoreBefore }
    private var after: Int {
        switch state {
        case .noChange, .checkFailed: before
        case .worse: before - 1
        default: MockData.coderScoreAfter
        }
    }

    private var headline: String {
        switch state {
        case .better, .added: "Coder got better"
        case .noChange: "No clear change"
        case .worse: "Coder did worse with this tool"
        case .checkConfirmed: "You confirmed it"
        case .checkFailed: "It didn't hold up"
        case .addingFailed: ""
        }
    }

    private var headlineColor: Color {
        switch state {
        case .noChange, .checkFailed: Theme.Colors.verdictNoChange
        case .worse: Theme.Colors.verdictWorse
        default: Theme.Colors.verdictBetter
        }
    }

    private var why: String {
        switch state {
        case .noChange: "That's useful too. Now everyone knows this tool doesn't help here."
        case .worse: "That's useful too. We won't give Coder this tool."
        case .checkConfirmed, .checkFailed: "Checks keep the Gym honest. Every trainer can trust what ships."
        default: "Your result helps every trainer. When others confirm it, Coder uses \(toolName) for everyone."
        }
    }

    private var toolName: String {
        state == .checkConfirmed || state == .checkFailed ? MockData.checkTool : app.selectedTool.name
    }

    var body: some View {
        if state == .addingFailed {
            PAT01States(state: .addingFailed)
        } else {
            ScreenScaffold {
                // E01
                TopBar(back: BackControl(label: "Menu") { app.backToMenu() }, title: "Your result")
            } content: {
                // E02
                Text(headline)
                    .condensedTitle(Theme.Fonts.headline, tracking: 1)
                    .foregroundStyle(headlineColor)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)
                    .opacity(revealed ? 1 : 0)
                    .scaleEffect(revealed ? 1 : 0.9)
                    .padding(.top, Theme.Space.s)

                // E03
                HStack(alignment: .top, spacing: Theme.Space.m) {
                    score(before, caption: "before")
                    Image(systemName: "arrow.right").font(.system(size: 28, weight: .bold))
                        .padding(.top, 10)
                    score(shownAfter, caption: "with \(toolName)")
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, Theme.Space.m)
                .background(RoundedRectangle(cornerRadius: Theme.Radius.card).fill(Theme.Colors.surface))
                .overlay(RoundedRectangle(cornerRadius: Theme.Radius.card).stroke(Theme.Colors.stroke, lineWidth: 1))

                // E04
                VStack(alignment: .leading, spacing: 8) {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text("+\(MockData.xpPerRun) XP").font(Theme.Fonts.hugeNumber)
                        Text(isAdded ? "Added. It counts once another trainer checks it."
                                     : "Pending until another trainer checks it")
                            .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    }
                    XPBar(progress: Double(app.xp + (isAdded ? 0 : MockData.xpPerRun)) / Double(app.xpForNextLevel))
                    Text("Level \(app.level) · \(app.xp + (isAdded ? 0 : MockData.xpPerRun))/\(app.xpForNextLevel) XP")
                        .font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                }

                // E05
                Text(why).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
            } bottom: {
                if isAdded {
                    Label("Added to the Gym", systemImage: "checkmark.circle.fill")
                        .font(Theme.Fonts.bodyBold)
                        .frame(maxWidth: .infinity, minHeight: 44)
                    NextLine(text: "come back tomorrow for new runs.")
                    PrimaryButton(title: "Back to menu") { app.backToMenu() }
                } else {
                    // E06, E07, E08
                    NextLine(text: "add your result to the Gym.")
                    PrimaryButton(title: adding ? "Adding…" : "Add my result to the gym", enabled: !adding) { add() }
                    Text("Your result and trainer name are public.")
                        .font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary)
                }
                // E09, E10, E11
                ShareLink(item: "Coder went from \(before) of 10 to \(after) of 10 with \(toolName) on OpenAgents.") {
                    Label("Share outside the app", systemImage: "square.and.arrow.up")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        .frame(maxWidth: .infinity, minHeight: 40)
                }
                HStack {
                    SecondaryLink(title: "Try another tool") { app.go(.gym(.returning)) }
                    SecondaryLink(title: "Ask about this result") { app.go(.conversation(.aboutResult)) }
                }
            }
            .task { await reveal() }
        }
    }

    private func score(_ n: Int, caption: String) -> some View {
        VStack(spacing: 2) {
            Text("\(n) of \(MockData.practiceTasks)").font(Theme.Fonts.hugeNumber)
                .contentTransition(.numericText())
            Text(caption).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
        }
        .frame(minWidth: 120)
    }

    private func reveal() async {
        added = state == .added
        guard !revealed else { return }
        try? await Task.sleep(for: .seconds(0.4))
        while shownAfter != after {
            try? await Task.sleep(for: .seconds(0.35))
            withAnimation(.snappy) { shownAfter += after > shownAfter ? 1 : -1 }
        }
        try? await Task.sleep(for: .seconds(0.2))
        withAnimation(Theme.Motion.reveal) { revealed = true }
        UINotificationFeedbackGenerator().notificationOccurred(.success)
    }

    private func add() {
        adding = true
        Task {
            try? await Task.sleep(for: .seconds(0.8))
            adding = false
            withAnimation { added = true }
            if app.addResult() { app.go(.levelUp(.withTitle)) }
        }
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

#Preview("SCR-05 Added") {
    NavigationStack { SCR05Result(state: .added) }.environment(MockApp())
}
