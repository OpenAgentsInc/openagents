import SwiftUI

// SCR-03 The Gym: pick a tool. RETIRED in revision 3 (now CARD-01 in chat).
// Kept as revision 2 drew it, reachable only from the Screen index.

enum SCR03State: String, Hashable, CaseIterable {
    case firstRun, returning, checkWaiting, noRunsLeft, loading, offline
}

struct SCR03Gym: View {
    @Environment(MockApp.self) private var app
    let state: SCR03State
    @State private var checkSelected = true
    @State private var selectedToolID = MockData.defaultTool.id

    private var isFirstRun: Bool { state == .firstRun }
    private var showsCheck: Bool { state == .checkWaiting && checkSelected }

    var body: some View {
        if state == .offline {
            PAT01States(state: .offline)
        } else {
            ScreenScaffold {
                // E01, E02
                TopBar(back: isFirstRun ? nil : BackControl(label: "Menu") { app.backToMenu() },
                       title: "The Gym", step: isFirstRun ? 2 : nil)
            } content: {
                // E03
                VStack(alignment: .leading, spacing: 6) {
                    Text("Give Coder a new tool.").font(Theme.Fonts.title)
                    Text("We'll test it on \(Rev2.practiceTasks) practice tasks and show you if Coder got better.")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                }

                SectionLabel(text: "Recommended").padding(.top, Theme.Space.xs)
                if state == .checkWaiting {
                    // E11
                    Button { checkSelected = true } label: { CheckCard().opacity(checkSelected ? 1 : 0.6) }
                        .buttonStyle(PressStyle())
                }
                // E04, E05
                ForEach(MockData.tools) { tool in
                    ToolCard(tool: tool,
                             selected: !showsCheck && selectedToolID == tool.id,
                             loadingCounts: state == .loading) {
                        checkSelected = false
                        selectedToolID = tool.id
                    }
                }

                // E06 (later)
                if MockData.showLaterFeatures && !isFirstRun {
                    SecondaryLink(title: "See all tools", icon: "square.grid.2x2") { app.go(.allTools(.normal)) }
                }
            } bottom: {
                // E07, E08, E09, E10
                if state == .noRunsLeft {
                    Text("New runs at 9:00 tomorrow.").font(Theme.Fonts.body)
                        .foregroundStyle(Theme.Colors.textSecondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    PrimaryButton(title: "Back to menu") { app.backToMenu() }
                } else {
                    Text(showsCheck ? "Takes about 5 minutes. Free. Doesn't use a run." : "Takes about 5 minutes. Free.")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    NextLine(text: showsCheck ? "tap Start the check." : "tap Start training.")
                    PrimaryButton(title: showsCheck ? "Start the check" : "Start training") {
                        if showsCheck { app.go(.retiredTraining(.check)) } else { app.go(.retiredTraining(isFirstRun ? .firstRun : .running)) }
                    }
                    Text("\(app.runsLeft) runs left today")
                        .font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary)
                }
            }
        }
    }
}

#Preview("SCR-03 First run") {
    NavigationStack { SCR03Gym(state: .firstRun) }.environment(MockApp())
}

#Preview("SCR-03 Check waiting") {
    NavigationStack { SCR03Gym(state: .checkWaiting) }.environment(MockApp())
}

#Preview("SCR-03 No runs left") {
    NavigationStack { SCR03Gym(state: .noRunsLeft) }.environment(MockApp())
}
