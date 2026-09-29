import SwiftUI

// SCR-02 Choose your agent. First run, step 1 of 3.

enum SCR02State: String, Hashable, CaseIterable {
    case normal, loading
}

struct SCR02ChooseAgent: View {
    @Environment(MockApp.self) private var app
    let state: SCR02State

    var body: some View {
        ScreenScaffold {
            // E01
            TopBar(title: "", step: 1) {
                EmptyView()
            }
            .overlay(alignment: .leading) {
                Text("STEP 1 OF 3").condensedTitle(Theme.Fonts.sectionLabel, tracking: Theme.Tracking.sectionLabel)
                    .foregroundStyle(Theme.Colors.textSecondary)
                    .padding(.leading, Theme.Space.page)
                    .onLongPressGesture { app.showIndex = true }
            }
        } content: {
            // E02
            VStack(alignment: .leading, spacing: 6) {
                Text("Choose your agent").font(Theme.Fonts.title)
                Text("Your agent is an AI that writes code. You'll train it to get better.")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
            }

            // E03
            Card(highlighted: true) {
                VStack(alignment: .leading, spacing: Theme.Space.s) {
                    ZStack {
                        LinearGradient(colors: [Color(white: 0.12), .black], startPoint: .top, endPoint: .bottom)
                        GridFloor(horizon: 0.55, lines: 14)
                        CoderFigure().frame(height: 210).padding(.top, 10)
                    }
                    .frame(height: 240)
                    .clipShape(RoundedRectangle(cornerRadius: Theme.Radius.card - 4))

                    HStack {
                        Text("Coder").condensedTitle(Theme.Fonts.headline)
                        Spacer()
                        Label("Selected", systemImage: "checkmark.circle.fill")
                            .font(Theme.Fonts.bodyBold)
                    }
                    Text("Writes and fixes code.").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    HStack(spacing: 6) {
                        Text("Today:").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                        if state == .loading {
                            LoadingBar(width: 150, height: 14)
                        } else {
                            Text("passes \(MockData.starterPassedToday) of \(MockData.starterTests) starter tests")
                                .font(Theme.Fonts.bodyBold)
                        }
                    }
                }
            }
        } bottom: {
            // E05, E04, E06
            NextLine(text: "choose Coder to begin.")
            PrimaryButton(title: "Choose Coder") { app.go(.cinematic(0)) }
            SecondaryLink(title: "Ask OpenAgents a question first") { app.go(.newChat(.fromFirstRun)) }
        }
    }
}

#Preview("SCR-02 Choose your agent") {
    NavigationStack { SCR02ChooseAgent(state: .normal) }.environment(MockApp())
}

#Preview("SCR-02 Loading") {
    NavigationStack { SCR02ChooseAgent(state: .loading) }.environment(MockApp())
}
